//! Creation spine — the universal artifact codex (W1 of the ForgeVision Lab weld,
//! knowledge drop `.forge/knowledge-drops/2026-08-18-forgevision-lab-2d3d.md`).
//!
//! Ported from v2 `forge-game-systems::creation` (donors, read before weld:
//! `F:\NewRepo\crates\forge-game-systems\src\creation\{artifact.rs,graph.rs,ledger.rs,ids.rs,hash.rs}`),
//! the same spine v2's world already consumed (`forge-studio\src\world.rs:60-61` —
//! "M9: every generated zone becomes an artifact fact"). Ironroot lineage: this is
//! part of the {dirge-of-ironroot, ironroot-edict, deveraux-game, akgame, astrakey,
//! astrakeyweb, goblin, adevstale, AKWEB} confluence, live at `forge-mud-v3`.
//!
//! Enumerated adaptations (L05 one-home drove every rename):
//! 1. `ArtifactKind` → [`CreationKind`] — `forge-scc-v3::buff::ArtifactKind` (compiler
//!    emits) and `forge-mud-v3::ironroot::scene_loader::EntityKind` (physics kinds)
//!    already hold the neighbouring names.
//! 2. `SourceKind` → [`CreationSource`] — `spine::SourceKind` is already exported at
//!    the crate root.
//! 3. Donor sub-entity ids (`NpcId`/`ZoneId`/`FactionId`/`MotifId`/`SecretId`/
//!    `ItemSetId`/`ItemId`) collapse to [`ArtifactId`] — one id space, typed by
//!    [`CreationKind`]. `ZoneId` alone already had three live definitions
//!    (`ironroot-signal-v3::ids`, `forge-cart-brain-v3::run_dev_run`, mud brain);
//!    a fourth would be a defect, not a port.
//! 4. Donor `Tick` → [`crate::fixed_point::SimTick`] (already home).
//! 5. Donor `hash.rs` folds into [`crate::checksum`] — `mix_u64` is byte-identical
//!    to `fnv1a64_fold`; `event_hash` here is allocation-free (donor built a `Vec`).
//! 6. `GraphPosition` `f32` → [`crate::fixed_point::MilliUnit`] (no floats in core).
//! 7. Payload bridge variants (Npc combat / Item mechanics / Zone biome / Motif
//!    pattern — WIRING-GAPS.md) are DEFERRED to W5: they need `forge-harmonics` /
//!    zones / items crates, and Crate Zero stays zero-dep (L06). [`ArtifactPayload`]
//!    carries only the dep-free variants today.
//! 8. NEW (not donor): [`Genre`], [`GenreBounds`], [`GENRE_BOUNDS`],
//!    [`MAX_CREATION_KIND`] — the ratified caps live as consts asserted by tests
//!    (L01 law-is-test). Numeric values are `Estimate` pending ARCH000 ratification;
//!    the SHAPE (powers of 3, per-genre rows) is settled.

use crate::checksum::{fnv1a64_fold, FNV_OFFSET_BASIS};
use crate::fixed_point::{MilliUnit, SimTick};

// ── Ids ─────────────────────────────────────────────────────────────────────

/// Stable id of one created thing, any [`CreationKind`]. One id space for the
/// whole codex (adaptation 3).
#[repr(transparent)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ArtifactId(pub u64);

/// Stable id of one accepted lore fact in the [`Ledger`].
#[repr(transparent)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct LoreFactId(pub u64);

/// Stable id of one [`GraphEdge`].
#[repr(transparent)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct EdgeId(pub u64);

/// Stable id of one run/timeline branch (forked by [`LedgerEventKind::ForkBranch`]).
#[repr(transparent)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BranchId(pub u64);

/// Stable id of one choice scene (W2 cyoa sieve — consumer:
/// `forge-mud-v3::ironroot::cyoa`).
#[repr(transparent)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SceneId(pub u64);

/// Stable id of one choice inside a scene (W2 cyoa sieve).
#[repr(transparent)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ChoiceId(pub u64);

// ── Kinds and header vocabulary ─────────────────────────────────────────────

/// The twelve creation kinds — the closed taxonomy every generated thing lives in
/// (donor `ArtifactKind`, artifact.rs:9-22). Closed by law: variety comes from
/// composition ([`FactTag`], rarity, scope), never from new variants
/// (forward-only ledger doctrine, `creation_dag.rs`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CreationKind {
    /// A person (toybox: "Person").
    Npc,
    /// A thing (toybox: "Thing").
    Item,
    /// A collection of things (toybox: "Collection").
    ItemSet,
    /// A faction-bound thing (toybox: "Faction Thing").
    FactionItem,
    /// A place (toybox: "Place").
    Zone,
    /// A secret (toybox: "Secret").
    Secret,
    /// A choice scene (toybox: "Choice Scene").
    Scene,
    /// A group (toybox: "Group").
    Faction,
    /// A rumor (toybox: "Rumor").
    Rumor,
    /// A song (toybox: "Song").
    Motif,
    /// A quest (toybox: "Quest").
    Quest,
    /// A world event (toybox: "Event").
    WorldEvent,
}

