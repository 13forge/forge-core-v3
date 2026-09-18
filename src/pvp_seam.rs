//! PVP relationship seam: one [`Pexil`] per (attacker, defender) pairing.
//!
//! Born from the 2026-08-19 "does home advantage force players to seek it
//! out" design thread (`docs/ADR-pvp-two-clock-seam.md`). Reuses
//! [`atom::Pexil`](crate::atom::Pexil) verbatim rather than inventing a
//! parallel 8-byte struct: the lattice's 5 trit lanes are exactly enough for
//! Advantage/Deathscar/Mercy/Presence + one reserved lane, and `payload`
//! already carries the sub-trit decay/dwell counters a 3-state trit cannot
//! hold on its own (revascularize over net-new, CLAUDE.md T3).
//!
//! TWO CLOCKS, NEVER MERGED (twin of `arch::DetClock`/`CreativeClock`,
//! ARCH-009 "two drums"): [`decay_tick`] ADVANCES the lattice — deterministic,
//! replayable, Drum-1, takes no wall-clock input. [`apply_presence`] and
//! [`apply_loss`] are the only two doors from a real observed event into the
//! lattice — each takes an explicit event value the caller can only construct
//! from something it actually witnessed. There is no `From<Duration>`, no
//! raw field write, no third door. That is the seam.
//!
//! Abuse-resistance is load-bearing, not incidental (Sean 2026-08-19: "think
//! about that 21-year-old with too much summer break who wants to break it —
//! because they will"):
//! - **Touch-and-go ping**: [`apply_presence`] only moves the Advantage lane
//!   after [`MIN_DWELL_PULSES`] of *sustained* presence — a single
//!   connect/disconnect (bot heartbeat, alt-tab visit) never accrues enough
//!   dwell to matter.
//! - **Passive banking**: [`decay_tick`] erodes Advantage every pulse,
//!   unconditionally, both directions equally — camping preserves nothing.
//! - **Farming the same weak opponent all summer**: [`apply_loss`] opens a
//!   Mercy window on the pairing; a second loss inside that window is a
//!   no-op, so repeat-killing one victim stops compounding.

use crate::atom::{CellOrdinal, Pexil, TritCell5D, ValidityMask};

/// Lane indices into the 5-trit lattice. Order is load-bearing (matches
/// [`TritCell5D::trits`]'s `[i8; 5]` positional order) — never reorder.
pub const LANE_ADVANTAGE: usize = 0;
/// Structural memory of a loss — decays far slower than Advantage.
pub const LANE_DEATHSCAR: usize = 1;
/// Grace window blocking a repeat Deathscar on the same pairing.
pub const LANE_MERCY: usize = 2;
/// Who was observed present most recently: `+1` home, `-1` away, `0` neither yet.
pub const LANE_PRESENCE: usize = 3;
/// Ethics tier / future ledger lane — untouched by this module.
pub const LANE_RESERVED_ETHICS: usize = 4;

/// `payload[0]` — pulses remaining until the next Advantage decay step.
const PAYLOAD_ADVANTAGE_COOLDOWN: usize = 0;
/// `payload[1]` — pulses remaining on the Deathscar's slower decay.
const PAYLOAD_DEATHSCAR_COOLDOWN: usize = 1;
/// `payload[2]` — pulses remaining on an active Mercy grace window.
const PAYLOAD_MERCY_COOLDOWN: usize = 2;
/// `payload[3]` — dwell pulses accumulated at the current Presence state.
const PAYLOAD_DWELL: usize = 3;

/// Decay-pulse cadence is the caller's choice (e.g. once per real second of
/// `SimTick`s) — these constants are counted in pulses, not raw ticks, so
/// they fit `u8` regardless of tick rate.
///
/// Pulses for Advantage to step once. Deliberately fast: a lead decays back
/// to neutral in a handful of pulses if nobody refreshes it.
pub const ADVANTAGE_DECAY_PULSES: u8 = 2;
/// Pulses for a Deathscar to clear. Far slower than Advantage — a loss is
/// structural memory, not a one-pulse blip.
pub const DEATHSCAR_DECAY_PULSES: u8 = 60;
/// Sustained dwell pulses required before a presence pulse may move
/// Advantage at all.
pub const MIN_DWELL_PULSES: u8 = 5;
/// Pulses a Mercy window stays open, blocking a new Deathscar on the same
/// pairing.
pub const MERCY_WINDOW_PULSES: u8 = 120;

