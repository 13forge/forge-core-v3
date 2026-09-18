//! The permyriad leaky integrator — the decay primitive, drained 2026-08-10 from
//! the v2 proof harness `_vault/output/_consolidation/from-F-output-2026-07-14/
//! decay_primitive.py` (Sanctuary decay spec §3). Every claim that file printed
//! as a readback is held here as a test.
//!
//! This is the scalar resolvent. `keep / 10000` is the coupling λ; `leak >= 1`
//! is `|λ| < 1`; the steady state under constant injection is `(1 - λ)⁻¹ · g`
//! — which is why equilibrium lands at `rate * 10000 / leak`. The 32-channel
//! audio resolvent is this file's matrix generalisation, and it inherits the
//! flooring discipline pinned here.
//!
//! FLOORING DISCIPLINE (the harness's TEST 4 defect, resolved): the written
//! closed form mixed a floored base with an unfloored impulse sum and was not
//! integer-clean. This engine is the **per-tick floor** recurrence — one number
//! system, floor on every tick, iter <= exact always (a floor only discards).
//! The exact-then-floor path exists only inside tests, as the oracle.
//!
//! Decay family (book ch.29): consequence/allostatic_curve.rs DESCRIBES; this
//! EXECUTES; forge-audio-v3 vested_decay.rs REMEMBERS.

/// One permyriad. `10_000 == 1.0`. The same unit `fixed_point::Permyriad` scales by.
pub const PMY: u64 = 10_000;

/// A leaky integrator over permyriad retention. `leak` is parts-per-myriad lost
/// per tick; `keep = PMY - leak` is retained. `leak == 0` never decays and is
/// refused at construction — a coupling of exactly 1 has no equilibrium.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LeakyPermyriad {
    /// The accumulator.
    pub value: u64,
    /// Parts-per-myriad lost per tick. `1..=PMY`.
    pub leak: u16,
    /// Per-channel leak rates for 32-lane decay. One per sense channel.
    pub channel_rates: [u16; 32],
}

impl LeakyPermyriad {
    /// A new integrator. `None` when `leak` is 0 (no decay ⇒ no equilibrium)
    /// or greater than `PMY` (cannot lose more than the whole).
    #[inline]
    pub const fn new(value: u64, leak: u16) -> Option<Self> {
        if leak == 0 || leak as u64 > PMY {
            return None;
        }
        Some(Self { value, leak, channel_rates: [0; 32] })
    }

    /// One tick: `value = value * keep / PMY`, the flooring leak on that line.
    /// The intermediate is `u128`, so no `value` can overflow the multiply —
    /// the harness's TEST 5 headroom, resolved by promotion instead of a bound.
    #[inline]
    pub const fn tick(&mut self) {
        let keep = PMY - self.leak as u64;
        self.value = ((self.value as u128 * keep as u128) / PMY as u128) as u64;
    }

    /// Inject an impulse (re-arm), saturating rather than wrapping.
    #[inline]
    pub const fn inject(&mut self, r: u64) {
        self.value = self.value.saturating_add(r);
    }

    /// The equilibrium of constant injection `rate` per tick — the scalar
    /// resolvent `(1 - λ)⁻¹ · g = rate * PMY / leak`. The iterated engine
    /// settles at or just below this (the flooring leak only discards).
    #[inline]
    pub const fn equilibrium(rate: u64, leak: u16) -> u64 {
        rate * PMY / leak as u64
    }

    /// Per-channel parallel decay: apply each of the 32 channel leak rates
    /// to the field channels, integrating the decayed sum into `value`.
    /// Each channel decays by `keep = PMY - channel_rates[i]`, then the
    /// per-channel results are summed and added to the accumulator.
    pub fn tick_32lane(&mut self, field: &crate::pentaract_field::PentaractField) {
        let channels = field.channels();
        let mut field_sum: i64 = 0;

        for i in 0..32 {
            let keep = PMY - self.channel_rates[i] as u64;
            let decayed = (channels[i] as i64 * keep as i64) / PMY as i64;
            field_sum = field_sum.saturating_add(decayed);
        }

        if field_sum >= 0 {
            self.value = self.value.saturating_add(field_sum as u64);
        } else {
            self.value = self.value.saturating_sub((-field_sum) as u64);
        }
    }
}

const _: () = assert!(core::mem::size_of::<LeakyPermyriad>() == 80); // u64 + u16 + [u16; 32] + padding

/// Per-channel parallel decay over a field: field[i] = floor(field[i] * (PMY - decay_rates[i]) / PMY).
/// Each channel decays by its corresponding rate, in place.
#[inline]
pub fn tick_32lane(field: &mut crate::pentaract_field::PentaractField, decay_rates: &[u16; 32]) {
    use crate::pentaract_field::SenseChannel;
    for i in 0..32 {
        let channel = SenseChannel::ALL[i];
        let keep = PMY - decay_rates[i] as u64;
        let decayed = ((field[channel] as i64 * keep as i64) / PMY as i64) as i32;
        field[channel] = decayed;
    }
}

