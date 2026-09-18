#![deny(missing_docs)]

/// Which constructive geometry system governs this socket.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GeometrySystem {
    /// Ad quadratum proportional system.
    AdQuadratum,
    /// Ad triangulum proportional system.
    AdTriangulum,
    /// Crossed arch structural form.
    CrossedArch,
    /// Pointed arch structural form.
    PointedArch,
    /// Tas-de-charge (multi-rib convergence) system.
    TasDeCharge,
    /// Central-third containment geometry.
    CentralThird,
    /// Generic/unspecialized system.
    Generic,
}

/// Structural connection role at this socket.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ConnectionRole {
    /// Base-plan node of primary structure.
    BasePlanNode,
    /// Vertex of octagonal plan.
    OctagonVertex,
    /// Central-plan crossing tower.
    CrossingTowerNode,
    /// Dome support ring node.
    DomeSupport,
    /// Vault rib start point.
    RibStart,
    /// Vault rib end point.
    RibEnd,
    /// Pointed arch springing point.
    PointedArchSpring,
    /// Pointed arch apex.
    PointedArchApex,
    /// Vertical elevation guide line.
    VerticalElevationGuide,
    /// Spire base attachment.
    SpireBase,
    /// Spire tip termination.
    SpireTip,
    /// Formeret (vault edge rib).
    Formeret,
    /// Transverse arch crossing.
    TransverseArch,
    /// Diagonal vault rib.
    DiagonalRib,
    /// Compound pier node.
    CompoundPier,
    /// Tas-de-charge hub convergence.
    TasDeChargeHub,
    /// Flying buttress thrust receiver.
    ButtressReceiver,
    /// Flying buttress foot/base.
    ButtressFoot,
    /// Glass wall span segment.
    GlassWallSpan,
    /// Generic mount point.
    GenericMount,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn construct_all_geometry_variants() {
        let _ = GeometrySystem::AdQuadratum;
        let _ = GeometrySystem::AdTriangulum;
        let _ = GeometrySystem::CrossedArch;
        let _ = GeometrySystem::PointedArch;
        let _ = GeometrySystem::TasDeCharge;
        let _ = GeometrySystem::CentralThird;
        let _ = GeometrySystem::Generic;
    }

    #[test]
    fn construct_all_connection_variants() {
        let _ = ConnectionRole::BasePlanNode;
        let _ = ConnectionRole::OctagonVertex;
        let _ = ConnectionRole::CrossingTowerNode;
        let _ = ConnectionRole::DomeSupport;
        let _ = ConnectionRole::RibStart;
        let _ = ConnectionRole::RibEnd;
        let _ = ConnectionRole::PointedArchSpring;
        let _ = ConnectionRole::PointedArchApex;
        let _ = ConnectionRole::VerticalElevationGuide;
        let _ = ConnectionRole::SpireBase;
        let _ = ConnectionRole::SpireTip;
        let _ = ConnectionRole::Formeret;
        let _ = ConnectionRole::TransverseArch;
        let _ = ConnectionRole::DiagonalRib;
        let _ = ConnectionRole::CompoundPier;
        let _ = ConnectionRole::TasDeChargeHub;
        let _ = ConnectionRole::ButtressReceiver;
        let _ = ConnectionRole::ButtressFoot;
        let _ = ConnectionRole::GlassWallSpan;
        let _ = ConnectionRole::GenericMount;
    }

    #[test]
    fn geometry_equality() {
        assert_eq!(GeometrySystem::Generic, GeometrySystem::Generic);
        assert_ne!(GeometrySystem::AdQuadratum, GeometrySystem::AdTriangulum);
    }

    #[test]
    fn connection_role_equality() {
        assert_eq!(ConnectionRole::CompoundPier, ConnectionRole::CompoundPier);
        assert_ne!(ConnectionRole::BasePlanNode, ConnectionRole::OctagonVertex);
    }
}
