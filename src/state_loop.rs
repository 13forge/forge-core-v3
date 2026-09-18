//! Dual-loop state machine with triple-buffer lock-free architecture.
//! Loop 1 (decay_loop): PentaractField → decay → vesting → write snapshot.
//! Loop 2 (readout_loop): stable snapshot → spectral → trit transition.
//! No locks, O(1) atomic swaps, deterministic snapshot isolation.

use crate::pentaract_field::PentaractField;
use crate::spectral::AntiShannonSpectral;
use crate::trit_bijection::TritStateMachine;
use crate::vested_leaky::VestedLeaky32;
use core::cell::Cell;
use core::sync::atomic::{AtomicUsize, Ordering};

/// One permyriad. Matches decay.rs PMY constant.
pub const PMY: u64 = 10_000;

/// VestedTripleBuffer: lock-free snapshot isolation for vested state. Three buffers rotate:
/// - write_idx: being filled by decay_loop (next to write)
/// - written_idx: just filled by decay_loop (ready to read)
/// - read_idx: stable snapshot for readout_loop
pub struct VestedTripleBuffer<T: Copy> {
    buffers: [Cell<T>; 3],
    write_idx: AtomicUsize,
    written_idx: AtomicUsize,
    read_idx: AtomicUsize,
}

impl<T: Copy> VestedTripleBuffer<T> {
    /// Create a new triple-buffer with all slots initialized to `default`.
    pub fn new(default: T) -> Self {
        Self {
            buffers: [Cell::new(default), Cell::new(default), Cell::new(default)],
            write_idx: AtomicUsize::new(0),
            written_idx: AtomicUsize::new(0),
            read_idx: AtomicUsize::new(1),
        }
    }

    /// Write a snapshot to the write buffer, then rotate: write → read, read → stale.
    /// After write_snapshot, the written data is available for advance_read to promote it.
    pub fn write_snapshot(&self, value: T) {
        let w_idx = self.write_idx.load(Ordering::Acquire);
        self.buffers[w_idx].set(value);
        self.written_idx.store(w_idx, Ordering::Release);
    }

    /// Read the current stable snapshot without locks.
    pub fn read_snapshot(&self) -> T {
        let r_idx = self.read_idx.load(Ordering::Acquire);
        self.buffers[r_idx].get()
    }

    /// Advance the read pointer to consume the latest written snapshot.
    /// Rotates: new_read = last_written, new_write = old_read.
    pub fn advance_read(&self) {
        let w_idx = self.written_idx.load(Ordering::Acquire);
        let r_idx = self.read_idx.load(Ordering::Acquire);
        self.read_idx.store(w_idx, Ordering::Release);
        self.write_idx.store(r_idx, Ordering::Release);
    }
}

// VestedTripleBuffer size assertion: 3 Cell-wrapped buffers of VestedLeaky32 (768 bytes each) + 3 atomics
// (AtomicUsize: 8 B on 64-bit; 4 B on wasm32, where the 8-aligned payload rounds 12 → 16).
#[cfg(target_pointer_width = "64")]
const _: () = assert!(core::mem::size_of::<VestedTripleBuffer<VestedLeaky32>>() == 768 * 3 + 24);
#[cfg(target_pointer_width = "32")]
const _: () = assert!(core::mem::size_of::<VestedTripleBuffer<VestedLeaky32>>() == 768 * 3 + 16);

/// Per-channel decay: apply channel_rates to field and sum the decayed results.
/// Returns a VestedLeaky32 with decay state populated.
fn decay_field(field: &PentaractField, decay_rates: &[u16; 32]) -> VestedLeaky32 {
    let mut vl = VestedLeaky32::new();
    let channels = field.channels();

    for i in 0..32 {
        let keep = PMY - decay_rates[i] as u64;
        let decayed = ((channels[i] as i64 * keep as i64) / PMY as i64) as u64;
        vl.decay[i] = decayed;
    }

    vl
}

/// Apply vest caps and return the per-channel maximum (vest floor).
fn vested_floor(decayed: &VestedLeaky32, vest_caps: &[u64; 32]) -> VestedLeaky32 {
    let mut vl = *decayed;
    vl.caps = *vest_caps;
    vl
}

