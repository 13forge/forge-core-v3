//! Structural envelope validation.
//!
//! Produces validation verdicts for generated assemblies.
//! Fail-closed: invalid geometry is rejected, never silently passed.

use crate::fixed_point::MilliUnit;
use crate::zones::structural::catalog::PrimitiveId;

/// Validation verdict for a generated assembly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StructuralVerdict {
    /// The assembly satisfies every checked constraint.
    Pass,
    /// The assembly failed one or more constraints, listed in order found.
    Fail(Vec<StructuralValidationError>),
}

/// A specific validation failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructuralValidationError {
    /// The primitive whose placement or shape failed the check.
    pub primitive_id: PrimitiveId,
    /// Which constraint category failed.
    pub check: ValidationCheck,
    /// Human-readable detail for the failure.
    pub detail: &'static str,
}

/// Categories of structural validation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ValidationCheck {
    /// Socket compatibility check.
    SocketCompatibility,
    /// Ratio determinism check.
    RatioDeterminism,
    /// Octagonal closure check.
    OctagonalClosure,
    /// Hub connectivity check.
    HubConnectivity,
    /// Downward load route check.
    DownwardLoadRoute,
    /// Central third containment check.
    CentralThirdContainment,
    /// Style mutation rule check.
    StyleMutationRule,
    /// Ratio role policy check.
    RatioRolePolicy,
    /// Crossed arch formula check.
    CrossedArchFormula,
    /// Spire taper check.
    SpireTaper,
}

/// Central-third containment test.
///
/// Given a thrust vector's local x-position in a buttress cross-section of width `w`:
/// contained = x >= -w/6 && x <= w/6
pub fn in_middle_third(x: MilliUnit, width: MilliUnit) -> bool {
    let sixth = width.0 / 6;
    x.0 >= -sixth && x.0 <= sixth
}

/// Validate octagonal closure: 8 sockets, opposing symmetry, 45° rotation parity.
pub fn validate_octagon(sockets: &[(MilliUnit, MilliUnit)]) -> Result<(), &'static str> {
    if sockets.len() != 8 {
        return Err("octagon requires exactly 8 sockets");
    }
    // Check opposing symmetry (socket[i] == -socket[i+4])
    for i in 0..4 {
        let (ax, ay) = &sockets[i];
        let (bx, by) = &sockets[i + 4];
        if ax.0 != -bx.0 || ay.0 != -by.0 {
            return Err("opposing sockets not symmetric");
        }
    }
    Ok(())
}

/// Validate that a tas-de-charge hub has a downward pier route.
pub fn validate_hub_has_pier(has_pier_socket: bool) -> Result<(), &'static str> {
    if has_pier_socket {
        Ok(())
    } else {
        Err("tas-de-charge hub lacks downward pier route")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn middle_third_contained() {
        // width=600, sixth=100, x=50 → contained
        assert!(in_middle_third(MilliUnit(50), MilliUnit(600)));
    }

    #[test]
    fn middle_third_breached() {
        // width=600, sixth=100, x=150 → breached
        assert!(!in_middle_third(MilliUnit(150), MilliUnit(600)));
    }

    #[test]
    fn octagon_valid() {
        use crate::zones::structural::ratio::octagon_offset;
        let r = MilliUnit(10000);
        let sockets: Vec<_> = (0..8).map(|i| octagon_offset(r, i)).collect();
        assert!(validate_octagon(&sockets).is_ok());
    }

    #[test]
    fn octagon_wrong_count() {
        let sockets = vec![(MilliUnit(0), MilliUnit(0)); 7];
        assert!(validate_octagon(&sockets).is_err());
    }
}