/// Fresh, untouched pairing: origin lattice, nothing yet observed.
/// [`ValidityMask::ALL_UNKNOWN`] marks every lane unresolved — an untouched
/// pairing is not the same as one that settled at neutral through real decay.
pub fn origin(pair: CellOrdinal) -> Pexil {
    Pexil { lattice: TritCell5D::ORIGIN, validity: ValidityMask::ALL_UNKNOWN, ordinal: pair, payload: [0; 4] }
}

/// Drum-1: advance the lattice by one decay pulse. Deterministic, replayable,
/// no wall-clock input — call this from the sim's own tick loop, never from
/// an I/O callback.
pub fn decay_tick(mut cell: Pexil) -> Pexil {
    let mut trits = cell.lattice.trits().unwrap_or([0; 5]);

    trits[LANE_ADVANTAGE] =
        step_toward_zero(trits[LANE_ADVANTAGE], &mut cell.payload[PAYLOAD_ADVANTAGE_COOLDOWN], ADVANTAGE_DECAY_PULSES);
    trits[LANE_DEATHSCAR] =
        step_toward_zero(trits[LANE_DEATHSCAR], &mut cell.payload[PAYLOAD_DEATHSCAR_COOLDOWN], DEATHSCAR_DECAY_PULSES);

    // Mercy does not fade gradually — it is live or it is over.
    if trits[LANE_MERCY] != 0 {
        if cell.payload[PAYLOAD_MERCY_COOLDOWN] <= 1 {
            trits[LANE_MERCY] = 0;
            cell.payload[PAYLOAD_MERCY_COOLDOWN] = 0;
        } else {
            cell.payload[PAYLOAD_MERCY_COOLDOWN] -= 1;
        }
    }

    cell.lattice = TritCell5D::from_trits(trits);
    cell
}

/// One symmetric decay step: counts a per-lane cooldown down; when it hits
/// zero, nudges `trit` one step toward `0` and resets the cooldown. A trit
/// already at `0` has nothing to decay.
fn step_toward_zero(trit: i8, cooldown: &mut u8, period: u8) -> i8 {
    if trit == 0 {
        *cooldown = 0;
        return 0;
    }
    if *cooldown == 0 {
        *cooldown = period;
    }
    *cooldown -= 1;
    if *cooldown == 0 {
        trit - trit.signum()
    } else {
        trit
    }
}

/// Nudge `trit` one step toward `target`. Never overshoots — `target` is
/// always in `-1..=1`, same as `trit`.
fn step_toward(trit: i8, target: i8) -> i8 {
    if trit < target {
        trit + 1
    } else if trit > target {
        trit - 1
    } else {
        trit
    }
}

/// A real, observed presence event — the only way Advantage can move toward
/// whoever is present.
pub struct PresencePulse {
    /// `true` if this pulse observed the pairing's home player physically
    /// present at home right now; `false` for the away player present away.
    pub at_home: bool,
}

/// Drum-2 door #1. Updates the Presence lane immediately (it IS the observed
/// fact), but only nudges Advantage once [`MIN_DWELL_PULSES`] of sustained
/// presence has accrued — defeats the touch-and-go ping exploit.
pub fn apply_presence(mut cell: Pexil, pulse: PresencePulse) -> Pexil {
    let mut trits = cell.lattice.trits().unwrap_or([0; 5]);
    let want = if pulse.at_home { 1i8 } else { -1i8 };

    if trits[LANE_PRESENCE] == want {
        cell.payload[PAYLOAD_DWELL] = cell.payload[PAYLOAD_DWELL].saturating_add(1);
    } else {
        trits[LANE_PRESENCE] = want;
        cell.payload[PAYLOAD_DWELL] = 1;
    }

    if cell.payload[PAYLOAD_DWELL] >= MIN_DWELL_PULSES {
        trits[LANE_ADVANTAGE] = step_toward(trits[LANE_ADVANTAGE], want);
        cell.payload[PAYLOAD_DWELL] = 0;
    }

    cell.lattice = TritCell5D::from_trits(trits);
    cell
}