/// The hard cap on creation kinds. `#2 MaxEntityKind` of the ForgeVision drop:
/// exactly the twelve donor variants, asserted exhaustively in tests.
pub const MAX_CREATION_KIND: usize = 12;

impl CreationKind {
    /// Every kind, in donor declaration order (stable — listings depend on it).
    pub const ALL: [CreationKind; MAX_CREATION_KIND] = [
        CreationKind::Npc,
        CreationKind::Item,
        CreationKind::ItemSet,
        CreationKind::FactionItem,
        CreationKind::Zone,
        CreationKind::Secret,
        CreationKind::Scene,
        CreationKind::Faction,
        CreationKind::Rumor,
        CreationKind::Motif,
        CreationKind::Quest,
        CreationKind::WorldEvent,
    ];

    /// Child-readable toybox label (donor `graph.rs::toybox_label_for`, :106-121).
    pub fn toybox_label(self) -> &'static str {
        match self {
            CreationKind::Npc => "Person",
            CreationKind::Item => "Thing",
            CreationKind::ItemSet => "Collection",
            CreationKind::FactionItem => "Faction Thing",
            CreationKind::Zone => "Place",
            CreationKind::Secret => "Secret",
            CreationKind::Scene => "Choice Scene",
            CreationKind::Faction => "Group",
            CreationKind::Rumor => "Rumor",
            CreationKind::Motif => "Song",
            CreationKind::Quest => "Quest",
            CreationKind::WorldEvent => "Event",
        }
    }
}

/// Lifecycle of one artifact (donor artifact.rs:25-32).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ArtifactStatus {
    /// Cheap, mutable, not yet world-accepted.
    Draft,
    /// Shown to the author for approval, still not world-accepted.
    Preview,
    /// World-accepted; the ledger holds its fact.
    Locked,
    /// Locked AND disclosed to the player.
    Revealed,
    /// Locked but actively hidden again.
    Suppressed,
    /// Retired from play; kept for provenance.
    Archived,
}

/// Who can see an artifact (donor artifact.rs:35-41).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Visibility {
    /// Everyone.
    Public,
    /// Circulates as hearsay.
    Rumor,
    /// Not yet surfaced.
    Hidden,
    /// Gated behind reveal conditions.
    Secret,
    /// Never surfaced to players.
    Forbidden,
}

/// Scarcity tier (donor artifact.rs:44-49).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Rarity {
    /// Baseline.
    Common,
    /// Notable.
    Uncommon,
    /// Scarce.
    Rare,
    /// One per world.
    Unique,
}

/// Spatial/temporal reach of an artifact (donor `Scope`, artifact.rs:52-58).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ArtifactScope {
    /// One spot.
    Local,
    /// One zone.
    Zone,
    /// One faction's holdings.
    Faction,
    /// The whole world.
    World,
    /// This run only; dies with the branch.
    RunOnly,
}

/// Where an artifact came from (donor `SourceKind`, artifact.rs:61-67; renamed —
/// adaptation 2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CreationSource {
    /// Hand-written by a designer.
    Authored,
    /// Seed-derived by a generator.
    Procedural,
    /// Brought in from outside the engine.
    Imported,
    /// Created by a player's choice.
    PlayerChoice,
    /// Emitted by a running system.
    SystemEvent,
}

/// Current holder of an artifact (donor artifact.rs:70-77; sub-ids collapsed to
/// [`ArtifactId`] — adaptation 3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Owner {
    /// Unowned.
    None,
    /// Held by an NPC artifact.
    Npc(ArtifactId),
    /// Held by a faction artifact.
    Faction(ArtifactId),
    /// Bound to a zone artifact.
    Zone(ArtifactId),
    /// Held by the player.
    Player,
    /// Ownership deliberately unresolved.
    Unknown,
}

/// Semantic tag on a fact or artifact (donor artifact.rs:80-87; sub-ids collapsed —
/// adaptation 3).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum FactTag {
    /// Free-text tag.
    Text(String),
    /// Points at a faction artifact.
    Faction(ArtifactId),
    /// Points at a zone artifact.
    Zone(ArtifactId),
    /// Points at a motif artifact.
    Motif(ArtifactId),
    /// Points at a secret artifact.
    Secret(ArtifactId),
    /// Points at an item-set artifact.
    ItemSet(ArtifactId),
}

