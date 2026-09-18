//! Ported verbatim from F:\NewRepo\crates\forge-consequence\src\rule.rs (2026-08-17 truth-hunt lineage port, completing the 2026-08-13 wce-tags-port).
//!
//! Consequence Rule Engine and Simulation State Pipeline.
//!
//! Provides the data structures and transition logic to map high-level simulation
//! state (`LocalSimState`) into low-level 16-byte `InteractionQuery` representations.
//! It also implements `ConsequenceRule` and `ConsequenceChain` as specified in
//! `.scaffold-contributor.toml` to support cascading dynamic/static rule dispatch
//! and cascading chain execution.

use super::curves::{CurveId, CurveTable};
use super::query::{Consequence, InteractionQuery};
use super::tags::*;
use serde::{Deserialize, Serialize};

/// High-level representation of a simulation cell's local state.
/// Used to derive structured `InteractionQuery` values deterministically.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct LocalSimState {
    /// Temperature in milli-Celsius (e.g. 20,000 = 20°C, 1,200,000 = 1200°C lava).
    pub temp_milli: i32,
    /// Moisture in Permyriad (0..=10_000, 10_000 = fully saturated).
    pub moisture_pmy: u16,
    /// Moon phase index (0..=12).
    pub celestial_moon_phase: u8,
    /// Is it daytime?
    pub celestial_day_night: bool,
    /// Material ID of the cell.
    pub material_id: u16,
    /// Faction owning/observing the cell.
    pub faction: u8,
    /// Relationship status: blessed=0, neutral=128, cursed=255.
    pub relationship: u8,
    /// Current degradation or growth stage of the cell (0..=255).
    pub current_degradation_state: u8,
}

impl LocalSimState {
    /// Construct a new default simulation state.
    pub fn new(material_id: u16) -> Self {
        Self {
            temp_milli: 20_000, // 20°C ambient
            moisture_pmy: 0,
            celestial_moon_phase: 0,
            celestial_day_night: true,
            material_id,
            faction: 0,
            relationship: 128, // neutral
            current_degradation_state: 0,
        }
    }

    /// Set temperature on the state.
    pub fn with_temp(mut self, temp_milli: i32) -> Self {
        self.temp_milli = temp_milli;
        self
    }

    /// Set moisture on the state.
    pub fn with_moisture(mut self, moisture_pmy: u16) -> Self {
        self.moisture_pmy = moisture_pmy;
        self
    }

    /// Set celestial context on the state.
    pub fn with_celestial(mut self, moon_phase: u8, day_night: bool) -> Self {
        self.celestial_moon_phase = moon_phase;
        self.celestial_day_night = day_night;
        self
    }

    /// Set relationship status.
    pub fn with_relationship(mut self, relationship: u8) -> Self {
        self.relationship = relationship;
        self
    }

    /// Translates environmental variables into an `InteractionQuery` under a specific
    /// source/target family-and-tag configuration.
    pub fn to_query(&self, src_family: u8, src_tag: u8, tgt_family: u8, tgt_tag: u8) -> InteractionQuery {
        // Compute resonance based on environment and tags
        let mut resonance_pmy = 5000; // default base resonance

        if src_family == SRC_FAMILY_SOUND && tgt_family == TGT_FAMILY_TERRAIN {
            // Sound resonance is highly dependent on material. Stone resonates perfectly.
            if self.material_id == 1 {
                resonance_pmy = 10_000;
            } else {
                resonance_pmy = 2_000;
            }
        } else if src_family == SRC_FAMILY_FIRE {
            // Heat and low moisture increase combustion potential
            let temp_factor = (self.temp_milli.max(0) as u32).min(100_000) / 10; // 0..=10_000
            let dry_factor = 10_000 - self.moisture_pmy;
            resonance_pmy = (((temp_factor + dry_factor as u32) / 2) as u16).min(10_000);
        } else if src_family == SRC_FAMILY_FLUID {
            // Fluid interactions scale with moisture
            resonance_pmy = self.moisture_pmy;
        }

        // Pack celestial context: MSB nibble = moon phase, LSB nibble bit 0 = day/night
        let mut context_celestial = (self.celestial_moon_phase & 0x0F) << 4;
        if self.celestial_day_night {
            context_celestial |= 0x01;
        }

        // Derive velocity: higher moisture or celestial state acts as a velocity booster for fluids
        let velocity_pmy = if src_family == SRC_FAMILY_FLUID {
            (self.moisture_pmy / 40) as u8 // 0..=250
        } else {
            0
        };

        InteractionQuery {
            source_tag: src_tag,
            source_family: src_family,
            target_tag: tgt_tag,
            target_family: tgt_family,
            intensity_pmy: 10_000, // full base intensity
            material_id: self.material_id,
            resonance_pmy,
            target_state: self.current_degradation_state,
            chain_depth: 0,
            context_celestial,
            velocity_pmy,
            faction: self.faction,
            relationship: self.relationship,
        }
    }
}