/// Spectral transformation: anti-Shannon concentration from vested state.
fn anti_shannon_per_channel(vested: &VestedLeaky32) -> AntiShannonSpectral {
    let mut v = *vested;
    AntiShannonSpectral::from_field(&v.compose())
}

/// Trit state machine transition: quantize spectral to trits and compute next state.
fn trit_machine_transition(
    spectral: &AntiShannonSpectral,
    machine: &TritStateMachine,
) -> TritStateMachine {
    let next = machine.next_state(&spectral.channels);
    TritStateMachine {
        prior_state: next,
        threshold: machine.threshold,
    }
}

/// 3-axis trit state: each axis is a trit (-1, 0, +1)
/// Captures duality: energy flow, boundary, distribution as emergent equilibria.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TriDualityState {
    /// Energy flow axis: +1=Accretion, 0=Equilibrium, -1=Decay.
    pub energy_flow: i8,
    /// Boundary axis: +1=Vesting, 0=Threshold, -1=Dissolution.
    pub boundary: i8,
    /// Distribution axis: +1=Purity, 0=Resonance, -1=Dispersion.
    pub distribution: i8,
}

impl TriDualityState {
    /// Create a new equilibrium state (all axes at 0).
    pub fn new() -> Self {
        Self {
            energy_flow: 0,
            boundary: 0,
            distribution: 0,
        }
    }
}

impl Default for TriDualityState {
    fn default() -> Self {
        Self::new()
    }
}

/// Compute energy axis: Accretion (input rate) vs Decay (FMA leak).
/// Result: +1 if accretion > decay, -1 if decay > accretion, 0 if equal.
pub fn compute_energy_axis(
    accretion_rate: u64,
    field: &PentaractField,
    decay_rates: &[u16; 32],
) -> i8 {
    let total_accretion = accretion_rate;
    let channels = field.channels();
    let total_decay = channels
        .iter()
        .zip(decay_rates.iter())
        .map(|(ch, rate)| (*ch as u64).wrapping_mul(*rate as u64) / PMY)
        .sum::<u64>();

    if total_accretion > total_decay {
        1i8
    } else if total_accretion < total_decay {
        -1i8
    } else {
        0i8
    }
}

/// Compute boundary axis: Vesting (fixity) vs Dissolution (erasure).
/// Result: +1 if vesting > dissolution, -1 if dissolution > vesting, 0 if equal.
pub fn compute_boundary_axis(
    vested: &VestedLeaky32,
    dissolution_rate: u64,
) -> i8 {
    let total_vesting = vested.caps.iter().sum::<u64>();
    let total_dissolution = dissolution_rate;

    if total_vesting > total_dissolution {
        1i8
    } else if total_vesting < total_dissolution {
        -1i8
    } else {
        0i8
    }
}

/// Compute distribution axis: Purity (singular peaks) vs Dispersion (flat noise).
/// High variance = singular peaks (+1), low variance = flat (-1), mid = resonance (0).
pub fn compute_distribution_axis(spectral: &AntiShannonSpectral) -> i8 {
    let concentrations = &spectral.channels;
    let total: u64 = concentrations.iter().map(|&c| c as u64).sum();
    let mean = if total > 0 {
        (total as f64) / 32.0
    } else {
        0.0
    };

    let variance = concentrations
        .iter()
        .map(|&c| {
            let diff = (c as f64) - mean;
            diff * diff
        })
        .sum::<f64>()
        / 32.0;

    if variance > 100.0 {
        1i8
    } else if variance < 10.0 {
        -1i8
    } else {
        0i8
    }
}

/// Detect 0-crossings: phase boundary when any axis crosses zero (sign flip).
/// Returns true if previous and current states have opposite signs on any axis.
pub fn detect_phase_shifts(prev_state: &TriDualityState, curr_state: &TriDualityState) -> bool {
    (prev_state.energy_flow * curr_state.energy_flow <= 0 && prev_state.energy_flow != 0)
        || (prev_state.boundary * curr_state.boundary <= 0 && prev_state.boundary != 0)
        || (prev_state.distribution * curr_state.distribution <= 0 && prev_state.distribution != 0)
}

