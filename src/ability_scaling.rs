//! Ability Scaling — codifies which stats abilities draw from.
//!
//! Each ability has a base power value and scales off one or more character stats
//! with individual coefficients. Damage and healing calculations use the scaling
//! formula: `base + Σ(stat × coefficient)` across all scaling sources.

/// Hermetic stat types that abilities can scale from. Distinct from the gem-socketing
/// `StatType` in forge-book (Str/Int) — these are character ability scaling stats.
/// Eight-stat block: Vigor, ShadowWeight, LogicDepth, Momentum, Tarnish, Resonance, Guilt, Clarity.
/// NOTE: Clarity starts at 0 (earned in play, never dealt at birth) per hermetics.rs:842.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum AbilityStat {
    /// Physical vigor and endurance.
    Vigor,
    /// Shadow weight and density (darkness affinity).
    ShadowWeight,
    /// Logic depth and reasoning power.
    LogicDepth,
    /// Momentum and kinetic force.
    Momentum,
    /// Tarnish and corrosion (material decay).
    Tarnish,
    /// Resonance and harmony.
    Resonance,
    /// Guilt and burden (note-ledger accrual).
    Guilt,
    /// Clarity and perception (earned in play, never dealt at birth).
    Clarity,
}

impl AbilityStat {
    /// All stat types in ordinal order.
    pub const ALL: [AbilityStat; 8] = [
        AbilityStat::Vigor,
        AbilityStat::ShadowWeight,
        AbilityStat::LogicDepth,
        AbilityStat::Momentum,
        AbilityStat::Tarnish,
        AbilityStat::Resonance,
        AbilityStat::Guilt,
        AbilityStat::Clarity,
    ];

    /// Short name for this stat.
    pub const fn short_name(self) -> &'static str {
        match self {
            AbilityStat::Vigor => "VIG",
            AbilityStat::ShadowWeight => "SHD",
            AbilityStat::LogicDepth => "LOG",
            AbilityStat::Momentum => "MOM",
            AbilityStat::Tarnish => "TAR",
            AbilityStat::Resonance => "RES",
            AbilityStat::Guilt => "GUL",
            AbilityStat::Clarity => "CLR",
        }
    }
}

/// One stat-coefficient pair for ability scaling.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScalingPair {
    /// Which stat this scales from.
    pub stat: AbilityStat,
    /// Coefficient applied to the stat value (0.0 to 1.0 typical).
    pub coeff: f32,
}

impl ScalingPair {
    /// Create a new scaling pair.
    #[inline]
    pub const fn new(stat: AbilityStat, coeff: f32) -> Self {
        ScalingPair { stat, coeff }
    }

    /// Compute scaled value: stat × coefficient.
    #[inline]
    pub fn apply(self, stat_value: f32) -> f32 {
        stat_value * self.coeff
    }
}

/// Definition of one ability's power and scaling properties.
#[derive(Debug, Clone, PartialEq)]
pub struct AbilityDef {
    /// Ability name.
    pub name: &'static str,
    /// Base damage or healing value, before scaling.
    pub base_power: f32,
    /// Stat-coefficient pairs that modify the base power.
    pub scaling_stats: Vec<ScalingPair>,
}

impl AbilityDef {
    /// Create a new ability definition.
    pub fn new(name: &'static str, base_power: f32) -> Self {
        AbilityDef {
            name,
            base_power,
            scaling_stats: Vec::new(),
        }
    }

    /// Add a scaling stat to this ability. Returns self for chaining.
    pub fn with_scaling(mut self, stat: AbilityStat, coeff: f32) -> Self {
        self.scaling_stats.push(ScalingPair::new(stat, coeff));
        self
    }

    /// Add multiple scaling pairs at once.
    pub fn with_scaling_pairs(mut self, pairs: Vec<ScalingPair>) -> Self {
        self.scaling_stats.extend(pairs);
        self
    }

    /// Compute total power based on character stats. Sums base power plus
    /// all scaling applications.
    pub fn compute_power(&self, character_stats: &CharacterStats) -> f32 {
        let scaled: f32 = self
            .scaling_stats
            .iter()
            .map(|pair| pair.apply(character_stats.get_stat(pair.stat)))
            .sum();
        self.base_power + scaled
    }
}

/// Character stat values used in ability power calculations.
/// All 8 stats: Vigor, ShadowWeight, LogicDepth, Momentum, Tarnish, Resonance, Guilt, Clarity.
/// Note: Clarity starts at 0 (earned in play, never dealt at birth).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CharacterStats {
    /// Physical vigor value.
    pub vigor: f32,
    /// Shadow weight value.
    pub shadow_weight: f32,
    /// Logic depth value.
    pub logic_depth: f32,
    /// Momentum value.
    pub momentum: f32,
    /// Tarnish value.
    pub tarnish: f32,
    /// Resonance value.
    pub resonance: f32,
    /// Guilt value (accrued via note-ledger).
    pub guilt: f32,
    /// Clarity value (earned in play, starts at 0).
    pub clarity: f32,
}

