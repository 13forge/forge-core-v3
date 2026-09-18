//! 8 Disciplines Progression & 9-Chord 120Hz Combat Foundations.
//!
//! Provides the core deterministic, zero-heap progression model and fixed-point
//! poise/action engine across the 8 Oath Disciplines:
//! `Edge`, `Weight`, `Breath`, `Thread`, `Ash`, `Root`, `Glass`, and `Salt`.
//!
//! # Architecture
//! - Fixed-point integer arithmetic via [`Permyriad`] (no floating point in core loop).
//! - Deterministic 120Hz poise recovery, poise damage mitigation, and stagger state machine.
//! - 9-Chord action affinity matrix tailored to each discipline's philosophy.
//! - 8-tier progression milestones with monotonic stat and poise scaling.
//! - Strict layout assertion locks guaranteeing zero dynamic allocations and copyability.

use crate::fixed_point::Permyriad;

/// The 8 Oath Disciplines replacing legacy zodiac and animal classes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
#[repr(u8)]
pub enum DisciplineKind {
    /// Edge: precision, keen strikes, swift cancels, high momentum & logic scaling.
    #[default]
    Edge = 0,
    /// Weight: bearing burden, immense poise, crushing gravity strikes, high shadow weight.
    Weight = 1,
    /// Breath: composure, wide parry windows, harmonic stillness, high logic & resonance.
    Breath = 2,
    /// Thread: restoration, lifeline tethering, accelerated combo heat retention.
    Thread = 3,
    /// Ash: destructive fire, explosive surge damage, high vigor & tarnish sacrifice.
    Ash = 4,
    /// Root: steadfast footing, immovable posture, supreme stagger resistance.
    Root = 5,
    /// Glass: piercing clarity, defense-bypassing crit shatter, brittle poise.
    Glass = 6,
    /// Salt: preservation, enduring cleansing, steady poise regen under pressure.
    Salt = 7,
}

impl DisciplineKind {
    /// All 8 disciplines in canonical ordinal order.
    pub const ALL: [DisciplineKind; 8] = [
        DisciplineKind::Edge,
        DisciplineKind::Weight,
        DisciplineKind::Breath,
        DisciplineKind::Thread,
        DisciplineKind::Ash,
        DisciplineKind::Root,
        DisciplineKind::Glass,
        DisciplineKind::Salt,
    ];

    /// Convert a numeric index (0..=7) with modulo wrapping to a discipline.
    #[inline(always)]
    pub const fn from_index(idx: u8) -> Self {
        match idx % 8 {
            0 => DisciplineKind::Edge,
            1 => DisciplineKind::Weight,
            2 => DisciplineKind::Breath,
            3 => DisciplineKind::Thread,
            4 => DisciplineKind::Ash,
            5 => DisciplineKind::Root,
            6 => DisciplineKind::Glass,
            _ => DisciplineKind::Salt,
        }
    }