/// Closed-loop tick: compute all 3 axes in parallel, detect 0-crossings for trit snap.
/// Returns (TriDualityState, next TritStateMachine, phase_shifted bool).
pub fn tick_6primitive_closed_loop(
    field: &PentaractField,
    decay_rates: &[u16; 32],
    vest_caps: &[u64; 32],
    accretion_rate: u64,
    dissolution_rate: u64,
    prev_trit_state: &TritStateMachine,
) -> (TriDualityState, TritStateMachine, bool) {
    let decayed = decay_field(field, decay_rates);
    let vested = vested_floor(&decayed, vest_caps);
    let spectral = anti_shannon_per_channel(&vested);

    let energy_axis = compute_energy_axis(accretion_rate, field, decay_rates);
    let boundary_axis = compute_boundary_axis(&vested, dissolution_rate);
    let distribution_axis = compute_distribution_axis(&spectral);

    let curr_state = TriDualityState {
        energy_flow: energy_axis,
        boundary: boundary_axis,
        distribution: distribution_axis,
    };

    let prev_duality = TriDualityState::new();
    let phase_shifted = detect_phase_shifts(&prev_duality, &curr_state);

    let next_trit = if phase_shifted {
        trit_machine_transition(&spectral, prev_trit_state)
    } else {
        TritStateMachine {
            prior_state: prev_trit_state.prior_state,
            threshold: prev_trit_state.threshold,
        }
    };

    (curr_state, next_trit, phase_shifted)
}

