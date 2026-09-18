#![deny(missing_docs)]

use super::semantic::{ConnectionRole, GeometrySystem};

/// Unique identifier for a catalog primitive.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PrimitiveId(pub u32);

/// Socket definition within a primitive catalog entry.
#[derive(Debug, Clone, Copy)]
pub struct SocketDef {
    /// Connection role at this socket.
    pub role: ConnectionRole,
    /// Load role at this socket.
    pub load: LoadRole,
    /// Local x-offset in milliunits.
    pub offset_x: i64,
    /// Local y-offset in milliunits.
    pub offset_y: i64,
}

/// Load-path role emitted or accepted by a socket.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LoadRole {
    /// No load transfer.
    None,
    /// Downward compression load.
    CompressionDown,
    /// Lateral thrust outward.
    LateralThrustOut,
    /// Consolidated hub load.
    ConsolidatedHubLoad,
    /// Buttress containment load.
    ButtressContainment,
    /// Downward compression hint.
    DownwardCompressionHint,
    /// Reduced lateral thrust hint.
    ReducedLateralThrustHint,
    /// Decorative (no structural load).
    DecorativeOnly,
}

/// A catalog entry describing an architectural primitive's socket layout.
#[derive(Debug, Clone)]
pub struct ArchPrimitiveDef {
    /// Unique primitive identifier.
    pub id: PrimitiveId,
    /// Human-readable name.
    pub name: &'static str,
    /// Architectural style family.
    pub style: StyleFamily,
    /// Constructive geometry system.
    pub geometry: GeometrySystem,
    /// Socket definitions on this primitive.
    pub socket_defs: &'static [SocketDef],
}

/// Architectural style family for generation routing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StyleFamily {
    /// Islamic geometric proportions.
    Islamic,
    /// Templar military architecture.
    Templar,
    /// Gothic pointed-arch style.
    Gothic,
    /// Hybrid style (with variant tag).
    Hybrid(u16),
    /// Generic/unspecialized style.
    Generic,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn primitive_id_construction() {
        let id = PrimitiveId(5);
        assert_eq!(id.0, 5);
    }

    #[test]
    fn primitive_id_equality() {
        assert_eq!(PrimitiveId(42), PrimitiveId(42));
        assert_ne!(PrimitiveId(1), PrimitiveId(2));
    }

    #[test]
    fn socket_def_construction() {
        let socket = SocketDef {
            role: ConnectionRole::CompoundPier,
            load: LoadRole::CompressionDown,
            offset_x: 100,
            offset_y: 200,
        };
        assert_eq!(socket.offset_x, 100);
        assert_eq!(socket.offset_y, 200);
    }

    #[test]
    fn arch_primitive_def_construction() {
        const SOCKETS: &[SocketDef] = &[SocketDef {
            role: ConnectionRole::BasePlanNode,
            load: LoadRole::CompressionDown,
            offset_x: 0,
            offset_y: 0,
        }];
        let def = ArchPrimitiveDef {
            id: PrimitiveId(1),
            name: "test_primitive",
            style: StyleFamily::Gothic,
            geometry: GeometrySystem::Generic,
            socket_defs: SOCKETS,
        };
        assert_eq!(def.id, PrimitiveId(1));
        assert_eq!(def.name, "test_primitive");
    }

    #[test]
    fn load_role_variants() {
        let _ = LoadRole::None;
        let _ = LoadRole::CompressionDown;
        let _ = LoadRole::LateralThrustOut;
        let _ = LoadRole::ConsolidatedHubLoad;
        let _ = LoadRole::ButtressContainment;
        let _ = LoadRole::DownwardCompressionHint;
        let _ = LoadRole::ReducedLateralThrustHint;
        let _ = LoadRole::DecorativeOnly;
    }

    #[test]
    fn style_family_variants() {
        let _ = StyleFamily::Islamic;
        let _ = StyleFamily::Templar;
        let _ = StyleFamily::Gothic;
        let _ = StyleFamily::Hybrid(1);
        let _ = StyleFamily::Generic;
    }
}
