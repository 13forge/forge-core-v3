//! ScalingProfile — unifies class+role stat distributions.
//!
//! Bridges three stat schemas:
//!   1. CoreStats (7 base attributes from creature_engine.rs)
//!   2. RpgStats (6 essence-derived attributes from essence_registry.rs)
//!   3. Ability scaling coefficients
//!
//! Design: class + role → {primary_stat, secondary_stats[], tertiary_stats[], scaling_matrix}
//! Provides deterministic stat allocation and coefficient lookup by (class, ability, stat).

/// Character class determines primary/secondary stat distributions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum CharacterClass {
    /// Physical powerhouse: STR primary, AGI/STA secondary.
    Warrior,
    /// Spell wielder: INT/WIS primary, CHA secondary.
    Mage,
    /// Swift striker: AGI/DEX primary, STR secondary.
    Rogue,
    /// Holy crusader: WIS/CHA primary, STR secondary.
    Paladin,
    /// Nature wielder: WIS/DEX primary, STA secondary.
    Ranger,
    /// Protective guardian: STA/STR primary, DEX secondary.
    Guardian,
}

impl CharacterClass {
    /// All character classes in ordinal order.
    pub const ALL: [CharacterClass; 6] = [
        CharacterClass::Warrior,
        CharacterClass::Mage,
        CharacterClass::Rogue,
        CharacterClass::Paladin,
        CharacterClass::Ranger,
        CharacterClass::Guardian,
    ];

    /// Human-readable name for this class.
    pub const fn name(self) -> &'static str {
        match self {
            CharacterClass::Warrior => "Warrior",
            CharacterClass::Mage => "Mage",
            CharacterClass::Rogue => "Rogue",
            CharacterClass::Paladin => "Paladin",
            CharacterClass::Ranger => "Ranger",
            CharacterClass::Guardian => "Guardian",
        }
    }
}

/// Role specialization layered on top of class. Refines stat focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum CharacterRole {
    /// Damage dealer: primary stat × 1.5, secondary × 1.2.
    Damage,
    /// Tank/defender: STA and AC gains, secondary stat × 1.3.
    Tank,
    /// Support/healer: WIS and CHA gains, secondary stat × 1.2.
    Support,
}

impl CharacterRole {
    /// All character roles in ordinal order.
    pub const ALL: [CharacterRole; 3] = [
        CharacterRole::Damage,
        CharacterRole::Tank,
        CharacterRole::Support,
    ];

    /// Human-readable name for this role.
    pub const fn name(self) -> &'static str {
        match self {
            CharacterRole::Damage => "Damage",
            CharacterRole::Tank => "Tank",
            CharacterRole::Support => "Support",
        }
    }
}

/// CoreStat variants: the 7 base attributes from creature_engine.rs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum CoreStat {
    /// Strength: raw physical power (STR).
    Str,
    /// Stamina: endurance, HP pool (STA).
    Sta,
    /// Agility: movement speed, dodge (AGI).
    Agi,
    /// Dexterity: precision, crit, ranged (DEX).
    Dex,
    /// Wisdom: awareness, magic resist (WIS).
    Wis,
    /// Intelligence: magic power, puzzle (INT).
    Int,
    /// Charisma: faction influence, trade (CHA).
    Cha,
}

impl CoreStat {
    /// All core stats in ordinal order.
    pub const ALL: [CoreStat; 7] = [
        CoreStat::Str,
        CoreStat::Sta,
        CoreStat::Agi,
        CoreStat::Dex,
        CoreStat::Wis,
        CoreStat::Int,
        CoreStat::Cha,
    ];

    /// Short 3-letter abbreviation.
    pub const fn abbr(self) -> &'static str {
        match self {
            CoreStat::Str => "STR",
            CoreStat::Sta => "STA",
            CoreStat::Agi => "AGI",
            CoreStat::Dex => "DEX",
            CoreStat::Wis => "WIS",
            CoreStat::Int => "INT",
            CoreStat::Cha => "CHA",
        }
    }

    /// Full name.
    pub const fn name(self) -> &'static str {
        match self {
            CoreStat::Str => "Strength",
            CoreStat::Sta => "Stamina",
            CoreStat::Agi => "Agility",
            CoreStat::Dex => "Dexterity",
            CoreStat::Wis => "Wisdom",
            CoreStat::Int => "Intelligence",
            CoreStat::Cha => "Charisma",
        }
    }
}

/// Stat assignment: a stat + priority level (primary/secondary/tertiary).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StatAssignment {
    /// The core stat.
    pub stat: CoreStat,
    /// Priority rank: 0 = primary, 1 = secondary, 2 = tertiary.
    pub priority: u8,
}