/// Dual-loop pentaract 5D state machine.
/// Loop 1: decay → vesting → triple_buffer write (atomic swap).
/// Loop 2: buffer read → spectral → trit machine (lock-free).
/// Returns (spectral, trit_machine) after one full cycle.
pub fn tick_pentaract_5d_dual_loop(
    field: &PentaractField,
    decay_rates: &[u16; 32],
    vest_caps: &[u64; 32],
    trit_prior: &TritStateMachine,
) -> (AntiShannonSpectral, TritStateMachine) {
    let buffer = VestedTripleBuffer::new(VestedLeaky32::new());

    let decayed = decay_field(field, decay_rates);
    let vested = vested_floor(&decayed, vest_caps);
    buffer.write_snapshot(vested);

    buffer.advance_read();
    let snapshot = buffer.read_snapshot();

    let spectral = anti_shannon_per_channel(&snapshot);
    let trit_machine = trit_machine_transition(&spectral, trit_prior);

    (spectral, trit_machine)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pentaract::Pentaract;
    use crate::pentaract_field::SenseChannel;
    use crate::trit_bijection::TritArray32;

    fn test_point() -> Pentaract {
        Pentaract::new(0x13, 1000, 2000, 3000, 4000, 0xC3A256FF, 0)
    }

    #[test]
    fn test_triple_buffer_snapshot_isolation() {
        let buf = VestedTripleBuffer::new(VestedLeaky32::new());

        let mut v1 = VestedLeaky32::new();
        v1.decay[0] = 5_000;

        let mut v2 = VestedLeaky32::new();
        v2.decay[0] = 7_000;

        buf.write_snapshot(v1);
        buf.advance_read();
        let snapshot1 = buf.read_snapshot();
        assert_eq!(snapshot1.decay[0], 5_000);

        buf.write_snapshot(v2);
        let snapshot_before_advance = buf.read_snapshot();
        assert_eq!(snapshot_before_advance.decay[0], 5_000, "snapshot remains stable until advance");

        buf.advance_read();
        let snapshot2 = buf.read_snapshot();
        assert_eq!(snapshot2.decay[0], 7_000);
    }

    #[test]
    fn test_dual_loop_determinism() {
        let mut field = PentaractField::quiet_at(test_point());
        field[SenseChannel::HeatGradient] = 5_000;
        field[SenseChannel::UvFlux] = 3_000;
        field[SenseChannel::LuxZero] = 2_000;

        let mut decay_rates = [100u16; 32];
        decay_rates[0] = 150;
        decay_rates[1] = 120;

        let vest_caps = [10_000u64; 32];

        let prior = TritArray32::from_base3_index(12345);
        let trit_machine = TritStateMachine {
            prior_state: prior,
            threshold: 10,
        };

        let (spec1, _) = tick_pentaract_5d_dual_loop(&field, &decay_rates, &vest_caps, &trit_machine);
        let (spec2, _) = tick_pentaract_5d_dual_loop(&field, &decay_rates, &vest_caps, &trit_machine);

        assert_eq!(spec1.concentration, spec2.concentration, "spectral concentration must be deterministic");
        for i in 0..32 {
            assert_eq!(spec1.channels[i], spec2.channels[i], "channel {} mismatch", i);
        }
    }

    #[test]
    fn test_decay_per_channel() {
        let mut field = PentaractField::quiet_at(test_point());
        field[SenseChannel::HeatGradient] = 10_000;
        field[SenseChannel::UvFlux] = 5_000;

        let decay_rates = [0u16; 32];
        let vested = decay_field(&field, &decay_rates);

        assert_eq!(vested.decay[0], 10_000, "no decay (0 rate) preserves channel");
        assert_eq!(vested.decay[1], 5_000);
    }

    #[test]
    fn test_decay_leaky_floor() {
        let mut field = PentaractField::quiet_at(test_point());
        field[SenseChannel::HeatGradient] = 10_000;

        let mut decay_rates = [0u16; 32];
        decay_rates[0] = 5_000;

        let vested = decay_field(&field, &decay_rates);

        let keep = PMY - decay_rates[0] as u64;
        let expected = (10_000u64 * keep) / PMY;
        assert_eq!(vested.decay[0], expected);
    }

    #[test]
    fn test_spectral_from_vested() {
        let mut vested = VestedLeaky32::new();
        vested.decay[0] = 10_000;
        vested.decay[1] = 5_000;
        for i in 2..32 {
            vested.decay[i] = 1_000;
        }

        let spectral = anti_shannon_per_channel(&vested);
        assert!(spectral.concentration >= 1 && spectral.concentration <= 32);
        assert_ne!(spectral.channels[0], 0);
    }

    #[test]
    fn test_200_ticks_determinism() {
        let mut field = PentaractField::quiet_at(test_point());
        field[SenseChannel::HeatGradient] = 8_000;
        field[SenseChannel::VitalityLux] = 4_000;

        let decay_rates = [50u16; 32];
        let vest_caps = [12_000u64; 32];

        let prior = TritArray32::from_base3_index(0);
        let trit_machine = TritStateMachine {
            prior_state: prior,
            threshold: 5,
        };

        let mut hashes = Vec::new();

        for tick in 0..200 {
            let (spec, _) = tick_pentaract_5d_dual_loop(&field, &decay_rates, &vest_caps, &trit_machine);
            let hash = spec.concentration as u64;
            for ch in 0..32 {
                let ch_val = spec.channels[ch] as u64;
                hashes.push(hash.wrapping_add(ch_val.wrapping_mul(tick as u64 + 1)));
            }
        }

        let mut field2 = PentaractField::quiet_at(test_point());
        field2[SenseChannel::HeatGradient] = 8_000;
        field2[SenseChannel::VitalityLux] = 4_000;

        let mut hashes2 = Vec::new();
        for tick in 0..200 {
            let (spec, _) = tick_pentaract_5d_dual_loop(&field2, &decay_rates, &vest_caps, &trit_machine);
            let hash = spec.concentration as u64;
            for ch in 0..32 {
                let ch_val = spec.channels[ch] as u64;
                hashes2.push(hash.wrapping_add(ch_val.wrapping_mul(tick as u64 + 1)));
            }
        }

        assert_eq!(hashes, hashes2, "200 ticks must produce identical hashes");
    }

    #[test]
    fn test_triple_buffer_no_lock() {
        let buf = VestedTripleBuffer::new(VestedLeaky32::new());

        let mut val = VestedLeaky32::new();
        val.decay[0] = 100;
        buf.write_snapshot(val);

        let snapshot1 = buf.read_snapshot();
        let snapshot2 = buf.read_snapshot();

        assert_eq!(snapshot1.decay[0], snapshot2.decay[0], "concurrent reads must see same value");
    }

    #[test]
    fn test_6primitive_energy_axis_accretion() {
        let mut field = PentaractField::quiet_at(test_point());
        field[SenseChannel::HeatGradient] = 5_000;
        field[SenseChannel::UvFlux] = 3_000;

        let decay_rates = [10u16; 32];
        let accretion_rate = 10_000u64;

        let axis = compute_energy_axis(accretion_rate, &field, &decay_rates);
        assert_eq!(axis, 1, "high accretion should yield +1");
    }

    #[test]
    fn test_6primitive_energy_axis_decay() {
        let mut field = PentaractField::quiet_at(test_point());
        field[SenseChannel::HeatGradient] = 1_000;
        field[SenseChannel::UvFlux] = 500;

        let mut decay_rates = [0u16; 32];
        decay_rates[0] = 9_000;
        decay_rates[1] = 8_000;

        let accretion_rate = 100u64;

        let axis = compute_energy_axis(accretion_rate, &field, &decay_rates);
        assert_eq!(axis, -1, "high decay should yield -1");
    }

    #[test]
    fn test_6primitive_energy_axis_equilibrium() {
        let mut field = PentaractField::quiet_at(test_point());
        field[SenseChannel::HeatGradient] = 100;
        field[SenseChannel::UvFlux] = 0;

        let mut decay_rates = [0u16; 32];
        decay_rates[0] = 10_000;
        let accretion_rate = 100u64;

        let axis = compute_energy_axis(accretion_rate, &field, &decay_rates);
        assert_eq!(axis, 0, "equal accretion and decay should yield 0");
    }

    #[test]
    fn test_6primitive_boundary_axis_vesting() {
        let mut vested = VestedLeaky32::new();
        vested.caps[0] = 5_000;
        vested.caps[1] = 3_000;
        for i in 2..32 {
            vested.caps[i] = 0;
        }

        let dissolution_rate = 1_000u64;

        let axis = compute_boundary_axis(&vested, dissolution_rate);
        assert_eq!(axis, 1, "high vesting should yield +1");
    }

    #[test]
    fn test_6primitive_boundary_axis_dissolution() {
        let mut vested = VestedLeaky32::new();
        for i in 0..32 {
            vested.caps[i] = 0;
        }

        let dissolution_rate = 10_000u64;

        let axis = compute_boundary_axis(&vested, dissolution_rate);
        assert_eq!(axis, -1, "high dissolution should yield -1");
    }

    #[test]
    fn test_6primitive_boundary_axis_threshold() {
        let mut vested = VestedLeaky32::new();
        vested.caps[0] = 5_000;
        for i in 1..32 {
            vested.caps[i] = 0;
        }

        let dissolution_rate = 5_000u64;

        let axis = compute_boundary_axis(&vested, dissolution_rate);
        assert_eq!(axis, 0, "equal vesting and dissolution should yield 0");
    }

    #[test]
    fn test_6primitive_distribution_axis_purity() {
        let spectral = AntiShannonSpectral {
            channels: [500u32; 32],
            concentration: 32,
        };

        let axis = compute_distribution_axis(&spectral);
        assert_eq!(axis, -1, "flat distribution should yield -1 (dispersion)");
    }

    #[test]
    fn test_6primitive_distribution_axis_singular() {
        let mut channels = [0u32; 32];
        channels[0] = 10_000;

        let spectral = AntiShannonSpectral {
            channels,
            concentration: 1,
        };

        let axis = compute_distribution_axis(&spectral);
        assert_eq!(axis, 1, "singular peak should yield +1 (purity)");
    }

    #[test]
    fn test_zero_crossing_detection_energy() {
        let prev_state = TriDualityState {
            energy_flow: 1,
            boundary: 0,
            distribution: 0,
        };

        let curr_state = TriDualityState {
            energy_flow: -1,
            boundary: 0,
            distribution: 0,
        };

        let phase_shifted = detect_phase_shifts(&prev_state, &curr_state);
        assert!(phase_shifted, "sign flip on energy_flow should detect phase shift");
    }

    #[test]
    fn test_zero_crossing_detection_boundary() {
        let prev_state = TriDualityState {
            energy_flow: 1,
            boundary: 1,
            distribution: 0,
        };

        let curr_state = TriDualityState {
            energy_flow: 1,
            boundary: -1,
            distribution: 0,
        };

        let phase_shifted = detect_phase_shifts(&prev_state, &curr_state);
        assert!(phase_shifted, "sign flip on boundary should detect phase shift");
    }

    #[test]
    fn test_zero_crossing_detection_distribution() {
        let prev_state = TriDualityState {
            energy_flow: 0,
            boundary: 0,
            distribution: 1,
        };

        let curr_state = TriDualityState {
            energy_flow: 0,
            boundary: 0,
            distribution: -1,
        };

        let phase_shifted = detect_phase_shifts(&prev_state, &curr_state);
        assert!(phase_shifted, "sign flip on distribution should detect phase shift");
    }

    #[test]
    fn test_zero_crossing_no_flip() {
        let prev_state = TriDualityState {
            energy_flow: 1,
            boundary: 1,
            distribution: 1,
        };

        let curr_state = TriDualityState {
            energy_flow: 1,
            boundary: 1,
            distribution: 1,
        };

        let phase_shifted = detect_phase_shifts(&prev_state, &curr_state);
        assert!(!phase_shifted, "same state should not detect phase shift");
    }

    #[test]
    fn test_6primitive_determinism() {
        let mut field = PentaractField::quiet_at(test_point());
        field[SenseChannel::HeatGradient] = 5_000;
        field[SenseChannel::UvFlux] = 3_000;

        let mut decay_rates = [100u16; 32];
        decay_rates[0] = 150;
        decay_rates[1] = 120;

        let mut vest_caps = [10_000u64; 32];
        vest_caps[0] = 15_000;

        let accretion_rate = 5_000u64;
        let dissolution_rate = 3_000u64;

        let prior = TritArray32::from_base3_index(12345);
        let trit_machine = TritStateMachine {
            prior_state: prior,
            threshold: 10,
        };

        let (state1, _trit1, phase1) =
            tick_6primitive_closed_loop(&field, &decay_rates, &vest_caps, accretion_rate, dissolution_rate, &trit_machine);

        let (state2, _trit2, phase2) =
            tick_6primitive_closed_loop(&field, &decay_rates, &vest_caps, accretion_rate, dissolution_rate, &trit_machine);

        assert_eq!(state1.energy_flow, state2.energy_flow, "energy_flow must be deterministic");
        assert_eq!(state1.boundary, state2.boundary, "boundary must be deterministic");
        assert_eq!(state1.distribution, state2.distribution, "distribution must be deterministic");
        assert_eq!(phase1, phase2, "phase_shifted must be deterministic");
    }

    #[test]
    fn test_6primitive_tri_duality_state_default() {
        let state = TriDualityState::default();
        assert_eq!(state.energy_flow, 0);
        assert_eq!(state.boundary, 0);
        assert_eq!(state.distribution, 0);
    }

    #[test]
    fn test_6primitive_phase_shift_triggers_transition() {
        let mut field = PentaractField::quiet_at(test_point());
        field[SenseChannel::HeatGradient] = 10_000;
        field[SenseChannel::UvFlux] = 5_000;

        let mut decay_rates = [5_000u16; 32];
        decay_rates[0] = 8_000;

        let vest_caps = [1_000u64; 32];
        let accretion_rate = 100u64;
        let dissolution_rate = 20_000u64;

        let prior = TritArray32::from_base3_index(100);
        let trit_machine = TritStateMachine {
            prior_state: prior,
            threshold: 10,
        };

        let (state, _next_trit, phase_shifted) =
            tick_6primitive_closed_loop(&field, &decay_rates, &vest_caps, accretion_rate, dissolution_rate, &trit_machine);

        if phase_shifted {
            assert_ne!(state.energy_flow, 0);
        }
    }
}