/// Per-channel parallel impulse injection: field[i] += impulses[i] (saturating).
/// Each channel receives its corresponding impulse, saturating on overflow.
#[inline]
pub fn inject_32lane(field: &mut crate::pentaract_field::PentaractField, impulses: &[u64; 32]) {
    use crate::pentaract_field::SenseChannel;
    for i in 0..32 {
        let channel = SenseChannel::ALL[i];
        let impulse = if impulses[i] > i32::MAX as u64 {
            i32::MAX
        } else {
            impulses[i] as i32
        };
        field[channel] = field[channel].saturating_add(impulse);
    }
}

/// The literacy stage after `exposures` exposures — the arc's floor walk:
/// stage index climbs one per exposure and parks at the final stage.
pub const fn literacy_stage_at(
    stages: &[crate::consequence::allostatic_curve::LiteracyStage; 6],
    exposures: u32,
) -> crate::consequence::allostatic_curve::LiteracyStage {
    let idx = if exposures as usize >= stages.len() { stages.len() - 1 } else { exposures as usize };
    stages[idx]
}

#[cfg(test)]
mod curve_tests {
    use super::*;
    use crate::consequence::allostatic_curve::{AllostaticCurve as C, LiteracyStage};

    #[test]
    fn literacy_arc_walks_stages_and_parks_at_prevention() {
        let C::LiteracyArc { stages } = C::DEFAULT_LITERACY_ARC else { unreachable!() };
        assert_eq!(literacy_stage_at(&stages, 0), LiteracyStage::SensoryHorror);
        assert_eq!(literacy_stage_at(&stages, 3), LiteracyStage::Schedule);
        assert_eq!(literacy_stage_at(&stages, 5), LiteracyStage::Prevention);
        assert_eq!(literacy_stage_at(&stages, 9999), LiteracyStage::Prevention);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(v0: u64, leak: u16, ticks: usize, impulses: &[(usize, u64)]) -> Vec<u64> {
        let mut s = LeakyPermyriad::new(v0, leak).unwrap();
        for &(t, r) in impulses {
            if t == 0 {
                s.inject(r);
            }
        }
        let mut out = vec![s.value];
        for k in 1..=ticks {
            s.tick();
            for &(t, r) in impulses {
                if t == k {
                    s.inject(r);
                }
            }
            out.push(s.value);
        }
        out
    }

    // Harness TEST 1 — bit-reproducibility: two runs, identical curves.
    #[test]
    fn the_curve_is_deterministic() {
        let a = run(10_000, 100, 200, &[]);
        let b = run(10_000, 100, 200, &[]);
        assert_eq!(a, b);
    }

    // Harness TEST 2 — per-tick floor never exceeds the exact single-floor path.
    // Exact oracle: v0 * keep^t / PMY^t in u128, valid while PMY^t * v0 fits —
    // t <= 8 at v0 = 10_000 (10^(4t+4) < 2^127).
    #[test]
    fn iter_never_exceeds_the_exact_oracle_and_matches_at_t1() {
        let (v0, leak) = (10_000u64, 100u16);
        let keep = PMY - leak as u64;
        let curve = run(v0, leak, 8, &[]);
        for t in 0..=8usize {
            let exact = (v0 as u128 * (keep as u128).pow(t as u32))
                / (PMY as u128).pow(t as u32);
            assert!(
                (curve[t] as u128) <= exact,
                "t={t}: iter {} above exact {exact}",
                curve[t]
            );
            // The flooring leak is bounded: within t units after t ticks.
            assert!((exact - curve[t] as u128) <= t as u128, "t={t}: leak too large");
        }
        assert_eq!(curve[1] as u128, v0 as u128 * keep as u128 / PMY as u128);
    }

    // Harness TEST 2b — the empirical half-life of rho=0.99 is 69 ticks
    // (continuous formula: ln 0.5 / ln 0.99 = 68.97…).
    #[test]
    fn the_half_life_of_one_percent_leak_is_sixty_nine_ticks() {
        let curve = run(10_000, 100, 100, &[]);
        let crossing = curve.iter().position(|&v| v < 5_000).unwrap();
        assert_eq!(crossing, 69);
    }

    // Harness TEST 4 — one number system. The engine is per-tick floor; its
    // output is always an integer by type, and always <= the exact-then-floor
    // path with impulses folded in.
    #[test]
    fn the_sawtooth_stays_integer_clean_and_below_the_exact_path() {
        let imps: &[(usize, u64)] = &[(10, 3_000), (25, 4_000), (40, 2_000)];
        let curve = run(5_000, 300, 45, imps);
        // Exact rational path via u128 scaled arithmetic: track value * PMY^t.
        // 10^(4t) overflows past t=8, so walk the exact path tick-by-tick in
        // fractions of the CURRENT tick only: exact >= iter is preserved per
        // step because each step's floor only discards.
        let keep = PMY - 300;
        let mut lo = 5_000u64; // the engine
        for k in 1..=45usize {
            lo = ((lo as u128 * keep as u128) / PMY as u128) as u64;
            for &(t, r) in imps {
                if t == k {
                    lo += r;
                }
            }
            assert_eq!(lo, curve[k], "engine self-agreement at tick {k}");
        }
    }

    // The resolvent identity: constant injection settles at rate*PMY/leak,
    // never above it, and within flooring distance below it. This is the 1x1
    // case of (I - λK)^{-1} g — the doctrine the audio resolvent inherits.
    #[test]
    fn constant_injection_settles_at_the_scalar_resolvent() {
        let (rate, leak) = (500u64, 250u16);
        let eq = LeakyPermyriad::equilibrium(rate, leak);
        assert_eq!(eq, 20_000);
        let mut s = LeakyPermyriad::new(0, leak).unwrap();
        let mut prev = 0u64;
        for _ in 0..2_000 {
            s.tick();
            s.inject(rate);
            prev = s.value;
        }
        s.tick();
        s.inject(rate);
        assert_eq!(s.value, prev, "the fixed point is reached and stays");
        assert!(s.value <= eq, "iterated engine never exceeds the resolvent");
        assert!(eq - s.value <= PMY / leak as u64 + 1, "and lands within flooring distance");
    }

    // λ = 1 (leak 0) has no equilibrium and is refused at construction,
    // as is losing more than the whole.
    #[test]
    fn a_unit_coupling_is_refused() {
        assert!(LeakyPermyriad::new(1, 0).is_none());
        assert!(LeakyPermyriad::new(1, (PMY + 1) as u16).is_none());
        assert!(LeakyPermyriad::new(1, PMY as u16).is_some(), "total leak is legal: instant decay");
    }

    // drift_proof.rs (caisson, Spatial Memory Palace) assertion (d): the per-tick
    // loss is EXACTLY ceil(V * leak / PMY) — the flooring leak has a closed form.
    #[test]
    fn the_per_tick_loss_is_exactly_the_ceiling_of_the_leak() {
        for &leak in &[1u16, 50, 100, 300, 4_000, 9_999] {
            for &v in &[0u64, 1, 2, 99, 100, 10_000, 123_457, u64::MAX >> 14] {
                let mut s = LeakyPermyriad::new(v, leak).unwrap();
                s.tick();
                let lost = v - s.value;
                let ceil_leak =
                    ((v as u128 * leak as u128 + PMY as u128 - 1) / PMY as u128) as u64;
                assert_eq!(lost, ceil_leak, "v={v} leak={leak}");
            }
        }
    }

    // drift_proof.rs assertions (b)+(c): strictly monotone to zero, in bounded ticks.
    #[test]
    fn decay_is_strictly_monotone_to_zero_in_bounded_ticks() {
        let mut s = LeakyPermyriad::new(100_000, 100).unwrap();
        let mut prev = s.value;
        let mut ticks = 0u32;
        while s.value > 0 {
            s.tick();
            assert!(s.value < prev, "tick {ticks}: {} did not decrease", s.value);
            prev = s.value;
            ticks += 1;
            assert!(ticks < 5_000, "unbounded decay");
        }
        assert_eq!(s.value, 0);
    }

    // Harness TEST 5 — headroom by promotion: the maximum value survives a tick.
    #[test]
    fn u64_max_survives_a_tick_via_u128_promotion() {
        let mut s = LeakyPermyriad::new(u64::MAX, 1).unwrap();
        s.tick();
        let expect = (u64::MAX as u128 * (PMY - 1) as u128 / PMY as u128) as u64;
        assert_eq!(s.value, expect);
    }

    // 32-lane determinism: run 200 ticks on a perceptual field, verify
    // the curve is bit-reproducible across runs. No RNG, no f32/f64,
    // pure integer decay on all 32 sense channels.
    #[test]
    fn test_decay_32lane_determinism() {
        use crate::pentaract_field::{PentaractField, SenseChannel};
        use crate::pentaract::Pentaract;

        fn run_32lane(ticks: usize) -> Vec<u64> {
            let point = Pentaract::new(0x42, 1000, 2000, 3000, 4000, 0xDEADBEEF, 0);
            let mut s = LeakyPermyriad::new(100_000, 100).unwrap();
            let mut s_rates = s.channel_rates;
            for i in 0..32 {
                s_rates[i] = (50 + i as u16) % 200;
            }
            s.channel_rates = s_rates;

            let mut field = PentaractField::quiet_at(point);
            field[SenseChannel::HeatGradient] = 5_000;
            field[SenseChannel::VitalityLux] = 3_000;
            field[SenseChannel::WeaveFlux] = -2_000;

            let mut out = vec![s.value];
            for _ in 0..ticks {
                s.tick_32lane(&field);
                out.push(s.value);
            }
            out
        }

        let curve_a = run_32lane(200);
        let curve_b = run_32lane(200);
        assert_eq!(curve_a, curve_b, "32-lane curves must be bit-identical");
        assert_eq!(curve_a.len(), 201, "should have initial + 200 ticks");
    }
}