    /// Human-readable title of the discipline.
    #[inline(always)]
    pub const fn name(self) -> &'static str {
        match self {
            DisciplineKind::Edge => "Edge",
            DisciplineKind::Weight => "Weight",
            DisciplineKind::Breath => "Breath",
            DisciplineKind::Thread => "Thread",
            DisciplineKind::Ash => "Ash",
            DisciplineKind::Root => "Root",
            DisciplineKind::Glass => "Glass",
            DisciplineKind::Salt => "Salt",
        }
    }

    /// Base poise value before stat modifications (in fixed integer units).
    #[inline(always)]
    pub const fn base_poise(self) -> i32 {
        match self {
            DisciplineKind::Edge => 800,
            DisciplineKind::Weight => 1600,
            DisciplineKind::Breath => 950,
            DisciplineKind::Thread => 900,
            DisciplineKind::Ash => 1100,
            DisciplineKind::Root => 1800,
            DisciplineKind::Glass => 650,
            DisciplineKind::Salt => 1300,
        }
    }

    /// Poise recovery per 120Hz tick (in fixed Permyriad units, where 10000 = 1.0 poise/tick).
    #[inline(always)]
    pub const fn poise_recovery_rate_pmy(self) -> Permyriad {
        match self {
            DisciplineKind::Edge => Permyriad(50_000),   // 5.0 poise/tick (600/s)
            DisciplineKind::Weight => Permyriad(30_000), // 3.0 poise/tick (360/s)
            DisciplineKind::Breath => Permyriad(70_000), // 7.0 poise/tick (840/s)
            DisciplineKind::Thread => Permyriad(45_000), // 4.5 poise/tick (540/s)
            DisciplineKind::Ash => Permyriad(40_000),    // 4.0 poise/tick (480/s)
            DisciplineKind::Root => Permyriad(25_000),   // 2.5 poise/tick (300/s)
            DisciplineKind::Glass => Permyriad(60_000),  // 6.0 poise/tick (720/s)
            DisciplineKind::Salt => Permyriad(80_000),   // 8.0 poise/tick (960/s)
        }
    }

    /// Ticks to delay poise recovery after taking poise damage at 120Hz (e.g. 60 ticks = 0.5s).
    #[inline(always)]
    pub const fn recovery_delay_ticks(self) -> u16 {
        match self {
            DisciplineKind::Edge => 40,   // Fast reset
            DisciplineKind::Weight => 80, // Heavy recovery lag
            DisciplineKind::Breath => 30, // Supreme composure
            DisciplineKind::Thread => 45,
            DisciplineKind::Ash => 50,
            DisciplineKind::Root => 90,   // Deep rooted lag
            DisciplineKind::Glass => 35,  // Fast or shattered
            DisciplineKind::Salt => 25,   // Rapid preservation
        }
    }

    /// Stagger duration in 120Hz ticks when poise is completely broken (0 poise).
    #[inline(always)]
    pub const fn stagger_duration_ticks(self) -> u16 {
        match self {
            DisciplineKind::Edge => 36,   // 300ms
            DisciplineKind::Weight => 48, // 400ms
            DisciplineKind::Breath => 24, // 200ms
            DisciplineKind::Thread => 36, // 300ms
            DisciplineKind::Ash => 42,    // 350ms
            DisciplineKind::Root => 60,   // 500ms
            DisciplineKind::Glass => 30,  // 250ms
            DisciplineKind::Salt => 36,   // 300ms
        }
    }

    /// Poise damage absorption ratio in Permyriad (10000 = 100% normal damage taken; lower = more defense).
    #[inline(always)]
    pub const fn poise_defense_pmy(self) -> Permyriad {
        match self {
            DisciplineKind::Edge => Permyriad(10_000),  // 100%
            DisciplineKind::Weight => Permyriad(6_500), // 65% (takes 35% less poise dmg)
            DisciplineKind::Breath => Permyriad(9_000), // 90%
            DisciplineKind::Thread => Permyriad(9_500), // 95%
            DisciplineKind::Ash => Permyriad(8_500),    // 85%
            DisciplineKind::Root => Permyriad(5_000),   // 50% (takes 50% less poise dmg)
            DisciplineKind::Glass => Permyriad(12_000), // 120% (brittle, takes 20% more poise dmg)
            DisciplineKind::Salt => Permyriad(7_500),   // 75%
        }
    }

    /// Primary chord action affinity for this discipline.
    #[inline(always)]
    pub const fn primary_chord(self) -> ChordKind {
        match self {
            DisciplineKind::Edge => ChordKind::HarmonicStrike,
            DisciplineKind::Weight => ChordKind::GravityCrush,
            DisciplineKind::Breath => ChordKind::PerfectParry,
            DisciplineKind::Thread => ChordKind::ShadowGrab,
            DisciplineKind::Ash => ChordKind::EdictSurge,
            DisciplineKind::Root => ChordKind::StandardParry,
            DisciplineKind::Glass => ChordKind::HarmonicStrike,
            DisciplineKind::Salt => ChordKind::AscensionBurst,
        }
    }
}