impl StatAssignment {
    /// Create a new stat assignment.
    #[inline]
    pub const fn new(stat: CoreStat, priority: u8) -> Self {
        StatAssignment { stat, priority }
    }

    /// Is this the primary stat (priority 0)?
    #[inline]
    pub const fn is_primary(self) -> bool {
        self.priority == 0
    }

    /// Is this a secondary stat (priority 1)?
    #[inline]
    pub const fn is_secondary(self) -> bool {
        self.priority == 1
    }

    /// Is this a tertiary stat (priority 2)?
    #[inline]
    pub const fn is_tertiary(self) -> bool {
        self.priority == 2
    }
}

/// Complete stat scaling profile for a class+role combination.
/// Maps core stats to primary/secondary/tertiary designations
/// and stores scaling coefficients for abilities.
#[derive(Debug, Clone)]
pub struct ScalingProfile {
    /// Character class.
    pub class: CharacterClass,
    /// Character role (optional specialization).
    pub role: Option<CharacterRole>,
    /// Stat assignments: core stats with priority levels.
    pub assignments: Vec<StatAssignment>,
    /// Scaling matrix: (ability_id, stat_type) → coefficient.
    /// Stored as a flat array of (ability_id: u8, stat_type: u8, coeff: f32).
    pub scaling_matrix: Vec<(u8, u8, f32)>,
}

impl ScalingProfile {
    /// Create a new scaling profile from class and optional role.
    pub fn new(class: CharacterClass, role: Option<CharacterRole>) -> Self {
        let assignments = Self::default_assignments(class);
        ScalingProfile {
            class,
            role,
            assignments,
            scaling_matrix: Vec::new(),
        }
    }

    /// Generate default stat assignments based on class alone.
    fn default_assignments(class: CharacterClass) -> Vec<StatAssignment> {
        match class {
            CharacterClass::Warrior => vec![
                StatAssignment::new(CoreStat::Str, 0),
                StatAssignment::new(CoreStat::Agi, 1),
                StatAssignment::new(CoreStat::Sta, 1),
                StatAssignment::new(CoreStat::Dex, 2),
            ],
            CharacterClass::Mage => vec![
                StatAssignment::new(CoreStat::Int, 0),
                StatAssignment::new(CoreStat::Wis, 0),
                StatAssignment::new(CoreStat::Cha, 1),
                StatAssignment::new(CoreStat::Dex, 2),
            ],
            CharacterClass::Rogue => vec![
                StatAssignment::new(CoreStat::Agi, 0),
                StatAssignment::new(CoreStat::Dex, 0),
                StatAssignment::new(CoreStat::Str, 1),
                StatAssignment::new(CoreStat::Sta, 2),
            ],
            CharacterClass::Paladin => vec![
                StatAssignment::new(CoreStat::Wis, 0),
                StatAssignment::new(CoreStat::Cha, 0),
                StatAssignment::new(CoreStat::Str, 1),
                StatAssignment::new(CoreStat::Sta, 2),
            ],
            CharacterClass::Ranger => vec![
                StatAssignment::new(CoreStat::Wis, 0),
                StatAssignment::new(CoreStat::Dex, 0),
                StatAssignment::new(CoreStat::Agi, 1),
                StatAssignment::new(CoreStat::Sta, 1),
            ],
            CharacterClass::Guardian => vec![
                StatAssignment::new(CoreStat::Sta, 0),
                StatAssignment::new(CoreStat::Str, 0),
                StatAssignment::new(CoreStat::Dex, 1),
                StatAssignment::new(CoreStat::Wis, 2),
            ],
        }
    }

    /// Get all primary stats for this profile.
    pub fn primary_stats(&self) -> Vec<CoreStat> {
        self.assignments
            .iter()
            .filter(|a| a.is_primary())
            .map(|a| a.stat)
            .collect()
    }

    /// Get all secondary stats for this profile.
    pub fn secondary_stats(&self) -> Vec<CoreStat> {
        self.assignments
            .iter()
            .filter(|a| a.is_secondary())
            .map(|a| a.stat)
            .collect()
    }

    /// Get all tertiary stats for this profile.
    pub fn tertiary_stats(&self) -> Vec<CoreStat> {
        self.assignments
            .iter()
            .filter(|a| a.is_tertiary())
            .map(|a| a.stat)
            .collect()
    }

    /// Look up the scaling coefficient for a (class, ability_id, stat_type).
    /// Returns the coefficient, or 0.0 if not found.
    pub fn lookup_scaling_coeff(&self, ability_id: u8, stat_type: u8) -> f32 {
        self.scaling_matrix
            .iter()
            .find(|(aid, stype, _)| *aid == ability_id && *stype == stat_type)
            .map(|(_, _, coeff)| *coeff)
            .unwrap_or(0.0)
    }

