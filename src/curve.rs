//! The climb — integer-only XP curve, level cap, and unbounded ascension past it.
//! No floats, no wall clock: `standing(total_xp)` is a pure function of one u64.
//! Ported 2026-09-13 verbatim from `F:\NewRepo\crates\forge-insights\src\rpg\curve.rs`
//! (v2 "The Long Toll"); only the `Standing` field docs are new (this crate denies
//! missing_docs). ONE HOME (L05): forge-insights-v3 and D:'s ironroot-core read this.

/// Last level before an ascension resets the ladder.
pub const LEVEL_CAP: u32 = 99;

/// Ascension surcharge, percent added per prior ascension.
pub const ASCEND_STEP_PCT: u64 = 25;

/// XP to cross from `level` into `level + 1`, before any ascension surcharge.
///
/// Superlinear so the tail is long: level 1 costs 733, level 98 costs 1_339_333.
/// The whole 1..99 climb totals [`TOTAL_XP_TO_CAP`], which the pacing test pins.
pub fn xp_to_next(level: u32) -> u64 {
    let l = level.max(1) as u64;
    400 * l * l / 3 + 600 * l
}

/// The surcharged cost of `level` for someone who has ascended `ascensions` times.
pub fn xp_to_next_at(level: u32, ascensions: u32) -> u64 {
    let pct = 100 + ASCEND_STEP_PCT * ascensions as u64;
    xp_to_next(level) / 100 * pct + xp_to_next(level) % 100 * pct / 100
}

/// Sum of every step from level 1 to `level`, first ascension only.
pub fn total_xp_for_level(level: u32) -> u64 {
    (1..level.min(LEVEL_CAP)).map(xp_to_next).sum()
}

/// XP for one whole ladder at `ascensions` — what the next reset will cost.
pub fn ladder_cost(ascensions: u32) -> u64 {
    (1..LEVEL_CAP).map(|l| xp_to_next_at(l, ascensions)).sum()
}

/// Where a lifetime XP total lands: level, ascensions, and progress into the level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Standing {
    /// Current level, 1..=[`LEVEL_CAP`].
    pub level: u32,
    /// Whole ladders climbed and reset.
    pub ascensions: u32,
    /// XP spent inside the current level.
    pub xp_into_level: u64,
    /// XP the current level costs to leave, at this ascension.
    pub xp_to_next: u64,
    /// The lifetime total this standing was folded from.
    pub total_xp: u64,
}

impl Standing {
    /// Progress through the current level, permyriad 0-10000.
    pub fn progress_permyriad(&self) -> u16 {
        if self.xp_to_next == 0 {
            return 10_000;
        }
        ((self.xp_into_level.min(self.xp_to_next) * 10_000) / self.xp_to_next) as u16
    }
}

/// Fold a lifetime XP total into a [`Standing`]. Pure, total, and monotonic in `total_xp`.
pub fn standing(total_xp: u64) -> Standing {
    let mut rem = total_xp;
    let mut ascensions = 0u32;
    loop {
        let cost = ladder_cost(ascensions);
        if rem < cost {
            break;
        }
        rem -= cost;
        ascensions += 1;
    }
    let mut level = 1u32;
    while level < LEVEL_CAP {
        let step = xp_to_next_at(level, ascensions);
        if rem < step {
            break;
        }
        rem -= step;
        level += 1;
    }
    Standing {
        level,
        ascensions,
        xp_into_level: rem,
        xp_to_next: xp_to_next_at(level, ascensions),
        total_xp,
    }
}

/// XP paid for one harness beat: board quality scaled by the streak and cut by debt.
///
/// `quality` is the beat's permyriad (green fraction of the board — a FAIL scores 0),
/// `mult_pct` is the streak multiplier, `curse_pct` the tech-debt malus. Integer only,
/// so a dead board pays nothing no matter how long the streak is.
pub fn beat_xp(quality: u16, mult_pct: u32, curse_pct: u32) -> u64 {
    let base = quality as u64 / 100;
    let boosted = base * mult_pct.max(100) as u64 / 100;
    boosted * (100 - curse_pct.min(90)) as u64 / 100
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xp_to_next_is_strictly_increasing_across_the_whole_ladder() {
        for l in 1..LEVEL_CAP - 1 {
            assert!(xp_to_next(l) < xp_to_next(l + 1), "step {l} must cost less than {}", l + 1);
        }
    }

    #[test]
    fn standing_round_trips_every_level_boundary() {
        for l in 1..LEVEL_CAP {
            let at = total_xp_for_level(l);
            let s = standing(at);
            assert_eq!(s.level, l, "total_xp_for_level({l}) must land on level {l}");
            assert_eq!(s.ascensions, 0);
            assert_eq!(s.xp_into_level, 0);
        }
    }

    #[test]
    fn standing_is_monotonic_and_never_skips_a_level() {
        // Step smaller than the cheapest level (733) so a skip is a real defect,
        // not an artefact of sampling coarser than the ladder.
        let mut prev = standing(0);
        for xp in (0..total_xp_for_level(20)).step_by(101) {
            let s = standing(xp);
            assert!(s.level >= prev.level, "level went backwards at {xp}");
            assert!(s.level <= prev.level + 1, "level skipped at {xp}");
            prev = s;
        }
    }

    #[test]
    fn ascension_resets_the_level_and_raises_the_next_ladder() {
        let one = ladder_cost(0);
        let s = standing(one);
        assert_eq!(s.ascensions, 1, "one full ladder is exactly one ascension");
        assert_eq!(s.level, 1, "ascension drops you back to level 1");
        assert!(ladder_cost(1) > one, "the second ladder must cost more than the first");
        assert!(ladder_cost(4) > ladder_cost(3), "the surcharge keeps climbing — there is no top");
    }

    #[test]
    fn a_dead_board_pays_nothing_no_matter_the_streak() {
        assert_eq!(beat_xp(0, 300, 0), 0, "FAIL scores 0 quality, so it must score 0 xp");
        assert_eq!(beat_xp(99, 300, 0), 0, "under one percent green is still nothing");
    }

    #[test]
    fn the_climb_to_cap_takes_years_of_real_beats() {
        // 217/225 green = 9644 permyriad, the board's live figure; a realistic mean
        // streak multiplier of 1.5x; no debt. This is the pacing receipt.
        let per_beat = beat_xp(9_644, 150, 0);
        let beats = total_xp_for_level(LEVEL_CAP) / per_beat;
        // 60s harness interval, 8 working hours a day = 480 beats a day.
        let days = beats / 480;
        assert!(days > 600, "level 99 must be more than 600 days of real work, got {days}");
        assert!(days < 1_500, "and it must be reachable — got {days} days");
    }

    #[test]
    fn debt_cuts_the_take_but_never_all_of_it() {
        let clean = beat_xp(10_000, 100, 0);
        assert!(beat_xp(10_000, 100, 20) < clean, "debt must bite");
        assert!(beat_xp(10_000, 100, 100) > 0, "but it can never zero a green board");
    }

    /// Act 1 stops at 20: the number D:'s pacing is tuned against, from the fn, not a literal.
    #[test]
    fn act_one_reaches_twenty_at_the_summed_steps() {
        assert_eq!(total_xp_for_level(20), (1..20u32).map(xp_to_next).sum::<u64>());
        assert_eq!(standing(total_xp_for_level(20)).level, 20);
    }
}