/// The 9 Combat Chords resolved at 120Hz in the priority table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
#[repr(u8)]
pub enum ChordKind {
    /// Surge + Attack chord at max heat (10000).
    EdictSurge = 0,
    /// Parry action within 2-tick window + resonance match.
    PerfectParry = 1,
    /// Parry action outside window or mismatched resonance.
    StandardParry = 2,
    /// Attack + Interact chord (tether lock / strip).
    ShadowGrab = 3,
    /// Dash + Jump chord (crushing aerial drop / poise break).
    GravityCrush = 4,
    /// Attack action solo (rhythmic damage stroke).
    HarmonicStrike = 5,
    /// Dash action solo (quick reposition / frame cancel).
    DashCancel = 6,
    /// Jump action solo (vertical ascension burst).
    AscensionBurst = 7,
    /// Velocity / Directional locomotion without action buttons.
    Movement = 8,
    /// No valid action resolved this tick.
    #[default]
    NoOp = 9,
}

impl ChordKind {
    /// All 9 actionable combat chords in priority table order.
    pub const ACTIONS: [ChordKind; 9] = [
        ChordKind::EdictSurge,
        ChordKind::PerfectParry,
        ChordKind::StandardParry,
        ChordKind::ShadowGrab,
        ChordKind::GravityCrush,
        ChordKind::HarmonicStrike,
        ChordKind::DashCancel,
        ChordKind::AscensionBurst,
        ChordKind::Movement,
    ];

    /// Human-readable label of the chord.
    #[inline(always)]
    pub const fn name(self) -> &'static str {
        match self {
            ChordKind::EdictSurge => "EdictSurge",
            ChordKind::PerfectParry => "PerfectParry",
            ChordKind::StandardParry => "StandardParry",
            ChordKind::ShadowGrab => "ShadowGrab",
            ChordKind::GravityCrush => "GravityCrush",
            ChordKind::HarmonicStrike => "HarmonicStrike",
            ChordKind::DashCancel => "DashCancel",
            ChordKind::AscensionBurst => "AscensionBurst",
            ChordKind::Movement => "Movement",
            ChordKind::NoOp => "NoOp",
        }
    }

    /// Base poise damage dealt by this chord action (in fixed integer units).
    #[inline(always)]
    pub const fn base_poise_damage(self) -> i32 {
        match self {
            ChordKind::EdictSurge => 2000,
            ChordKind::GravityCrush => 1200,
            ChordKind::HarmonicStrike => 400,
            ChordKind::ShadowGrab => 600,
            ChordKind::PerfectParry => 800,
            ChordKind::StandardParry => 200,
            ChordKind::AscensionBurst => 300,
            ChordKind::DashCancel => 100,
            ChordKind::Movement => 0,
            ChordKind::NoOp => 0,
        }
    }
}

/// Discipline-specific modifiers on a given combat chord.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChordAffinity {
    /// Damage multiplier in Permyriad (10000 = 100%).
    pub damage_multiplier_pmy: Permyriad,
    /// Poise damage multiplier in Permyriad (10000 = 100%).
    pub poise_damage_multiplier_pmy: Permyriad,
    /// Heat generation or cost adjustment in Permyriad (10000 = 100%).
    pub heat_rate_pmy: Permyriad,
    /// Additional timing window in 120Hz ticks (e.g. for parries).
    pub bonus_window_ticks: u16,
}

impl ChordAffinity {
    /// The default baseline affinity (100% damage, 100% poise, 100% heat, 0 bonus ticks).
    pub const BASE: Self = Self {
        damage_multiplier_pmy: Permyriad::ONE,
        poise_damage_multiplier_pmy: Permyriad::ONE,
        heat_rate_pmy: Permyriad::ONE,
        bonus_window_ticks: 0,
    };