impl CharacterStats {
    /// Create new character stats.
    #[inline]
    pub const fn new(
        vigor: f32,
        shadow_weight: f32,
        logic_depth: f32,
        momentum: f32,
        tarnish: f32,
        resonance: f32,
        guilt: f32,
        clarity: f32,
    ) -> Self {
        CharacterStats {
            vigor,
            shadow_weight,
            logic_depth,
            momentum,
            tarnish,
            resonance,
            guilt,
            clarity,
        }
    }

    /// Get a stat value by type.
    #[inline]
    pub fn get_stat(&self, stat: AbilityStat) -> f32 {
        match stat {
            AbilityStat::Vigor => self.vigor,
            AbilityStat::ShadowWeight => self.shadow_weight,
            AbilityStat::LogicDepth => self.logic_depth,
            AbilityStat::Momentum => self.momentum,
            AbilityStat::Tarnish => self.tarnish,
            AbilityStat::Resonance => self.resonance,
            AbilityStat::Guilt => self.guilt,
            AbilityStat::Clarity => self.clarity,
        }
    }
}

/// Registry of core abilities and their scaling properties.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AbilityRegistry {
    /// Physical melee strike: scales from Vigor (primary) and Momentum (secondary).
    Slash,
    /// Fireball magic: scales from LogicDepth (primary).
    Fireball,
    /// Light healing: scales from LogicDepth and Resonance.
    Heal,
    /// Piercing strike: scales from Momentum and Vigor.
    Thrust,
    /// Elemental beam: scales from LogicDepth.
    Beam,
}

impl AbilityRegistry {
    /// Get the ability definition for this registry variant.
    pub fn definition(self) -> AbilityDef {
        match self {
            AbilityRegistry::Slash => AbilityDef::new("Slash", 20.0)
                .with_scaling(AbilityStat::Vigor, 0.15)
                .with_scaling(AbilityStat::Momentum, 0.05),
            AbilityRegistry::Fireball => AbilityDef::new("Fireball", 35.0)
                .with_scaling(AbilityStat::LogicDepth, 0.20),
            AbilityRegistry::Heal => AbilityDef::new("Heal", 25.0)
                .with_scaling(AbilityStat::LogicDepth, 0.10)
                .with_scaling(AbilityStat::Resonance, 0.08),
            AbilityRegistry::Thrust => AbilityDef::new("Thrust", 30.0)
                .with_scaling(AbilityStat::Momentum, 0.12)
                .with_scaling(AbilityStat::Vigor, 0.08),
            AbilityRegistry::Beam => AbilityDef::new("Beam", 40.0)
                .with_scaling(AbilityStat::LogicDepth, 0.18),
        }
    }

    /// Compute ability power for a given character.
    /// Shortcut: `ability.definition().compute_power(stats)`.
    pub fn compute_power(self, stats: &CharacterStats) -> f32 {
        self.definition().compute_power(stats)
    }

    /// Get all core abilities in ordinal order.
    pub const fn all() -> [AbilityRegistry; 5] {
        [
            AbilityRegistry::Slash,
            AbilityRegistry::Fireball,
            AbilityRegistry::Heal,
            AbilityRegistry::Thrust,
            AbilityRegistry::Beam,
        ]
    }
}