// ── Header and payloads ─────────────────────────────────────────────────────

/// The universal header every created thing shares, so one UI rail can treat
/// NPCs, zones, secrets, items, songs, factions, scenes, and sets consistently
/// (donor artifact.rs:90-128).
#[derive(Debug, Clone)]
pub struct ArtifactHeader {
    /// Stable id.
    pub id: ArtifactId,
    /// Which of the twelve kinds this is.
    pub kind: CreationKind,
    /// Display name.
    pub name: String,
    /// Lifecycle state.
    pub status: ArtifactStatus,
    /// Who can see it.
    pub visibility: Visibility,
    /// Scarcity tier.
    pub rarity: Rarity,
    /// Reach.
    pub scope: ArtifactScope,
    /// Provenance.
    pub source: CreationSource,
    /// Current holder.
    pub owner: Owner,
    /// Ancestry (zone a NPC spawned in, set an item belongs to, …).
    pub parent_ids: Vec<ArtifactId>,
    /// Semantic tags.
    pub tags: Vec<FactTag>,
    /// The seed everything about this artifact re-derives from
    /// (determinism = identity: same seed, same artifact).
    pub seed: u64,
    /// Stable content hash; starts as `seed`, sealed by [`ArtifactHeader::seal`].
    pub hash: u64,
}

impl ArtifactHeader {
    /// A fresh draft with donor-verbatim defaults (artifact.rs:107-123): hidden,
    /// common, local, procedural, unowned.
    pub fn draft(id: ArtifactId, kind: CreationKind, name: impl Into<String>, seed: u64) -> Self {
        Self {
            id,
            kind,
            name: name.into(),
            status: ArtifactStatus::Draft,
            visibility: Visibility::Hidden,
            rarity: Rarity::Common,
            scope: ArtifactScope::Local,
            source: CreationSource::Procedural,
            owner: Owner::None,
            parent_ids: Vec::new(),
            tags: Vec::new(),
            seed,
            hash: seed,
        }
    }

    /// True once the world has accepted this artifact (donor artifact.rs:125-127).
    pub fn is_locked(&self) -> bool {
        matches!(
            self.status,
            ArtifactStatus::Locked | ArtifactStatus::Revealed | ArtifactStatus::Suppressed
        )
    }

    /// Seal the content hash from id, kind ordinal, seed, name bytes AND the
    /// parent lineage, so drift is detectable (donor pattern
    /// `combine_stable`/`hash_str`).
    ///
    /// PARENT LINEAGE ADDED 2026-08-26. It was missing: the header carried
    /// `parent_ids` and the seal ignored them, so two artifacts identical but
    /// for their ancestry sealed to the SAME hash and re-parenting was
    /// invisible to drift detection. That is the same defect class as a
    /// receipt whose verdict sits outside its own digest — a field the
    /// structure treats as meaningful and the seal does not.
    ///
    /// `soul.rs::seal_soulword` is the shape this follows: a sealed word
    /// carries its `parent` as part of what was sealed, never beside it.
    ///
    /// Parents are folded SORTED. A lineage is a SET — an artifact whose
    /// `parent_ids` Vec is merely reordered has not changed ancestry, and a
    /// seal that moved on a reorder would report drift that did not happen.
    ///
    /// No length prefix: an FNV fold over a sorted sequence already separates
    /// `[A]` from `[A, A]` and `[]` from `[0]`, so a count would be arithmetic
    /// that no test can justify. (Written in, then removed — an L18 sabotage
    /// that failed to redden anything is how it was caught.)
    pub fn seal(&mut self) {
        let mut h = FNV_OFFSET_BASIS;
        h = fnv1a64_fold(h, self.id.0);
        h = fnv1a64_fold(h, kind_ordinal(self.kind));
        h = fnv1a64_fold(h, self.seed);
        h = fnv1a64_fold(h, crate::checksum::hash_bytes_fnv1a(self.name.as_bytes()));

        let mut parents: Vec<u64> = self.parent_ids.iter().map(|p| p.0).collect();
        parents.sort_unstable();
        for p in parents {
            h = fnv1a64_fold(h, p);
        }
        self.hash = h;
    }
}

/// A secret's structure: one truth, optional cover story, reveal conditions,
/// false leads (donor artifact.rs:172-177 — dep-free, ported whole).
#[derive(Debug, Clone)]
pub struct SecretArtifact {
    /// The fact this secret protects.
    pub truth: LoreFactId,
    /// The lie told in its place, if any.
    pub cover_story: Option<LoreFactId>,
    /// Facts that must exist before it can be revealed.
    pub reveal_conditions: Vec<LoreFactId>,
    /// Facts that point away from the truth.
    pub false_leads: Vec<LoreFactId>,
}