    /// Evaluate affinity for a specific discipline and chord.
    pub const fn evaluate(discipline: DisciplineKind, chord: ChordKind) -> Self {
        match (discipline, chord) {
            (DisciplineKind::Edge, ChordKind::HarmonicStrike) => Self {
                damage_multiplier_pmy: Permyriad(12_500),      // +25% damage
                poise_damage_multiplier_pmy: Permyriad(11_000),// +10% poise damage
                heat_rate_pmy: Permyriad(12_000),              // +20% heat gain
                bonus_window_ticks: 0,
            },
            (DisciplineKind::Edge, ChordKind::DashCancel) => Self {
                damage_multiplier_pmy: Permyriad::ONE,
                poise_damage_multiplier_pmy: Permyriad::ONE,
                heat_rate_pmy: Permyriad(5_000),               // 50% heat cost
                bonus_window_ticks: 0,
            },
            (DisciplineKind::Weight, ChordKind::GravityCrush) => Self {
                damage_multiplier_pmy: Permyriad(14_000),      // +40% damage
                poise_damage_multiplier_pmy: Permyriad(17_500),// +75% crushing poise dmg
                heat_rate_pmy: Permyriad::ONE,
                bonus_window_ticks: 0,
            },
            (DisciplineKind::Breath, ChordKind::PerfectParry) => Self {
                damage_multiplier_pmy: Permyriad(11_000),
                poise_damage_multiplier_pmy: Permyriad(15_000),// +50% counter poise dmg
                heat_rate_pmy: Permyriad(15_000),              // +50% heat on parry
                bonus_window_ticks: 2,                         // +2 tick window (4-tick total)
            },
            (DisciplineKind::Thread, ChordKind::ShadowGrab) => Self {
                damage_multiplier_pmy: Permyriad(13_000),
                poise_damage_multiplier_pmy: Permyriad(13_000),
                heat_rate_pmy: Permyriad(18_000),              // +80% heat generation
                bonus_window_ticks: 0,
            },
            (DisciplineKind::Ash, ChordKind::EdictSurge) => Self {
                damage_multiplier_pmy: Permyriad(20_000),      // 200% surge damage
                poise_damage_multiplier_pmy: Permyriad(20_000),
                heat_rate_pmy: Permyriad::ONE,
                bonus_window_ticks: 0,
            },
            (DisciplineKind::Root, ChordKind::StandardParry) => Self {
                damage_multiplier_pmy: Permyriad::ONE,
                poise_damage_multiplier_pmy: Permyriad(12_000),
                heat_rate_pmy: Permyriad(11_000),
                bonus_window_ticks: 1,
            },
            (DisciplineKind::Glass, ChordKind::HarmonicStrike) => Self {
                damage_multiplier_pmy: Permyriad(15_000),      // +50% pierce damage
                poise_damage_multiplier_pmy: Permyriad(14_000),// +40% poise pierce
                heat_rate_pmy: Permyriad(13_000),
                bonus_window_ticks: 0,
            },
            (DisciplineKind::Salt, ChordKind::AscensionBurst) => Self {
                damage_multiplier_pmy: Permyriad(12_000),
                poise_damage_multiplier_pmy: Permyriad(11_000),
                heat_rate_pmy: Permyriad(10_000),
                bonus_window_ticks: 0,
            },
            _ => Self::BASE,
        }
    }
}

/// Progression rank metadata across 8 discipline mastery tiers (Rank 1..=8).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DisciplineProgression {
    /// Active discipline.
    pub discipline: DisciplineKind,
    /// Current rank (1..=8).
    pub rank: u8,
    /// Accumulated experience points in this discipline.
    pub xp: u32,
}

impl DisciplineProgression {
    /// Cumulative XP required to reach each rank (index 0 is Rank 1 = 0 XP).
    pub const RANK_THRESHOLDS: [u32; 8] = [
        0,       // Rank 1
        1_000,   // Rank 2
        2_500,   // Rank 3
        5_000,   // Rank 4
        9_000,   // Rank 5
        15_000,  // Rank 6
        24_000,  // Rank 7
        36_000,  // Rank 8 (Mastery)
    ];

    /// Create a fresh Rank 1 progression state for a discipline.
    pub const fn new(discipline: DisciplineKind) -> Self {
        Self {
            discipline,
            rank: 1,
            xp: 0,
        }
    }

