//! The Blueprint authoring schema — canonical home (L05), moved here from
//! `forge-zones-v3` 2026-08-20 so every crate can reach it without a new
//! dependency edge (`forge-core-v3` is the workspace `dag_root`, L06).
//! Ported originally from `F:\NewRepo\crates\forge-tile-crawler\src\
//! architecture\blueprint.rs`. `MilliUnit`-typed throughout, zero floats.
//! `serde` derives dropped — this crate is dependency-free by law;
//! `forge-zones-v3` re-exports this module for its own JSON-facing callers.
//!
//! **Scope cut (L15, named plainly, carried over):** `BlueprintDocument::
//! constraints`/`validation` (v2: `Option<ConstraintSet>`/
//! `Option<ValidationReport>`) are cut — nothing downstream reads them.

use crate::fixed_point::MilliUnit;

// ─── Coordinate Primitives ───────────────────────────────────────────

/// 2D position in MilliUnits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Pos2D {
    /// Horizontal offset.
    pub x: MilliUnit,
    /// Vertical (top-down) offset.
    pub y: MilliUnit,
}

/// 2D axis-aligned rectangle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Rect2D {
    /// Left edge.
    pub x: MilliUnit,
    /// Top edge (top-down convention).
    pub y: MilliUnit,
    /// Width.
    pub w: MilliUnit,
    /// Height (top-down convention).
    pub h: MilliUnit,
}

/// 3D position in MilliUnits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Vec3M {
    /// X axis.
    pub x: MilliUnit,
    /// Y axis (vertical).
    pub y: MilliUnit,
    /// Z axis.
    pub z: MilliUnit,
}

/// 3D axis-aligned bounding box.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Bounds3D {
    /// The box's minimum corner.
    pub origin: Vec3M,
    /// The box's extent along each axis.
    pub size: Vec3M,
}

// ─── Blueprint Mode ──────────────────────────────────────────────────

/// Whether a blueprint authors 2D (side-scroll/tilemap) or 3D geometry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BlueprintMode {
    /// Flat, top-down or side-scroll authoring.
    Mode2D,
    /// Full volumetric authoring.
    Mode3D,
}

// ─── Authoring State ─────────────────────────────────────────────────

/// How far a blueprint has moved through its authoring pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AuthoringState {
    /// Procedurally generated, unreviewed.
    Generated,
    /// A human has made changes since generation.
    Edited,
    /// Reviewed and signed off, not yet locked.
    Approved,
    /// Locked into the build; further edits start a new variant.
    Committed,
}

// ─── Metadata ────────────────────────────────────────────────────────

/// Atlas-linkage and lore metadata a blueprint document carries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlueprintMeta {
    /// The blueprint's display name.
    pub name: String,
    /// The zone id this blueprint authors.
    pub zone: String,
    /// Atlas linkage — which section/chapter/act this blueprint belongs to.
    pub section_id: Option<String>,
    /// Atlas linkage — which scene this blueprint belongs to.
    pub scene_id: Option<String>,
    /// Room number within its scene, if numbered.
    pub room: Option<u32>,
    /// Story act this blueprint belongs to.
    pub act: Option<String>,
    /// Biome tag driving material/parallax defaults.
    pub biome: Option<String>,
    /// Tone/mood tag for audio and lighting defaults.
    pub mood: Option<String>,
    /// Free-form search/filter tags.
    pub tags: Vec<String>,
    /// True when this document is a variant of another blueprint.
    pub is_variant: bool,
    /// The blueprint id this is a variant of, when `is_variant`.
    pub variant_of: Option<String>,
    /// Where this document sits in the authoring pipeline.
    pub authoring_state: AuthoringState,
}

// ─── Blueprint Document (root) ───────────────────────────────────────

/// The authoritative spatial logic for a level/room/zone before geometry,
/// art, lighting, or gameplay instantiation. Blueprint first, beauty
/// second.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlueprintDocument {
    /// The document's own id.
    pub id: String,
    /// Schema revision.
    pub version: u32,
    /// 2D or 3D authoring mode.
    pub mode: BlueprintMode,
    /// Deterministic authoring seed.
    pub seed: u64,
    /// Atlas-linkage and lore metadata.
    pub meta: BlueprintMeta,
    /// The node/edge/layer graph this document authors.
    pub graph: BlueprintGraph,
}

// ─── Graph ───────────────────────────────────────────────────────────

/// Lightweight handle into the node array.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeId(pub u32);

/// Lightweight handle into the edge array.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EdgeId(pub u32);

/// The blueprint's node/edge/layer graph.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlueprintGraph {
    /// Every authored node.
    pub nodes: Vec<BlueprintNode>,
    /// Every authored relationship between nodes.
    pub edges: Vec<BlueprintEdge>,
    /// Authoring-tool visibility/locking groups.
    pub layers: Vec<BlueprintLayer>,
}

// ─── Nodes ───────────────────────────────────────────────────────────

/// One authored spatial or logic element in the blueprint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlueprintNode {
    /// This node's own id.
    pub id: NodeId,
    /// What kind of element this node authors.
    pub node_type: NodeType,
    /// Human-readable label; falls back to a generated name when absent.
    pub label: Option<String>,
    /// Free-form search/filter tags.
    pub tags: Vec<String>,
    /// The node's placement — flat (2D) or volumetric (3D).
    pub bounds: NodeBounds,
}