/// A faction's structure (donor artifact.rs:195-199; `claimed_zones` collapsed to
/// [`ArtifactId`] — adaptation 3).
#[derive(Debug, Clone)]
pub struct FactionArtifact {
    /// The law the faction shows the world.
    pub public_law: String,
    /// The law it actually runs on.
    pub private_law: Option<String>,
    /// Zone artifacts it claims.
    pub claimed_zones: Vec<ArtifactId>,
}

/// The engine-data half of a graph node. W1 carries only the dep-free variants;
/// the v2 bridge payloads (NPC combat sheet, assembled item, biome + terrain span,
/// generated motif pattern — WIRING-GAPS.md) land in W5 where their crates live
/// (adaptation 7).
#[derive(Debug, Clone)]
pub enum ArtifactPayload {
    /// No payload yet (draft, or header-only kinds like Rumor).
    Empty,
    /// Free-text lore payload.
    Lore(String),
    /// A secret's structure.
    Secret(SecretArtifact),
    /// A faction's structure.
    Faction(FactionArtifact),
}

// ── Graph ───────────────────────────────────────────────────────────────────

/// Authoring-canvas position of a node, in MilliUnits (adaptation 6 — donor used
/// `f32`; core carries no floats).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct GraphPosition {
    /// Canvas x, MilliUnits.
    pub x: MilliUnit,
    /// Canvas y, MilliUnits.
    pub y: MilliUnit,
}

/// The eighteen asymmetric, typed relations between artifacts (donor
/// graph.rs:27-46, ported verbatim).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RelationKind {
    /// A contains/possesses B.
    Has,
    /// A is part of B's holdings.
    BelongsTo,
    /// A conceals B.
    Hides,
    /// A discloses B.
    Reveals,
    /// A requires B.
    Needs,
    /// A prevents B.
    Blocks,
    /// A mutates B.
    Changes,
    /// A opens B.
    Unlocks,
    /// A sings to B (motif relation).
    SingsTo,
    /// A remembers B.
    Remembers,
    /// A has forgotten B.
    Forgets,
    /// A is a fragment of B.
    PieceOf,
    /// A is owned by B.
    OwnedBy,
    /// A was made by B.
    MadeBy,
    /// A was stolen from B.
    StolenFrom,
    /// A is claimed by B.
    ClaimedBy,
    /// A is taboo to B.
    TabooTo,
    /// A was issued by B.
    IssuedBy,
}

/// One node: header + canvas position + child-readable label + engine payload
/// (donor graph.rs:49-56).
#[derive(Debug, Clone)]
pub struct GraphNode {
    /// Stable id (matches `header.id`).
    pub id: ArtifactId,
    /// Kind (matches `header.kind`).
    pub kind: CreationKind,
    /// The universal header.
    pub header: ArtifactHeader,
    /// Authoring-canvas position.
    pub position: GraphPosition,
    /// Child-readable label (toybox face).
    pub toybox_label: String,
    /// Full engine data (toolbox face).
    pub toolbox_payload: ArtifactPayload,
}

/// One typed, gated edge (donor graph.rs:59-68).
#[derive(Debug, Clone)]
pub struct GraphEdge {
    /// Stable id.
    pub id: EdgeId,
    /// Source artifact.
    pub from: ArtifactId,
    /// Target artifact.
    pub to: ArtifactId,
    /// The relation this edge asserts.
    pub relation: RelationKind,
    /// Lifecycle of the relation itself.
    pub status: ArtifactStatus,
    /// Who can see the relation.
    pub visibility: Visibility,
    /// Facts required before the relation is live.
    pub required_facts: Vec<LoreFactId>,
    /// Provenance of the edge.
    pub created_by: CreationSource,
}

/// The toybox/toolbox node graph (donor graph.rs:71-104, ported verbatim).
#[derive(Debug, Default, Clone)]
pub struct CreationGraph {
    /// All nodes, insertion order.
    pub nodes: Vec<GraphNode>,
    /// All edges, insertion order.
    pub edges: Vec<GraphEdge>,
}

impl CreationGraph {
    /// Append a node.
    pub fn add_node(&mut self, node: GraphNode) {
        self.nodes.push(node);
    }

    /// Append an edge.
    pub fn add_edge(&mut self, edge: GraphEdge) {
        self.edges.push(edge);
    }

    /// Find a node by id (linear — the graph is authoring-scale, not hot-path).
    pub fn node(&self, id: ArtifactId) -> Option<&GraphNode> {
        self.nodes.iter().find(|n| n.id == id)
    }