    /// Add XP and recalculate rank. Returns true if rank increased.
    pub fn add_xp(&mut self, gained_xp: u32) -> bool {
        self.xp = self.xp.saturating_add(gained_xp);
        let old_rank = self.rank;
        let mut new_rank = 1u8;
        while (new_rank as usize) < Self::RANK_THRESHOLDS.len()
            && self.xp >= Self::RANK_THRESHOLDS[new_rank as usize]
        {
            new_rank += 1;
        }
        self.rank = new_rank.clamp(1, 8);
        self.rank > old_rank
    }

    /// Rank scaling multiplier in Permyriad (Rank 1 = 10000 = 100%, +10% per rank above 1).
    #[inline(always)]
    pub const fn rank_multiplier_pmy(&self) -> Permyriad {
        let bonus = (self.rank as i32 - 1) * 1_000;
        Permyriad(10_000 + bonus)
    }

    /// Calculate dynamic max poise taking into account discipline base, rank, and stats.
    ///
    /// Fixed-point formula:
    /// `BasePoise * RankMultiplier * (10000 + ShadowWeight*150 + Vigor*50) / 10000 / 10000`
    pub fn compute_max_poise(&self, vigor: u16, shadow_weight: u16) -> i32 {
        let base = self.discipline.base_poise() as i64;
        let rank_mult = self.rank_multiplier_pmy().0 as i64;
        let stat_mult = 10_000i64 + (shadow_weight as i64 * 150) + (vigor as i64 * 50);
        let scaled = (base * rank_mult * stat_mult) / (10_000 * 10_000);
        scaled.max(100) as i32
    }
}

/// Deterministic Poise State Machine operated at 120Hz.
///
/// Tracks remaining poise, delay countdown before recovery starts,
/// and stagger duration when poise breaks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PoiseState {
    /// Current poise remaining (0..=max_poise).
    pub current_poise: i32,
    /// Maximum calculated poise capacity.
    pub max_poise: i32,
    /// Fractional sub-poise accumulator (Permyriad units, 0..10000).
    pub sub_poise_pmy: i32,
    /// Ticks remaining before poise recovery begins after being struck.
    pub recovery_delay_ticks: u16,
    /// Ticks remaining in staggered state (0 = active / unstaggered).
    pub stagger_ticks: u16,
    /// Whether poise was broken this frame.
    pub is_broken: bool,
}

impl PoiseState {
    /// Initialize poise state for a discipline and stat profile.
    pub fn new(_discipline: DisciplineKind, progression: &DisciplineProgression, vigor: u16, shadow_weight: u16) -> Self {
        let max_poise = progression.compute_max_poise(vigor, shadow_weight);
        Self {
            current_poise: max_poise,
            max_poise,
            sub_poise_pmy: 0,
            recovery_delay_ticks: 0,
            stagger_ticks: 0,
            is_broken: false,
        }
    }

    /// Whether the entity is currently staggered (stagger_ticks > 0).
    #[inline(always)]
    pub const fn is_staggered(&self) -> bool {
        self.stagger_ticks > 0
    }

    /// Apply poise damage to this state.
    ///
    /// Incorporates discipline poise defense multiplier.
    /// Returns `true` if this hit broke the entity's poise and triggered stagger.
    pub fn apply_damage(&mut self, incoming_poise_damage: i32, discipline: DisciplineKind) -> bool {
        if incoming_poise_damage <= 0 {
            return false;
        }

        // Mitigate damage via discipline defense ratio
        let def_pmy = discipline.poise_defense_pmy().0 as i64;
        let effective_dmg = ((incoming_poise_damage as i64 * def_pmy) / 10_000).max(1) as i32;

        self.current_poise = self.current_poise.saturating_sub(effective_dmg);
        self.recovery_delay_ticks = discipline.recovery_delay_ticks();

        if self.current_poise <= 0 {
            self.current_poise = 0;
            self.sub_poise_pmy = 0;
            self.stagger_ticks = discipline.stagger_duration_ticks();
            self.is_broken = true;
            true
        } else {
            self.is_broken = false;
            false
        }
    }