/// A node's placement, either flat (2D) or volumetric (3D).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NodeBounds {
    /// A flat, top-down rectangle.
    Rect(Rect2D),
    /// A full 3D bounding volume.
    Volume(Bounds3D),
}

/// What kind of element a blueprint node authors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NodeType {
    /// An enclosed, walkable space.
    Room,
    /// A standing surface, not necessarily enclosed.
    Platform,
    /// A hazard the player falls into.
    Pit,
    /// A collision/sightline blocker.
    Wall,
    /// A combat encounter's own zone.
    Arena,
    /// A save/respawn point.
    Checkpoint,
    /// A lore/interaction shrine.
    Shrine,
    /// The zone where an encounter triggers.
    EncounterZone,
    /// A gate the player must satisfy a condition to pass.
    TraversalGate,
    /// A story beat trigger point.
    NarrativeBeat,
    /// A patch of a named biome's flora/terrain.
    BiomePatch,
    /// A dynamic-lighting placement zone.
    LightingZone,
    /// A generic 3D volume with no more specific type.
    Volume,
    /// A connecting passage between rooms.
    Corridor,
    /// A single billboard mob placement (flat sprite, EQ-1999 style).
    MobSpawn,
}

// ─── Edges ───────────────────────────────────────────────────────────

/// A relationship between two blueprint nodes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlueprintEdge {
    /// This edge's own id.
    pub id: EdgeId,
    /// The source node.
    pub from: NodeId,
    /// The destination node.
    pub to: NodeId,
    /// What kind of relationship this edge authors.
    pub edge_type: EdgeType,
}

/// What kind of relationship a blueprint edge authors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EdgeType {
    /// The two nodes are spatially next to each other.
    Adjacent,
    /// The two nodes are traversable between.
    Connects,
    /// Traversal requires a jump.
    RequiresJump,
    /// Traversal requires a climb.
    RequiresClimb,
    /// Traversal requires a key item.
    RequiresKey,
    /// The two nodes have a sightline between them.
    LineOfSight,
    /// An AI patrol route between the two nodes.
    PatrolPath,
    /// This edge is on the main critical path.
    CriticalPath,
    /// This edge is on an optional side path.
    OptionalPath,
    /// The two nodes mirror each other (e.g. a puzzle twin).
    MirrorLink,
    /// Traversal only exists in a specific phase/era.
    PhaseTransition,
    /// Traversal is locked until a boss is defeated.
    BossLock,
}

// ─── Layers ──────────────────────────────────────────────────────────

/// A named grouping of nodes, e.g. for authoring-tool visibility/locking.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlueprintLayer {
    /// This layer's own id.
    pub id: String,
    /// Display name.
    pub name: String,
    /// What kind of content this layer groups.
    pub layer_type: LayerType,
    /// Whether the authoring tool currently shows this layer.
    pub visible: bool,
    /// Whether the authoring tool currently allows editing this layer.
    pub locked: bool,
    /// Node IDs belonging to this layer.
    pub items: Vec<NodeId>,
}

impl BlueprintGraph {
    /// Find all nodes of a given type.
    pub fn nodes_of_type(&self, nt: NodeType) -> Vec<&BlueprintNode> {
        self.nodes.iter().filter(|n| n.node_type == nt).collect()
    }

    /// Find node by ID.
    pub fn node(&self, id: NodeId) -> Option<&BlueprintNode> {
        self.nodes.iter().find(|n| n.id == id)
    }

    /// Get all neighbor node IDs reachable from a given node (outgoing edges).
    pub fn neighbors(&self, id: NodeId) -> Vec<NodeId> {
        self.edges.iter()
            .filter(|e| e.from == id)
            .map(|e| e.to)
            .chain(self.edges.iter().filter(|e| e.to == id).map(|e| e.from))
            .collect()
    }

    /// BFS reachability from a start node. Returns set of reachable NodeIds.
    pub fn reachable_from(&self, start: NodeId) -> Vec<NodeId> {
        let mut visited = Vec::new();
        let mut queue = std::collections::VecDeque::new();
        queue.push_back(start);
        visited.push(start);

        while let Some(current) = queue.pop_front() {
            for neighbor in self.neighbors(current) {
                if !visited.contains(&neighbor) {
                    visited.push(neighbor);
                    queue.push_back(neighbor);
                }
            }
        }
        visited
    }

    /// Find edges of a specific type from a node.
    pub fn edges_of_type_from(&self, id: NodeId, et: EdgeType) -> Vec<NodeId> {
        self.edges.iter()
            .filter(|e| e.from == id && e.edge_type == et)
            .map(|e| e.to)
            .collect()
    }
}

/// What kind of content a blueprint layer groups.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LayerType {
    /// The playable-space geometry.
    PlaySpace,
    /// Collision-only geometry.
    Collision,
    /// Navmesh/pathing geometry.
    Navigation,
    /// Encounter trigger zones.
    Encounter,
    /// Spawn points.
    Spawns,
    /// Decorative props.
    Props,
    /// Lore/narrative markers.
    Lore,
    /// Lighting placements.
    Lighting,
    /// Audio placements.
    Audio,
    /// Parallax background layers.
    Parallax,
    /// Attachment sockets for authored props.
    Sockets,
    /// Authoring-time constraint annotations.
    Constraints,
    /// Free-form authoring notes.
    Annotations,
}