    /// Nodes reachable one hop out from `id`.
    pub fn children_of(&self, id: ArtifactId) -> Vec<&GraphNode> {
        self.edges
            .iter()
            .filter(|e| e.from == id)
            .filter_map(|e| self.node(e.to))
            .collect()
    }

    /// Edges pointing at `id`.
    pub fn incoming_to(&self, id: ArtifactId) -> Vec<&GraphEdge> {
        self.edges.iter().filter(|e| e.to == id).collect()
    }

    /// Edges leaving `id`.
    pub fn outgoing_from(&self, id: ArtifactId) -> Vec<&GraphEdge> {
        self.edges.iter().filter(|e| e.from == id).collect()
    }
}

// ── Ledger ──────────────────────────────────────────────────────────────────

/// What a lore fact asserts (donor ledger.rs:11-20, ported verbatim).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LoreFactKind {
    /// An artifact exists in the world.
    ArtifactExists,
    /// A relation exists between two artifacts.
    RelationExists,
    /// A secret exists (its truth still hidden).
    SecretExists,
    /// A secret's truth is now disclosed.
    SecretRevealed,
    /// An artifact is world-accepted.
    ArtifactLocked,
    /// An artifact is retired.
    ArtifactArchived,
    /// The run's state changed.
    RunStateChanged,
    /// A timeline branch forked.
    BranchForked,
}

/// One accepted fact (donor ledger.rs:23-32; `Tick` → `SimTick`, adaptation 4).
#[derive(Debug, Clone)]
pub struct LoreFact {
    /// Stable id.
    pub id: LoreFactId,
    /// What it asserts.
    pub kind: LoreFactKind,
    /// Provenance.
    pub source: CreationSource,
    /// The artifact it is about, if any.
    pub artifact: Option<ArtifactId>,
    /// Facts this one derives from.
    pub parent_ids: Vec<LoreFactId>,
    /// Semantic tags.
    pub tags: Vec<FactTag>,
    /// When the world accepted it.
    pub locked_at_tick: SimTick,
    /// Stable content hash.
    pub hash: u64,
}

/// The six commitment events (donor ledger.rs:35-42; `SecretId`/`BranchId` args
/// follow adaptations 3 and the kept [`BranchId`]).
#[derive(Debug, Clone)]
pub enum LedgerEventKind {
    /// World-accept an artifact.
    LockArtifact {
        /// The artifact being locked.
        artifact: ArtifactId,
    },
    /// World-accept a relation.
    LockRelation {
        /// Source artifact.
        from: ArtifactId,
        /// Target artifact.
        to: ArtifactId,
        /// The relation being locked.
        relation: RelationKind,
    },
    /// Disclose a secret's truth.
    RevealSecret {
        /// The secret artifact being revealed.
        secret: ArtifactId,
    },
    /// Hide a locked artifact again.
    SuppressArtifact {
        /// The artifact being suppressed.
        artifact: ArtifactId,
    },
    /// Retire an artifact.
    ArchiveArtifact {
        /// The artifact being archived.
        artifact: ArtifactId,
    },
    /// Fork the run's timeline.
    ForkBranch {
        /// Branch forked from.
        from: BranchId,
        /// New branch.
        to: BranchId,
    },
}

/// One committed event (donor ledger.rs:45-51).
#[derive(Debug, Clone)]
pub struct LedgerEvent {
    /// When it happened.
    pub tick: SimTick,
    /// What happened.
    pub kind: LedgerEventKind,
    /// Provenance.
    pub source: CreationSource,
    /// Facts this event creates.
    pub creates_facts: Vec<LoreFactId>,
    /// Stable content hash, set by [`Ledger::push_event`].
    pub hash: u64,
}

/// The append-only commitment layer: drafts are cheap, ledger events are where
/// the world accepts a fact (donor ledger.rs:53-72). This is also the Terraforma
/// seam — a per-world relay replays exactly these events over a seed.
#[derive(Debug, Default, Clone)]
pub struct Ledger {
    /// Accepted facts, append order.
    pub facts: Vec<LoreFact>,
    /// Committed events, append order.
    pub events: Vec<LedgerEvent>,
}

impl Ledger {
    /// True if the fact is already accepted.
    pub fn has_fact(&self, id: LoreFactId) -> bool {
        self.facts.iter().any(|f| f.id == id)
    }

    /// Commit an event: its hash is sealed here, then it is appended. Append-only —
    /// there is no removal path, by law.
    pub fn push_event(&mut self, mut event: LedgerEvent) {
        event.hash = event_hash(&event);
        self.events.push(event);
    }

    /// Accept a fact.
    pub fn push_fact(&mut self, fact: LoreFact) {
        self.facts.push(fact);
    }
}

