//! Central-Third Buttress Solver.
//!
//! Scales a flying buttress profile until the thrust vector
//! is contained within the middle third of the cross-section.
//! Uses ad quadratum (sqrt(2)) scaling steps.

use crate::fixed_point::MilliUnit;
use crate::zones::structural::ratio::{self, ConstructiveRatio};
use crate::zones::structural::validate;

/// Thrust vector from a tas-de-charge hub.
#[derive(Debug, Clone, Copy)]
pub struct ThrustVector {
    /// Origin x-coordinate.
    pub origin_x: MilliUnit,
    /// Origin y-coordinate.
    pub origin_y: MilliUnit,
    /// Direction x-component.
    pub direction_x: MilliUnit,
    /// Direction y-component.
    pub direction_y: MilliUnit,
    /// Magnitude of thrust.
    pub magnitude: MilliUnit,
}

/// Buttress cross-section profile.
#[derive(Debug, Clone, Copy)]
pub struct ButtressProfile {
    /// Width of cross-section.
    pub width: MilliUnit,
    /// Height of cross-section.
    pub height: MilliUnit,
    /// Position x-coordinate.
    pub position_x: i64,
    /// Position y-coordinate.
    pub position_y: i64,
}

/// Record of one scaling attempt.
#[derive(Debug, Clone)]
pub struct ButtressProfileTest {
    /// Iteration number.
    pub iteration: u8,
    /// Width at this iteration.
    pub width: MilliUnit,
    /// Projection value.
    pub projection: MilliUnit,
    /// Whether thrust is contained.
    pub contained: bool,
}

/// Full trace of the containment solve.
#[derive(Debug, Clone)]
pub struct ContainmentTrace {
    /// All test iterations.
    pub tests: Vec<ButtressProfileTest>,
    /// Final verdict.
    pub verdict: ContainmentVerdict,
    /// Final profile state.
    pub final_profile: ButtressProfile,
}

/// Outcome of the containment solve.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContainmentVerdict {
    /// Thrust is contained within middle third.
    Contained,
    /// Width exceeded budget before containment.
    BreachedBudget,
}

const MAX_ITERATIONS: u8 = 8;

/// Project thrust vector onto buttress local x-axis (simplified 2D projection).
fn project_thrust(thrust: &ThrustVector, profile: &ButtressProfile) -> MilliUnit {
    // Simplified: lateral component relative to buttress center
    let dx = thrust.origin_x.0 + thrust.direction_x.0 - profile.position_x;
    MilliUnit(dx * thrust.magnitude.0 / 10000)
}

/// Solve buttress containment by scaling width until thrust is in middle third.
pub fn solve(mut profile: ButtressProfile, thrust: &ThrustVector, max_width: MilliUnit) -> ContainmentTrace {
    let mut tests = Vec::with_capacity(MAX_ITERATIONS as usize);

    for iteration in 0..MAX_ITERATIONS {
        let projection = project_thrust(thrust, &profile);
        let contained = validate::in_middle_third(projection, profile.width);

        tests.push(ButtressProfileTest {
            iteration,
            width: profile.width,
            projection,
            contained,
        });

        if contained {
            return ContainmentTrace {
                tests,
                verdict: ContainmentVerdict::Contained,
                final_profile: profile,
            };
        }

        // Scale width by sqrt(2) (ad quadratum step)
        let new_width = ratio::resolve(ConstructiveRatio::Sqrt2, profile.width);
        if new_width.0 > max_width.0 {
            return ContainmentTrace {
                tests,
                verdict: ContainmentVerdict::BreachedBudget,
                final_profile: profile,
            };
        }
        profile.width = new_width;
    }

    ContainmentTrace {
        tests,
        verdict: ContainmentVerdict::BreachedBudget,
        final_profile: profile,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contained_immediately() {
        let profile = ButtressProfile { width: MilliUnit(6000), height: MilliUnit(10000), position_x: 0, position_y: 0 };
        let thrust = ThrustVector {
            origin_x: MilliUnit(0), origin_y: MilliUnit(5000),
            direction_x: MilliUnit(100), direction_y: MilliUnit(0),
            magnitude: MilliUnit(500),
        };
        let trace = solve(profile, &thrust, MilliUnit(50000));
        assert_eq!(trace.verdict, ContainmentVerdict::Contained);
        assert_eq!(trace.tests.len(), 1);
    }

    #[test]
    fn scales_then_contains() {
        let profile = ButtressProfile { width: MilliUnit(100), height: MilliUnit(10000), position_x: 0, position_y: 0 };
        let thrust = ThrustVector {
            origin_x: MilliUnit(0), origin_y: MilliUnit(5000),
            direction_x: MilliUnit(500), direction_y: MilliUnit(0),
            magnitude: MilliUnit(1000),
        };
        let trace = solve(profile, &thrust, MilliUnit(50000));
        assert_eq!(trace.verdict, ContainmentVerdict::Contained);
        assert!(trace.tests.len() > 1);
    }

    #[test]
    fn breaches_budget() {
        let profile = ButtressProfile { width: MilliUnit(100), height: MilliUnit(10000), position_x: 0, position_y: 0 };
        let thrust = ThrustVector {
            origin_x: MilliUnit(0), origin_y: MilliUnit(5000),
            direction_x: MilliUnit(50000), direction_y: MilliUnit(0),
            magnitude: MilliUnit(10000),
        };
        let trace = solve(profile, &thrust, MilliUnit(500));
        assert_eq!(trace.verdict, ContainmentVerdict::BreachedBudget);
    }
}