/// Trigger condition types supporting dynamic and static rule dispatch.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum TriggerCondition {
    /// No verification needed; fires unconditionally.
    None,
    /// Counter threshold must be exceeded.
    CounterThresholdPassed {
        /// The counter threshold that must be passed.
        threshold: u32,
    },
    /// A specific target degradation state must be reached or exceeded.
    StateReached {
        /// State index (0..=255)
        state: u8,
    },
    /// A specific combination of source and target families is targeted.
    TagCombo {
        /// Expected source family.
        src_family: u8,
        /// Expected target family.
        tgt_family: u8,
    },
}

/// Asset Schema representing a Consequence Rule mapping trigger conditions to effects.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct ConsequenceRule {
    /// Unique identifier for this rule.
    pub rule_id: u32,
    /// Condition required to trigger this rule.
    pub trigger_condition: TriggerCondition,
    /// Optional target curve associated with the rule.
    pub curve_id: Option<CurveId>,
    /// Base intensity scaling for consequence propagation (0..=10_000).
    pub base_intensity_pmy: u16,
    /// Loss of energy on chain hop (0..=10_000 Permyriad).
    pub chain_decay_pmy: u16,
}

impl ConsequenceRule {
    /// Creates a new rule with a specific ID and trigger condition.
    pub fn new(rule_id: u32, trigger_condition: TriggerCondition) -> Self {
        Self {
            rule_id,
            trigger_condition,
            curve_id: None,
            base_intensity_pmy: 10_000,
            chain_decay_pmy: 1_000, // 10% decay by default
        }
    }

    /// Bind a CurveId to the rule.
    pub fn with_curve(mut self, curve_id: CurveId) -> Self {
        self.curve_id = Some(curve_id);
        self
    }

    /// Set propagation parameters.
    pub fn with_propagation(mut self, intensity: u16, decay: u16) -> Self {
        self.base_intensity_pmy = intensity;
        self.chain_decay_pmy = decay;
        self
    }

    /// Evaluate if this rule triggers given cell state and current evaluation values.
    pub fn evaluates_trigger(&self, counter: u32, current_state: u8, query: &InteractionQuery) -> bool {
        match self.trigger_condition {
            TriggerCondition::None => true,
            TriggerCondition::CounterThresholdPassed { threshold } => counter >= threshold,
            TriggerCondition::StateReached { state } => current_state >= state,
            TriggerCondition::TagCombo { src_family, tgt_family } => {
                query.source_family == src_family && query.target_family == tgt_family
            }
        }
    }
}

/// Chain of multiple consequence rules representing a cascading propagation system.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct ConsequenceChain {
    /// Unique identifier for the chain.
    pub chain_id: u32,
    /// List of ordered consequence rules.
    pub rules: Vec<ConsequenceRule>,
    /// Limit of cascading steps (hops) to prevent infinite loops.
    pub max_depth: u8,
}

impl ConsequenceChain {
    /// Create a new empty consequence chain.
    pub fn new(chain_id: u32, max_depth: u8) -> Self {
        Self {
            chain_id,
            rules: Vec::new(),
            max_depth,
        }
    }

    /// Add a rule to the chain.
    pub fn add_rule(&mut self, rule: ConsequenceRule) {
        self.rules.push(rule);
    }