    /// Add a scaling coefficient to the matrix.
    /// (ability_id, stat_type) → coefficient.
    pub fn add_scaling(&mut self, ability_id: u8, stat_type: u8, coeff: f32) {
        if coeff != 0.0 {
            self.scaling_matrix.push((ability_id, stat_type, coeff));
        }
    }

    /// Compute the total stat pool for this profile.
    /// Sums: primary stats × 1.5 + secondary stats × 1.2 + tertiary stats × 1.0.
    pub fn compute_stat_pool(&self, base_stats: &CoreStatValues) -> f32 {
        let mut pool = 0.0;
        for assignment in &self.assignments {
            let value = base_stats.get_stat(assignment.stat);
            let weight = match assignment.priority {
                0 => 1.5, // primary
                1 => 1.2, // secondary
                _ => 1.0, // tertiary
            };
            pool += value * weight;
        }
        pool
    }
}

/// Character stat values for the 7 core attributes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CoreStatValues {
    /// Strength stat value.
    pub str: f32,
    /// Stamina stat value.
    pub sta: f32,
    /// Agility stat value.
    pub agi: f32,
    /// Dexterity stat value.
    pub dex: f32,
    /// Wisdom stat value.
    pub wis: f32,
    /// Intelligence stat value.
    pub int: f32,
    /// Charisma stat value.
    pub cha: f32,
}

impl CoreStatValues {
    /// Create new core stat values.
    #[inline]
    pub const fn new(str: f32, sta: f32, agi: f32, dex: f32, wis: f32, int: f32, cha: f32) -> Self {
        CoreStatValues { str, sta, agi, dex, wis, int, cha }
    }

    /// Get a stat value by type.
    #[inline]
    pub fn get_stat(&self, stat: CoreStat) -> f32 {
        match stat {
            CoreStat::Str => self.str,
            CoreStat::Sta => self.sta,
            CoreStat::Agi => self.agi,
            CoreStat::Dex => self.dex,
            CoreStat::Wis => self.wis,
            CoreStat::Int => self.int,
            CoreStat::Cha => self.cha,
        }
    }

    /// Set a stat value by type.
    #[inline]
    pub fn set_stat(&mut self, stat: CoreStat, value: f32) {
        match stat {
            CoreStat::Str => self.str = value,
            CoreStat::Sta => self.sta = value,
            CoreStat::Agi => self.agi = value,
            CoreStat::Dex => self.dex = value,
            CoreStat::Wis => self.wis = value,
            CoreStat::Int => self.int = value,
            CoreStat::Cha => self.cha = value,
        }
    }

    /// Sum all stats.
    #[inline]
    pub fn total(&self) -> f32 {
        self.str + self.sta + self.agi + self.dex + self.wis + self.int + self.cha
    }
}