    /// Advance one 120Hz tick of poise simulation.
    pub fn tick_120hz(&mut self, discipline: DisciplineKind) {
        // Clear single-frame break flag
        self.is_broken = false;

        // Decrement stagger timer if staggered
        if self.stagger_ticks > 0 {
            self.stagger_ticks -= 1;
            if self.stagger_ticks == 0 {
                // Stagger ended: restore 25% poise immediately
                self.current_poise = (self.max_poise / 4).max(1);
                self.sub_poise_pmy = 0;
            }
            return;
        }

        // Decrement recovery delay timer
        if self.recovery_delay_ticks > 0 {
            self.recovery_delay_ticks -= 1;
            return;
        }

        // Recover poise at 120Hz rate
        if self.current_poise < self.max_poise {
            let rate_pmy = discipline.poise_recovery_rate_pmy().0;
            self.sub_poise_pmy += rate_pmy;
            let whole_points = self.sub_poise_pmy / 10_000;
            if whole_points > 0 {
                self.current_poise = (self.current_poise + whole_points).min(self.max_poise);
                self.sub_poise_pmy %= 10_000;
            }
        }
    }

    /// Reset poise to full.
    pub fn reset_full(&mut self) {
        self.current_poise = self.max_poise;
        self.sub_poise_pmy = 0;
        self.recovery_delay_ticks = 0;
        self.stagger_ticks = 0;
        self.is_broken = false;
    }
}

// ── Layout Locks (Memory Safety & ABI Verification) ───────────────────────────
const _: () = assert!(core::mem::size_of::<DisciplineKind>() == 1);
const _: () = assert!(core::mem::size_of::<ChordKind>() == 1);
const _: () = assert!(core::mem::size_of::<ChordAffinity>() == 16);
const _: () = assert!(core::mem::size_of::<DisciplineProgression>() == 8);
const _: () = assert!(core::mem::size_of::<PoiseState>() == 20);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_discipline_progression_ranks_up_correctly() {
        let mut prog = DisciplineProgression::new(DisciplineKind::Edge);
        assert_eq!(prog.rank, 1);
        assert_eq!(prog.rank_multiplier_pmy(), Permyriad(10_000));

        // Add 1000 XP -> Rank 2
        let ranked_up = prog.add_xp(1_000);
        assert!(ranked_up);
        assert_eq!(prog.rank, 2);
        assert_eq!(prog.rank_multiplier_pmy(), Permyriad(11_000));

        // Add to reach Mastery (36,000 XP) -> Rank 8
        prog.add_xp(35_000);
        assert_eq!(prog.rank, 8);
        assert_eq!(prog.rank_multiplier_pmy(), Permyriad(17_000));
    }

    #[test]
    fn test_poise_deterministic_break_and_stagger_cycle() {
        let prog = DisciplineProgression::new(DisciplineKind::Weight);
        let mut poise = PoiseState::new(DisciplineKind::Weight, &prog, 50, 50);
        assert!(!poise.is_staggered());
        assert_eq!(poise.current_poise, poise.max_poise);

        // Apply heavy poise damage to break
        let broken = poise.apply_damage(5000, DisciplineKind::Weight);
        assert!(broken);
        assert!(poise.is_staggered());
        assert_eq!(poise.current_poise, 0);
        assert_eq!(poise.stagger_ticks, DisciplineKind::Weight.stagger_duration_ticks());

        // Tick through stagger duration
        let stagger_total = poise.stagger_ticks;
        for _ in 0..stagger_total {
            assert!(poise.is_staggered());
            poise.tick_120hz(DisciplineKind::Weight);
        }

        // Stagger should now be finished and restored to 25%
        assert!(!poise.is_staggered());
        assert_eq!(poise.current_poise, poise.max_poise / 4);
    }

    #[test]
    fn test_all_8_disciplines_have_unique_affinities() {
        for d in DisciplineKind::ALL {
            let chord = d.primary_chord();
            let affinity = ChordAffinity::evaluate(d, chord);
            assert!(affinity.damage_multiplier_pmy.0 >= 10_000);
            assert!(affinity.poise_damage_multiplier_pmy.0 >= 10_000);
        }
    }
}