    /// Evaluates the cascading propagation of the chain starting from an initial query and local simulation state.
    /// Simulates energy decays per hop, stepping counters, and returning the full log of generated consequences.
    pub fn evaluate(
        &self,
        table: &CurveTable,
        initial_query: &InteractionQuery,
        state: &LocalSimState,
    ) -> Vec<Consequence> {
        let mut results = Vec::new();
        let mut current_query = *initial_query;
        let mut current_counter = 0u32;
        let mut current_state = state.current_degradation_state;

        for (hop, rule) in self.rules.iter().enumerate() {
            if hop as u8 >= self.max_depth || current_query.chain_depth >= self.max_depth {
                break;
            }

            // Verify trigger condition
            if !rule.evaluates_trigger(current_counter, current_state, &current_query) {
                continue;
            }

            // Resolve curve ID from rule or lookup
            let curve_id = rule.curve_id.unwrap_or_else(|| {
                table.lookup(
                    current_query.source_family,
                    current_query.source_tag,
                    current_query.target_family,
                    current_query.target_tag,
                )
            });

            if let Some(curve) = table.get(curve_id) {
                // Decay query intensity based on rule's chain decay and hop depth
                let decay = rule.chain_decay_pmy;
                let new_intensity = (current_query.intensity_pmy as u32)
                    .saturating_mul(10_000 - decay as u32) / 10_000;
                current_query.intensity_pmy = (new_intensity as u16).min(10_000);
                current_query.chain_depth = current_query.chain_depth.saturating_add(1);

                // Simulate step
                if let Some(conseq) = curve.step(
                    &mut current_counter,
                    &mut current_state,
                    current_query.intensity_pmy,
                    current_query.resonance_pmy,
                ) {
                    results.push(conseq);
                }
            } else {
                break; // Unresolved curve stops the cascade
            }
        }

        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::curves::CurveTable;

    #[test]
    fn test_simulation_state_translation() {
        let state = LocalSimState::new(1) // Stone
            .with_temp(50_000)
            .with_moisture(8_000)
            .with_celestial(4, false);

        // Stone + sound should trigger full resonance
        let query = state.to_query(SRC_FAMILY_SOUND, SRC_SOUND, TGT_FAMILY_TERRAIN, TGT_STONE);
        assert_eq!(query.resonance_pmy, 10_000);
        assert_eq!(query.material_id, 1);
        assert_eq!(query.context_celestial, (4 << 4)); // moon=4, day=false (0)

        // Wood + sound should trigger low resonance
        let wood_state = LocalSimState::new(2).with_moisture(2_000);
        let query_wood = wood_state.to_query(SRC_FAMILY_SOUND, SRC_SOUND, TGT_FAMILY_TERRAIN, TGT_STONE);
        assert_eq!(query_wood.resonance_pmy, 2_000);
    }

    #[test]
    fn test_trigger_conditions() {
        let rule_none = ConsequenceRule::new(1, TriggerCondition::None);
        let rule_counter = ConsequenceRule::new(2, TriggerCondition::CounterThresholdPassed { threshold: 1_000 });
        let rule_state = ConsequenceRule::new(3, TriggerCondition::StateReached { state: 2 });
        let rule_tag = ConsequenceRule::new(4, TriggerCondition::TagCombo {
            src_family: SRC_FAMILY_FLUID,
            tgt_family: TGT_FAMILY_TERRAIN,
        });

        let query = InteractionQuery {
            source_family: SRC_FAMILY_FLUID,
            target_family: TGT_FAMILY_TERRAIN,
            ..InteractionQuery::default()
        };

        assert!(rule_none.evaluates_trigger(0, 0, &query));
        assert!(!rule_counter.evaluates_trigger(500, 0, &query));
        assert!(rule_counter.evaluates_trigger(1200, 0, &query));
        assert!(!rule_state.evaluates_trigger(0, 1, &query));
        assert!(rule_state.evaluates_trigger(0, 2, &query));
        assert!(rule_tag.evaluates_trigger(0, 0, &query));
    }

    #[test]
    fn test_consequence_chain_evaluation() {
        let table = CurveTable::full();
        let state = LocalSimState::new(1).with_moisture(10_000); // Stone, full moisture

        // Water on stone curve (ID 0) needs thresholds: [800, 1_600, 2_400, 3_200, 4_000, 0, 0, 0]
        // Let's build a chain of rules that execute water-on-stone multiple times
        let mut chain = ConsequenceChain::new(100, 5);

        let rule1 = ConsequenceRule::new(10, TriggerCondition::None)
            .with_curve(0)
            .with_propagation(10_000, 500); // 5% decay

        let rule2 = ConsequenceRule::new(11, TriggerCondition::None)
            .with_curve(0)
            .with_propagation(10_000, 500);

        chain.add_rule(rule1);
        chain.add_rule(rule2);

        let query = state.to_query(SRC_FAMILY_FLUID, SRC_WATER_FLOW, TGT_FAMILY_TERRAIN, TGT_STONE);
        let consequences = chain.evaluate(&table, &query, &state);

        // Step increases counter. Let's make sure things run without panic
        assert_eq!(consequences.len(), 0); // thresholds not crossed immediately in just 2 quick single ticks with small base rate
    }
}