impl Default for CoreStatValues {
    fn default() -> Self {
        CoreStatValues::new(0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn warrior_primary_is_str() {
        let profile = ScalingProfile::new(CharacterClass::Warrior, None);
        let primaries = profile.primary_stats();
        assert!(primaries.contains(&CoreStat::Str), "Warrior primary should include STR");
    }

    #[test]
    fn warrior_secondaries_include_agi_and_sta() {
        let profile = ScalingProfile::new(CharacterClass::Warrior, None);
        let secondaries = profile.secondary_stats();
        assert!(secondaries.contains(&CoreStat::Agi), "Warrior secondaries should include AGI");
        assert!(secondaries.contains(&CoreStat::Sta), "Warrior secondaries should include STA");
    }

    #[test]
    fn mage_primary_is_int_and_wis() {
        let profile = ScalingProfile::new(CharacterClass::Mage, None);
        let primaries = profile.primary_stats();
        assert!(primaries.contains(&CoreStat::Int), "Mage primary should include INT");
        assert!(primaries.contains(&CoreStat::Wis), "Mage primary should include WIS");
    }

    #[test]
    fn rogue_primary_is_agi_and_dex() {
        let profile = ScalingProfile::new(CharacterClass::Rogue, None);
        let primaries = profile.primary_stats();
        assert!(primaries.contains(&CoreStat::Agi), "Rogue primary should include AGI");
        assert!(primaries.contains(&CoreStat::Dex), "Rogue primary should include DEX");
    }

    #[test]
    fn scaling_profile_lookup_scaling_coeff() {
        let mut profile = ScalingProfile::new(CharacterClass::Warrior, None);
        profile.add_scaling(1, 2, 0.5);
        let coeff = profile.lookup_scaling_coeff(1, 2);
        assert!((coeff - 0.5).abs() < 0.001, "Scaling coefficient lookup failed");
    }

    #[test]
    fn scaling_profile_lookup_missing_returns_zero() {
        let profile = ScalingProfile::new(CharacterClass::Warrior, None);
        let coeff = profile.lookup_scaling_coeff(99, 99);
        assert_eq!(coeff, 0.0, "Missing scaling coefficient should return 0.0");
    }

    #[test]
    fn core_stat_values_get_stat() {
        let stats = CoreStatValues::new(10.0, 20.0, 30.0, 40.0, 50.0, 60.0, 70.0);
        assert_eq!(stats.get_stat(CoreStat::Str), 10.0);
        assert_eq!(stats.get_stat(CoreStat::Sta), 20.0);
        assert_eq!(stats.get_stat(CoreStat::Agi), 30.0);
        assert_eq!(stats.get_stat(CoreStat::Dex), 40.0);
        assert_eq!(stats.get_stat(CoreStat::Wis), 50.0);
        assert_eq!(stats.get_stat(CoreStat::Int), 60.0);
        assert_eq!(stats.get_stat(CoreStat::Cha), 70.0);
    }

    #[test]
    fn core_stat_values_set_stat() {
        let mut stats = CoreStatValues::default();
        stats.set_stat(CoreStat::Str, 15.0);
        assert_eq!(stats.str, 15.0);
    }

    #[test]
    fn core_stat_values_total() {
        let stats = CoreStatValues::new(10.0, 20.0, 30.0, 40.0, 50.0, 60.0, 70.0);
        assert_eq!(stats.total(), 280.0);
    }

    #[test]
    fn stat_assignment_priority_checks() {
        let primary = StatAssignment::new(CoreStat::Str, 0);
        let secondary = StatAssignment::new(CoreStat::Agi, 1);
        let tertiary = StatAssignment::new(CoreStat::Dex, 2);

        assert!(primary.is_primary());
        assert!(!primary.is_secondary());
        assert!(!primary.is_tertiary());

        assert!(!secondary.is_primary());
        assert!(secondary.is_secondary());
        assert!(!secondary.is_tertiary());

        assert!(!tertiary.is_primary());
        assert!(!tertiary.is_secondary());
        assert!(tertiary.is_tertiary());
    }

    #[test]
    fn character_class_all_have_names() {
        for class in CharacterClass::ALL.iter() {
            assert!(!class.name().is_empty(), "Class should have a non-empty name");
        }
    }

    #[test]
    fn character_role_all_have_names() {
        for role in CharacterRole::ALL.iter() {
            assert!(!role.name().is_empty(), "Role should have a non-empty name");
        }
    }

    #[test]
    fn core_stat_all_have_abbreviations() {
        for stat in CoreStat::ALL.iter() {
            let abbr = stat.abbr();
            assert_eq!(abbr.len(), 3, "Abbreviation should be 3 characters");
        }
    }

    #[test]
    fn scaling_profile_compute_stat_pool() {
        let profile = ScalingProfile::new(CharacterClass::Warrior, None);
        let stats = CoreStatValues::new(10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0);
        let pool = profile.compute_stat_pool(&stats);
        // Warrior: STR(primary)×1.5 + AGI(secondary)×1.2 + STA(secondary)×1.2 + DEX(tertiary)×1.0
        // = 10×1.5 + 10×1.2 + 10×1.2 + 10×1.0 = 15 + 12 + 12 + 10 = 49
        assert!((pool - 49.0).abs() < 0.001, "Stat pool calculation failed");
    }

    #[test]
    fn guardian_primary_is_sta_and_str() {
        let profile = ScalingProfile::new(CharacterClass::Guardian, None);
        let primaries = profile.primary_stats();
        assert!(primaries.contains(&CoreStat::Sta), "Guardian primary should include STA");
        assert!(primaries.contains(&CoreStat::Str), "Guardian primary should include STR");
    }

    #[test]
    fn paladin_primary_is_wis_and_cha() {
        let profile = ScalingProfile::new(CharacterClass::Paladin, None);
        let primaries = profile.primary_stats();
        assert!(primaries.contains(&CoreStat::Wis), "Paladin primary should include WIS");
        assert!(primaries.contains(&CoreStat::Cha), "Paladin primary should include CHA");
    }

    #[test]
    fn ranger_primary_is_wis_and_dex() {
        let profile = ScalingProfile::new(CharacterClass::Ranger, None);
        let primaries = profile.primary_stats();
        assert!(primaries.contains(&CoreStat::Wis), "Ranger primary should include WIS");
        assert!(primaries.contains(&CoreStat::Dex), "Ranger primary should include DEX");
    }

    #[test]
    fn all_classes_have_defined_assignments() {
        for class in CharacterClass::ALL.iter() {
            let profile = ScalingProfile::new(*class, None);
            assert!(!profile.assignments.is_empty(), "All classes should have stat assignments");
        }
    }
}