/// Stable hash of one event (donor ledger.rs:74-109, rewritten allocation-free
/// over [`fnv1a64_fold`] — adaptation 5). Field order is the hash contract; the
/// leading discriminant per variant matches the donor's 1..=6.
pub fn event_hash(event: &LedgerEvent) -> u64 {
    let mut h = FNV_OFFSET_BASIS;
    h = fnv1a64_fold(h, event.tick.0 as u64);
    h = fnv1a64_fold(h, event.source as u64);
    match event.kind {
        LedgerEventKind::LockArtifact { artifact } => {
            h = fnv1a64_fold(h, 1);
            h = fnv1a64_fold(h, artifact.0);
        }
        LedgerEventKind::LockRelation { from, to, relation } => {
            h = fnv1a64_fold(h, 2);
            h = fnv1a64_fold(h, from.0);
            h = fnv1a64_fold(h, to.0);
            h = fnv1a64_fold(h, relation as u64);
        }
        LedgerEventKind::RevealSecret { secret } => {
            h = fnv1a64_fold(h, 3);
            h = fnv1a64_fold(h, secret.0);
        }
        LedgerEventKind::SuppressArtifact { artifact } => {
            h = fnv1a64_fold(h, 4);
            h = fnv1a64_fold(h, artifact.0);
        }
        LedgerEventKind::ArchiveArtifact { artifact } => {
            h = fnv1a64_fold(h, 5);
            h = fnv1a64_fold(h, artifact.0);
        }
        LedgerEventKind::ForkBranch { from, to } => {
            h = fnv1a64_fold(h, 6);
            h = fnv1a64_fold(h, from.0);
            h = fnv1a64_fold(h, to.0);
        }
    }
    for fact in &event.creates_facts {
        h = fnv1a64_fold(h, fact.0);
    }
    h
}

/// Stable ordinal of a kind for hashing (declaration order of [`CreationKind::ALL`]).
fn kind_ordinal(kind: CreationKind) -> u64 {
    CreationKind::ALL.iter().position(|k| *k == kind).unwrap_or(0) as u64
}

// ── Genre bounds (#1 of the ForgeVision drop) ───────────────────────────────

/// The four launch genres. New genres append (forward-only), never reorder.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Genre {
    /// ASCII MUD (forge-mud-v3, the singing terminal).
    Mud,
    /// Card game — no spatial extent; the board is a layout, not a world.
    Ccg,
    /// Facility/consequence simulation (SLAPP-class).
    Sim,
    /// Free-camera 3D world.
    Open3D,
}

/// Hard per-genre world caps, all powers of 3 (ternary paradigm — trits, not
/// bits, are the native radix). Values are `Estimate` pending ARCH000
/// ratification; the shape and the powers-of-3 law are settled and test-asserted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GenreBounds {
    /// Which genre this row caps.
    pub genre: Genre,
    /// World extent per axis, in cells (0 = non-spatial genre).
    pub world_extent_cells: u32,
    /// Maximum zones per world.
    pub max_zones: u32,
    /// Maximum live entities at once.
    pub max_live_entities: u32,
    /// Maximum atlas pages per world.
    pub max_atlas_pages: u32,
    /// Simulation tick rate, Hz (0 = turn-based). The UI clock stays 120 Hz
    /// regardless (two clocks, ARCH-004 canvas law).
    pub sim_hz: u32,
}

/// The bounds table, one row per [`Genre`], same order as the enum.
pub const GENRE_BOUNDS: [GenreBounds; 4] = [
    GenreBounds {
        genre: Genre::Mud,
        world_extent_cells: 81,
        max_zones: 243,
        max_live_entities: 729,
        max_atlas_pages: 9,
        sim_hz: 3,
    },
    GenreBounds {
        genre: Genre::Ccg,
        world_extent_cells: 0,
        max_zones: 81,
        max_live_entities: 243,
        max_atlas_pages: 27,
        sim_hz: 0,
    },
    GenreBounds {
        genre: Genre::Sim,
        world_extent_cells: 243,
        max_zones: 27,
        max_live_entities: 2187,
        max_atlas_pages: 81,
        sim_hz: 3,
    },
    GenreBounds {
        genre: Genre::Open3D,
        world_extent_cells: 729,
        max_zones: 729,
        max_live_entities: 6561,
        max_atlas_pages: 243,
        sim_hz: 27,
    },
];

/// Look up the bounds row for a genre.
pub fn bounds_for(genre: Genre) -> GenreBounds {
    // The table is enum-ordered; a mismatch is a construction defect the test
    // below turns into a red bar, so a linear scan stays honest AND obvious.
    GENRE_BOUNDS
        .iter()
        .copied()
        .find(|b| b.genre == genre)
        .unwrap_or(GENRE_BOUNDS[0])
}