/// A real, observed loss event.
pub struct LossEvent {
    /// `true` if the loser was the pairing's home player.
    pub loser_at_home: bool,
}

/// Drum-2 door #2. Pins a Deathscar toward whichever side benefited and
/// opens a Mercy window on the pairing — unless a Mercy window from a prior
/// loss is still active, in which case this call is a no-op (defeats
/// repeat-farming the same opponent).
pub fn apply_loss(mut cell: Pexil, event: LossEvent) -> Pexil {
    let mut trits = cell.lattice.trits().unwrap_or([0; 5]);

    if trits[LANE_MERCY] != 0 {
        return cell;
    }

    trits[LANE_DEATHSCAR] = if event.loser_at_home { -1 } else { 1 };
    cell.payload[PAYLOAD_DEATHSCAR_COOLDOWN] = DEATHSCAR_DECAY_PULSES;
    trits[LANE_MERCY] = 1;
    cell.payload[PAYLOAD_MERCY_COOLDOWN] = MERCY_WINDOW_PULSES;

    cell.lattice = TritCell5D::from_trits(trits);
    cell
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pulses_to_zero(mut trit_lane_at: i8, period: u8) -> u32 {
        let mut cooldown = 0u8;
        let mut n = 0u32;
        while trit_lane_at != 0 {
            trit_lane_at = step_toward_zero(trit_lane_at, &mut cooldown, period);
            n += 1;
        }
        n
    }

    #[test]
    fn decay_is_symmetric_both_directions() {
        assert_eq!(
            pulses_to_zero(1, ADVANTAGE_DECAY_PULSES),
            pulses_to_zero(-1, ADVANTAGE_DECAY_PULSES),
            "camping a home lead must erode exactly as fast as an away deficit recovers"
        );
    }

    #[test]
    fn deathscar_outlasts_advantage_decay() {
        assert!(
            DEATHSCAR_DECAY_PULSES > ADVANTAGE_DECAY_PULSES,
            "a loss must be structural memory, not a one-pulse Advantage blip"
        );
    }

    #[test]
    fn touch_and_go_does_not_move_advantage() {
        let cell = origin(CellOrdinal(0));
        let after = apply_presence(cell, PresencePulse { at_home: true });
        assert_eq!(after.lattice.trits().unwrap()[LANE_ADVANTAGE], 0, "one pulse is not sustained dwell");
    }

    #[test]
    fn sustained_presence_moves_advantage_after_min_dwell() {
        let mut cell = origin(CellOrdinal(0));
        for _ in 0..MIN_DWELL_PULSES {
            cell = apply_presence(cell, PresencePulse { at_home: true });
        }
        assert_eq!(cell.lattice.trits().unwrap()[LANE_ADVANTAGE], 1);
    }

    #[test]
    fn mercy_blocks_a_second_deathscar() {
        let cell = origin(CellOrdinal(0));
        let once = apply_loss(cell, LossEvent { loser_at_home: true });
        let twice = apply_loss(once, LossEvent { loser_at_home: true });
        assert_eq!(once.lattice, twice.lattice, "a second loss inside the Mercy window must be a no-op");
        assert_eq!(once.payload, twice.payload, "the Mercy cooldown must not refresh on the blocked call");
    }

    #[test]
    fn fold_mirrors_the_pairing_for_the_other_frame() {
        let mut trits = [0i8; 5];
        trits[LANE_ADVANTAGE] = 1;
        let cell_lattice = TritCell5D::from_trits(trits);
        let mirrored = cell_lattice.fold().unwrap();
        assert_eq!(mirrored.trits().unwrap()[LANE_ADVANTAGE], -1, "one number, two frames — home's +1 is away's -1");
    }
}
