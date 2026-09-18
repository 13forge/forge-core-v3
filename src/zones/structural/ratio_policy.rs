//! Ratio Role Policy — prevents misuse of constructive ratios.
//!
//! Ad quadratum (sqrt(2)) owns horizontal plans + lateral containment.
//! Ad triangulum (sqrt(3)) owns vertical elevation + spire tapering.

use crate::zones::structural::ratio::ConstructiveRatio;
use crate::zones::structural::semantic::GeometrySystem;

/// Axis role that a ratio is permitted to serve.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AxisRole {
    /// The horizontal footprint/plan layout.
    HorizontalPlan,
    /// Lateral thrust containment (e.g. buttress cross-section).
    LateralContainment,
    /// Vertical elevation / height.
    VerticalElevation,
    /// Spire taper profile.
    SpireTaper,
    /// Vault rib routing.
    VaultRibRouting,
}

/// Policy entry: which axes a ratio may serve.
struct RatioPolicy {
    ratio: ConstructiveRatio,
    permitted: &'static [AxisRole],
}

const POLICIES: &[RatioPolicy] = &[
    RatioPolicy { ratio: ConstructiveRatio::Sqrt2, permitted: &[AxisRole::HorizontalPlan, AxisRole::LateralContainment] },
    RatioPolicy { ratio: ConstructiveRatio::Sqrt2Over2, permitted: &[AxisRole::HorizontalPlan, AxisRole::LateralContainment] },
    RatioPolicy { ratio: ConstructiveRatio::AdQuadratumDiagonal, permitted: &[AxisRole::HorizontalPlan, AxisRole::LateralContainment] },
    RatioPolicy { ratio: ConstructiveRatio::Sqrt3, permitted: &[AxisRole::VerticalElevation, AxisRole::SpireTaper, AxisRole::VaultRibRouting] },
    RatioPolicy { ratio: ConstructiveRatio::EquilateralAltitude, permitted: &[AxisRole::VerticalElevation, AxisRole::SpireTaper, AxisRole::VaultRibRouting] },
    RatioPolicy { ratio: ConstructiveRatio::AdTriangulumAltitude, permitted: &[AxisRole::VerticalElevation, AxisRole::SpireTaper, AxisRole::VaultRibRouting] },
    RatioPolicy { ratio: ConstructiveRatio::One, permitted: &[AxisRole::HorizontalPlan, AxisRole::VerticalElevation, AxisRole::LateralContainment, AxisRole::SpireTaper, AxisRole::VaultRibRouting] },
    RatioPolicy { ratio: ConstructiveRatio::Half, permitted: &[AxisRole::HorizontalPlan, AxisRole::VerticalElevation, AxisRole::LateralContainment, AxisRole::SpireTaper, AxisRole::VaultRibRouting] },
];

/// Check if a ratio is permitted for a given axis role.
pub fn check_ratio_role(ratio: ConstructiveRatio, axis: AxisRole) -> Result<(), &'static str> {
    for policy in POLICIES {
        if policy.ratio == ratio {
            if policy.permitted.contains(&axis) {
                return Ok(());
            } else {
                return Err("ratio not permitted for this axis role");
            }
        }
    }
    Ok(())
}

/// Validate that a geometry system uses appropriate ratios.
pub fn validate_geometry_ratios(system: GeometrySystem, ratio: ConstructiveRatio) -> Result<(), &'static str> {
    match system {
        GeometrySystem::AdQuadratum => {
            check_ratio_role(ratio, AxisRole::HorizontalPlan)
        }
        GeometrySystem::AdTriangulum | GeometrySystem::PointedArch => {
            check_ratio_role(ratio, AxisRole::VerticalElevation)
        }
        GeometrySystem::CentralThird => {
            check_ratio_role(ratio, AxisRole::LateralContainment)
        }
        GeometrySystem::TasDeCharge => {
            check_ratio_role(ratio, AxisRole::VaultRibRouting)
        }
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sqrt2_allowed_horizontal() {
        assert!(check_ratio_role(ConstructiveRatio::Sqrt2, AxisRole::HorizontalPlan).is_ok());
    }

    #[test]
    fn sqrt2_denied_vertical() {
        assert!(check_ratio_role(ConstructiveRatio::Sqrt2, AxisRole::VerticalElevation).is_err());
    }

    #[test]
    fn sqrt3_allowed_vertical() {
        assert!(check_ratio_role(ConstructiveRatio::Sqrt3, AxisRole::VerticalElevation).is_ok());
    }

    #[test]
    fn sqrt3_denied_horizontal() {
        assert!(check_ratio_role(ConstructiveRatio::Sqrt3, AxisRole::HorizontalPlan).is_err());
    }

    #[test]
    fn ad_quadratum_system_rejects_sqrt3() {
        assert!(validate_geometry_ratios(GeometrySystem::AdQuadratum, ConstructiveRatio::Sqrt3).is_err());
    }

    #[test]
    fn ad_triangulum_system_rejects_sqrt2() {
        assert!(validate_geometry_ratios(GeometrySystem::AdTriangulum, ConstructiveRatio::Sqrt2).is_err());
    }

    #[test]
    fn central_third_accepts_sqrt2() {
        assert!(validate_geometry_ratios(GeometrySystem::CentralThird, ConstructiveRatio::Sqrt2).is_ok());
    }
}