/// Compute the total power (damage or healing) of an ability for a given character.
///
/// This is the primary function for resolving ability effects in gameplay.
/// Returns: `base_power + Σ(stat × coefficient)`
#[inline]
pub fn compute_ability_power(ability: AbilityRegistry, stats: &CharacterStats) -> f32 {
    ability.compute_power(stats)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slash_with_str100_dex50_correct_damage() {
        let stats = CharacterStats::new(100.0, 0.0, 0.0, 50.0, 0.0, 0.0, 0.0, 0.0);
        let damage = compute_ability_power(AbilityRegistry::Slash, &stats);
        let expected = 20.0 + (100.0 * 0.15) + (50.0 * 0.05);
        assert!((damage - expected).abs() < 0.001, "Slash damage mismatch: got {}, expected {}", damage, expected);
    }

    #[test]
    fn fireball_scales_from_logic_depth() {
        let stats = CharacterStats::new(0.0, 0.0, 100.0, 0.0, 0.0, 0.0, 0.0, 0.0);
        let damage = compute_ability_power(AbilityRegistry::Fireball, &stats);
        let expected = 35.0 + (100.0 * 0.20);
        assert!((damage - expected).abs() < 0.001, "Fireball damage mismatch: got {}, expected {}", damage, expected);
    }

    #[test]
    fn heal_scales_from_logic_and_resonance() {
        let stats = CharacterStats::new(0.0, 0.0, 50.0, 0.0, 0.0, 60.0, 0.0, 0.0);
        let healing = compute_ability_power(AbilityRegistry::Heal, &stats);
        let expected = 25.0 + (50.0 * 0.10) + (60.0 * 0.08);
        assert!((healing - expected).abs() < 0.001, "Heal power mismatch: got {}, expected {}", healing, expected);
    }

    #[test]
    fn thrust_scales_from_momentum_and_vigor() {
        let stats = CharacterStats::new(80.0, 0.0, 0.0, 90.0, 0.0, 0.0, 0.0, 0.0);
        let damage = compute_ability_power(AbilityRegistry::Thrust, &stats);
        let expected = 30.0 + (90.0 * 0.12) + (80.0 * 0.08);
        assert!((damage - expected).abs() < 0.001, "Thrust damage mismatch: got {}, expected {}", damage, expected);
    }

    #[test]
    fn beam_scales_from_logic_depth() {
        let stats = CharacterStats::new(0.0, 0.0, 120.0, 0.0, 0.0, 0.0, 0.0, 0.0);
        let damage = compute_ability_power(AbilityRegistry::Beam, &stats);
        let expected = 40.0 + (120.0 * 0.18);
        assert!((damage - expected).abs() < 0.001, "Beam damage mismatch: got {}, expected {}", damage, expected);
    }

    #[test]
    fn ability_def_can_be_constructed_manually() {
        let ability = AbilityDef::new("TestAbility", 50.0)
            .with_scaling(AbilityStat::Vigor, 0.25)
            .with_scaling(AbilityStat::LogicDepth, 0.15);

        let stats = CharacterStats::new(100.0, 0.0, 80.0, 0.0, 0.0, 0.0, 0.0, 0.0);
        let power = ability.compute_power(&stats);
        let expected = 50.0 + (100.0 * 0.25) + (80.0 * 0.15);
        assert!((power - expected).abs() < 0.001, "Manual ability power mismatch: got {}, expected {}", power, expected);
    }

    #[test]
    fn scaling_pair_apply_works() {
        let pair = ScalingPair::new(AbilityStat::Vigor, 0.20);
        let scaled = pair.apply(150.0);
        assert!((scaled - 30.0).abs() < 0.001, "Scaling pair apply failed: got {}, expected 30.0", scaled);
    }

    #[test]
    fn character_stats_get_stat_by_type() {
        let stats = CharacterStats::new(10.0, 20.0, 30.0, 40.0, 50.0, 60.0, 70.0, 80.0);
        assert_eq!(stats.get_stat(AbilityStat::Vigor), 10.0);
        assert_eq!(stats.get_stat(AbilityStat::ShadowWeight), 20.0);
        assert_eq!(stats.get_stat(AbilityStat::LogicDepth), 30.0);
        assert_eq!(stats.get_stat(AbilityStat::Momentum), 40.0);
        assert_eq!(stats.get_stat(AbilityStat::Tarnish), 50.0);
        assert_eq!(stats.get_stat(AbilityStat::Resonance), 60.0);
        assert_eq!(stats.get_stat(AbilityStat::Guilt), 70.0);
        assert_eq!(stats.get_stat(AbilityStat::Clarity), 80.0);
    }

    #[test]
    fn all_abilities_can_be_defined() {
        for ability in AbilityRegistry::all().iter() {
            let def = ability.definition();
            assert!(!def.name.is_empty(), "Ability has no name");
            assert!(def.base_power >= 0.0, "Base power should be non-negative");
        }
    }

    #[test]
    fn zero_stats_yields_base_power_only() {
        let stats = CharacterStats::new(0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
        for ability in AbilityRegistry::all().iter() {
            let power = ability.compute_power(&stats);
            let base = ability.definition().base_power;
            assert!((power - base).abs() < 0.001, "Zero stats should yield base power for {}", ability.definition().name);
        }
    }

    #[test]
    fn multiple_stats_sum_correctly() {
        let stats = CharacterStats::new(50.0, 40.0, 30.0, 20.0, 0.0, 10.0, 0.0, 0.0);
        let slash_power = compute_ability_power(AbilityRegistry::Slash, &stats);
        let expected = 20.0 + (50.0 * 0.15) + (20.0 * 0.05);
        assert!((slash_power - expected).abs() < 0.001, "Multi-stat scaling failed");
    }
}