/// Headless organ entry: print the codex constants as a runtime receipt
/// (kind count, toybox labels, genre bounds rows). Exit 0 always.
pub fn run(_args: &[String]) -> i32 {
    println!("creation-spine: MAX_CREATION_KIND={MAX_CREATION_KIND}");
    for kind in CreationKind::ALL {
        println!("  kind {:?} => {}", kind, kind.toybox_label());
    }
    for b in GENRE_BOUNDS {
        println!(
            "  genre {:?}: extent={} zones={} entities={} atlas={} sim_hz={}",
            b.genre, b.world_extent_cells, b.max_zones, b.max_live_entities, b.max_atlas_pages, b.sim_hz
        );
    }
    0
}

// Layout locks (crate doctrine, lib.rs:1-2): the id words stay exactly u64-wide.
const _: () = assert!(core::mem::size_of::<ArtifactId>() == 8);
const _: () = assert!(core::mem::size_of::<LoreFactId>() == 8);
const _: () = assert!(core::mem::size_of::<EdgeId>() == 8);
const _: () = assert!(core::mem::size_of::<BranchId>() == 8);
const _: () = assert!(core::mem::size_of::<SceneId>() == 8);
const _: () = assert!(core::mem::size_of::<ChoiceId>() == 8);
const _: () = assert!(CreationKind::ALL.len() == MAX_CREATION_KIND);
const _: () = assert!(GENRE_BOUNDS.len() == 4);

#[cfg(test)]
mod tests {
    use super::*;

    /// L01: the twelve-kind cap is the law, held by exhaustive match — adding a
    /// variant without growing ALL fails here at compile time.
    #[test]
    fn creation_spine_kind_count_is_exhaustive() {
        for kind in CreationKind::ALL {
            // Exhaustive: a new variant breaks this match before it breaks play.
            let label = match kind {
                CreationKind::Npc
                | CreationKind::Item
                | CreationKind::ItemSet
                | CreationKind::FactionItem
                | CreationKind::Zone
                | CreationKind::Secret
                | CreationKind::Scene
                | CreationKind::Faction
                | CreationKind::Rumor
                | CreationKind::Motif
                | CreationKind::Quest
                | CreationKind::WorldEvent => kind.toybox_label(),
            };
            assert!(!label.is_empty());
        }
        assert_eq!(CreationKind::ALL.len(), MAX_CREATION_KIND);
    }

    /// Donor determinism law: same event, same hash; different order, different hash.
    #[test]
    fn creation_spine_event_hash_is_stable_and_order_sensitive() {
        let ev = |from: u64, to: u64| LedgerEvent {
            tick: SimTick(960),
            kind: LedgerEventKind::LockRelation {
                from: ArtifactId(from),
                to: ArtifactId(to),
                relation: RelationKind::Hides,
            },
            source: CreationSource::Procedural,
            creates_facts: vec![LoreFactId(7)],
            hash: 0,
        };
        assert_eq!(event_hash(&ev(1, 2)), event_hash(&ev(1, 2)));
        assert_ne!(event_hash(&ev(1, 2)), event_hash(&ev(2, 1)));
    }

    /// Ledger seals hashes on commit and stays append-only in shape.
    #[test]
    fn creation_spine_ledger_seals_on_push() {
        let mut ledger = Ledger::default();
        ledger.push_event(LedgerEvent {
            tick: SimTick(1),
            kind: LedgerEventKind::LockArtifact { artifact: ArtifactId(13) },
            source: CreationSource::PlayerChoice,
            creates_facts: Vec::new(),
            hash: 0,
        });
        assert_eq!(ledger.events.len(), 1);
        assert_ne!(ledger.events[0].hash, 0);
        assert!(!ledger.has_fact(LoreFactId(1)));
    }

    /// Graph traversal round-trip on a two-node, one-edge world.
    #[test]
    fn creation_spine_graph_children_traversal() {
        let mut g = CreationGraph::default();
        let mut zone = ArtifactHeader::draft(ArtifactId(1), CreationKind::Zone, "Ironroot Vale", 42);
        zone.seal();
        let npc = ArtifactHeader::draft(ArtifactId(2), CreationKind::Npc, "The Operator", 43);
        g.add_node(GraphNode {
            id: zone.id,
            kind: zone.kind,
            header: zone,
            position: GraphPosition::default(),
            toybox_label: CreationKind::Zone.toybox_label().to_string(),
            toolbox_payload: ArtifactPayload::Empty,
        });
        g.add_node(GraphNode {
            id: npc.id,
            kind: npc.kind,
            header: npc,
            position: GraphPosition { x: MilliUnit(1000), y: MilliUnit(2000) },
            toybox_label: CreationKind::Npc.toybox_label().to_string(),
            toolbox_payload: ArtifactPayload::Lore("waits by the gate".to_string()),
        });
        g.add_edge(GraphEdge {
            id: EdgeId(1),
            from: ArtifactId(1),
            to: ArtifactId(2),
            relation: RelationKind::Has,
            status: ArtifactStatus::Draft,
            visibility: Visibility::Hidden,
            required_facts: Vec::new(),
            created_by: CreationSource::Procedural,
        });
        let kids = g.children_of(ArtifactId(1));
        assert_eq!(kids.len(), 1);
        assert_eq!(kids[0].id, ArtifactId(2));
        assert_eq!(g.incoming_to(ArtifactId(2)).len(), 1);
        assert_eq!(g.outgoing_from(ArtifactId(2)).len(), 0);
    }

    /// Draft defaults are donor-verbatim, and sealing moves the hash off the seed.
    #[test]
    fn creation_spine_draft_defaults_and_seal() {
        let mut h = ArtifactHeader::draft(ArtifactId(9), CreationKind::Secret, "The Debt", 777);
        assert_eq!(h.status, ArtifactStatus::Draft);
        assert_eq!(h.visibility, Visibility::Hidden);
        assert_eq!(h.rarity, Rarity::Common);
        assert_eq!(h.scope, ArtifactScope::Local);
        assert_eq!(h.source, CreationSource::Procedural);
        assert_eq!(h.owner, Owner::None);
        assert!(!h.is_locked());
        assert_eq!(h.hash, 777);
        h.seal();
        assert_ne!(h.hash, 777);
        let before = h.hash;
        h.seal();
        assert_eq!(h.hash, before, "sealing is idempotent");
    }

    /// Ancestry is part of what was sealed, not a field beside it. Two headers
    /// identical but for their parents must not share a hash, or re-parenting
    /// an artifact is invisible to drift detection.
    #[test]
    fn the_seal_carries_the_parent_lineage() {
        let seal_with = |parents: &[u64]| {
            let mut h = ArtifactHeader::draft(ArtifactId(4), CreationKind::Npc, "Warden", 11);
            h.parent_ids = parents.iter().map(|&p| ArtifactId(p)).collect();
            h.seal();
            h.hash
        };

        let orphan = seal_with(&[]);
        let one = seal_with(&[7]);
        let two = seal_with(&[7, 9]);

        assert_ne!(orphan, one, "gaining a parent must move the seal");
        assert_ne!(one, two, "gaining a second parent must move it again");
        assert_ne!(orphan, two);
    }

    /// A lineage is a SET. Reordering the Vec is not a change of ancestry, and
    /// a seal that moved on a reorder would report drift that did not happen.
    #[test]
    fn reordering_the_same_parents_does_not_move_the_seal() {
        let seal_with = |parents: &[u64]| {
            let mut h = ArtifactHeader::draft(ArtifactId(4), CreationKind::Npc, "Warden", 11);
            h.parent_ids = parents.iter().map(|&p| ArtifactId(p)).collect();
            h.seal();
            h.hash
        };
        assert_eq!(seal_with(&[7, 9, 2]), seal_with(&[2, 7, 9]));
        assert_eq!(seal_with(&[9, 2, 7]), seal_with(&[2, 7, 9]));
    }

    /// The count is folded in, so a repeated parent is not the same as one.
    #[test]
    fn a_repeated_parent_is_not_a_single_parent() {
        let seal_with = |parents: &[u64]| {
            let mut h = ArtifactHeader::draft(ArtifactId(4), CreationKind::Npc, "Warden", 11);
            h.parent_ids = parents.iter().map(|&p| ArtifactId(p)).collect();
            h.seal();
            h.hash
        };
        assert_ne!(seal_with(&[7]), seal_with(&[7, 7]));
    }

    /// #1 of the drop: every nonzero bound is a power of 3, and rows are
    /// enum-ordered so `bounds_for` never falls through.
    #[test]
    fn creation_spine_genre_bounds_are_powers_of_three() {
        fn pow3(mut v: u32) -> bool {
            while v % 3 == 0 {
                v /= 3;
            }
            v == 1
        }
        for b in GENRE_BOUNDS {
            for v in [b.world_extent_cells, b.max_zones, b.max_live_entities, b.max_atlas_pages, b.sim_hz] {
                assert!(v == 0 || pow3(v), "{:?}: {v} is not a power of 3", b.genre);
            }
        }
        assert_eq!(bounds_for(Genre::Open3D).world_extent_cells, 729);
        assert_eq!(bounds_for(Genre::Ccg).sim_hz, 0);
    }
}
