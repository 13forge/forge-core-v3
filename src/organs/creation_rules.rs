//! Creation rules — NEW pieces drained from donor `tools\ironroot_creation_engine`
//! onto `creation_spine` types (2026-09-01). Renamed on collision:
//! `ValidationReport`/`ValidationIssue` -> `Creation*` (twin in forge-zones-v3::blueprint_validate).

use super::creation_spine::*;
use crate::fixed_point::SimTick;

// ── validation (donor validation.rs) ────────────────────────────────────────

/// Why a graph or edge failed a validation check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoreValidationError {
    /// A new fact contradicts one already locked.
    ContradictsLockedFact,
    /// A secret's truth surfaced before its reveal conditions.
    RevealsSecretTooEarly,
    /// A speaker asserts a fact they could not know.
    SpeakerKnowsImpossibleFact,
    /// A `Unique`-rarity artifact appears more than once.
    DuplicateUniqueArtifact,
    /// A node names a parent id that has no node.
    MissingParentArtifact(ArtifactId),
    /// An edge requires a fact the ledger has not accepted.
    MissingRequiredFact(LoreFactId),
    /// A faction relation is not legal.
    InvalidFactionRelation,
    /// A zone scope is not legal.
    InvalidZoneScope,
    /// A disclosure level was skipped.
    BreaksDisclosureLevel,
    /// An edge's (from, to, relation) triple is not on the toybox allow-list.
    InvalidEdgeForToybox {
        /// Source kind.
        from: CreationKind,
        /// Target kind.
        to: CreationKind,
        /// The relation asserted.
        relation: RelationKind,
    },
}

/// How serious a validation issue is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidationSeverity {
    /// Worth noting, not blocking.
    Info,
    /// Should be fixed, does not block.
    Warning,
    /// Blocks acceptance.
    Blocker,
}

/// One found problem.
#[derive(Debug, Clone)]
pub struct CreationValidationIssue {
    /// How serious.
    pub severity: ValidationSeverity,
    /// What went wrong.
    pub error: LoreValidationError,
    /// Human-readable detail.
    pub note: String,
}

/// All issues found by one [`validate_graph`] call.
#[derive(Debug, Default, Clone)]
pub struct CreationValidationReport {
    /// Every issue found, in scan order.
    pub issues: Vec<CreationValidationIssue>,
}

impl CreationValidationReport {
    /// True if any issue is a [`ValidationSeverity::Blocker`].
    pub fn is_blocked(&self) -> bool {
        self.issues.iter().any(|i| i.severity == ValidationSeverity::Blocker)
    }
}

/// Scan a graph for duplicate uniques, dangling parents, illegal toybox edges,
/// and edges missing their required facts.
pub fn validate_graph(graph: &CreationGraph, ledger: &Ledger) -> CreationValidationReport {
    let mut report = CreationValidationReport::default();

    for node in &graph.nodes {
        if node.header.rarity == Rarity::Unique {
            let duplicate_count = graph
                .nodes
                .iter()
                .filter(|n| n.header.name == node.header.name && n.header.rarity == Rarity::Unique)
                .count();
            if duplicate_count > 1 {
                report.issues.push(CreationValidationIssue {
                    severity: ValidationSeverity::Blocker,
                    error: LoreValidationError::DuplicateUniqueArtifact,
                    note: format!("Unique artifact '{}' appears more than once", node.header.name),
                });
            }
        }

        for parent in &node.header.parent_ids {
            if graph.node(*parent).is_none() {
                report.issues.push(CreationValidationIssue {
                    severity: ValidationSeverity::Blocker,
                    error: LoreValidationError::MissingParentArtifact(*parent),
                    note: format!("Artifact {:?} references missing parent {:?}", node.id, parent),
                });
            }
        }
    }

    for edge in &graph.edges {
        let Some(from) = graph.node(edge.from) else { continue };
        let Some(to) = graph.node(edge.to) else { continue };
        if !is_toybox_edge_allowed(from.kind, to.kind, edge.relation) {
            report.issues.push(CreationValidationIssue {
                severity: ValidationSeverity::Warning,
                error: LoreValidationError::InvalidEdgeForToybox {
                    from: from.kind,
                    to: to.kind,
                    relation: edge.relation,
                },
                note: "Edge is not child-simple. Allow in toolbox only or add a clearer relation.".to_string(),
            });
        }

        for fact in &edge.required_facts {
            if !ledger.has_fact(*fact) {
                report.issues.push(CreationValidationIssue {
                    severity: ValidationSeverity::Warning,
                    error: LoreValidationError::MissingRequiredFact(*fact),
                    note: format!("Edge {:?} requires missing fact {:?}", edge.id, fact),
                });
            }
        }
    }

    report
}

/// True if `(from, to, relation)` is on the toybox-simple allow-list.
pub fn is_toybox_edge_allowed(from: CreationKind, to: CreationKind, relation: RelationKind) -> bool {
    use CreationKind::*;
    use RelationKind::*;

    matches!(
        (from, to, relation),
        (Zone, Npc, Has)
            | (Zone, Item, Has)
            | (Zone, Secret, Hides)
            | (Zone, WorldEvent, Has)
            | (Npc, Secret, Hides)
            | (Npc, Item, Has)
            | (Npc, Faction, BelongsTo)
            | (Npc, Motif, Remembers)
            | (Item, Secret, Reveals)
            | (Item, ItemSet, PieceOf)
            | (Item, Faction, BelongsTo)
            | (ItemSet, Item, Has)
            | (Secret, Scene, Unlocks)
            | (Secret, WorldEvent, Reveals)
            | (Motif, Zone, SingsTo)
            | (Motif, Secret, Reveals)
            | (Scene, Secret, Reveals)
            | (Scene, WorldEvent, Changes)
            | (Faction, Item, Has)
            | (Faction, Zone, Has)
    )
}

// ── reset (donor reset.rs) ──────────────────────────────────────────────────

/// What a reset clears.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResetScope {
    /// Deletes drafts only.
    Preview,
    /// Clears the active run projection.
    Run,
    /// Archives or deletes a whole branch.
    Branch,
}

/// One timeline fork.
#[derive(Debug, Clone)]
pub struct WorldBranch {
    /// Stable id.
    pub id: BranchId,
    /// Branch this one forked from, if any.
    pub parent_branch: Option<BranchId>,
    /// Facts locked in this branch.
    pub locked_facts: Vec<LoreFactId>,
    /// When this branch was created.
    pub created_at: SimTick,
    /// True once archived.
    pub archived: bool,
}

/// One run's live fact projection.
#[derive(Debug, Clone)]
pub struct RunProjection {
    /// This run's id.
    pub run_id: u64,
    /// The seed the run started from.
    pub root_seed: u64,
    /// Facts currently active.
    pub active_facts: Vec<LoreFactId>,
    /// Facts disclosed to the player.
    pub revealed_facts: Vec<LoreFactId>,
    /// Facts hidden again after being active.
    pub suppressed_facts: Vec<LoreFactId>,
}

/// True if an artifact in `status` should be removed by a reset of `scope`.
pub fn should_remove_artifact_on_reset(status: ArtifactStatus, scope: ResetScope) -> bool {
    match scope {
        ResetScope::Preview => matches!(status, ArtifactStatus::Draft | ArtifactStatus::Preview),
        ResetScope::Run => false,
        ResetScope::Branch => !matches!(status, ArtifactStatus::Locked | ArtifactStatus::Revealed),
    }
}

// ── item sets (donor item_sets.rs; sub-ids collapsed to ArtifactId, adaptation 3) ──

/// What kind of reward a set bonus grants.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SetBonusKind {
    /// Numeric stat change.
    StatModifier,
    /// Discloses a secret.
    RevealSecret,
    /// Opens a scene.
    UnlockScene,
    /// Points toward a motif's direction.
    HearMotifDirection,
    /// Interrupts a world event.
    InterruptWorldEvent,
    /// Disguises the wearer as another faction.
    FactionDisguise,
}

/// One bonus, active at a piece-count threshold.
#[derive(Debug, Clone)]
pub struct SetBonus {
    /// Pieces owned before this bonus activates.
    pub required_piece_count: u8,
    /// What kind of bonus.
    pub kind: SetBonusKind,
    /// Human-readable description.
    pub description: String,
    /// Facts granted once active.
    pub grants_facts: Vec<LoreFactId>,
}

/// A fragmented item set: useful alone, revealing together.
#[derive(Debug, Clone)]
pub struct ItemSet {
    /// Stable id.
    pub id: ArtifactId,
    /// Display name.
    pub name: String,
    /// Owning faction artifact, if any.
    pub faction: Option<ArtifactId>,
    /// Zone artifact this set originates from, if any.
    pub origin_zone: Option<ArtifactId>,
    /// Facts required before this set can be assembled.
    pub required_lore_facts: Vec<LoreFactId>,
    /// Item artifacts that make up this set.
    pub pieces: Vec<ArtifactId>,
    /// Bonuses active below full completion.
    pub partial_bonuses: Vec<SetBonus>,
    /// Bonus active only at full completion.
    pub full_bonus: Option<SetBonus>,
    /// Secret artifacts this set unlocks.
    pub secret_unlocks: Vec<ArtifactId>,
    /// Fact locked once this set is complete.
    pub locked_fact: Option<LoreFactId>,
}

/// An item bound to one faction's use.
#[derive(Debug, Clone)]
pub struct FactionItem {
    /// The item artifact.
    pub item_id: ArtifactId,
    /// The faction artifact it is bound to.
    pub faction_id: ArtifactId,
    /// Minimum faction rank required to use it, if any.
    pub rank_required: Option<u8>,
    /// Minimum reputation required to use it, if any.
    pub reputation_required: Option<i32>,
    /// Factions for whom using it is taboo.
    pub taboo_if_used_by: Vec<ArtifactId>,
    /// Fact disclosed publicly about this item.
    pub public_lore: LoreFactId,
    /// Fact known only privately, if any.
    pub private_lore: Option<LoreFactId>,
    /// Facts unlocked if this item is used in betrayal.
    pub betrayal_effects: Vec<LoreFact>,
}

impl ItemSet {
    /// True if every piece is present in `owned_items`.
    pub fn is_complete_with(&self, owned_items: &[ArtifactId]) -> bool {
        self.pieces.iter().all(|piece| owned_items.contains(piece))
    }

    /// Every bonus active given `owned_items`.
    pub fn active_bonuses(&self, owned_items: &[ArtifactId]) -> Vec<&SetBonus> {
        let count = self.pieces.iter().filter(|piece| owned_items.contains(piece)).count() as u8;
        let mut bonuses: Vec<&SetBonus> = self
            .partial_bonuses
            .iter()
            .filter(|bonus| count >= bonus.required_piece_count)
            .collect();

        if let Some(full) = &self.full_bonus {
            if count as usize == self.pieces.len() {
                bonuses.push(full);
            }
        }

        bonuses
    }
}

// ── ui contract (donor ui_contract.rs) ──────────────────────────────────────

/// Which face of the creation UI is active.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CreationUiMode {
    /// Child-simple card view.
    Toybox,
    /// Full engine-data view.
    Forge,
}

/// One action the creation UI can dispatch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiAction {
    /// Re-roll this artifact.
    Roll,
    /// Open detail view.
    Inspect,
    /// World-accept this artifact.
    Lock,
    /// Lock as a secret.
    LockAsSecret,
    /// Lock as a rumor.
    LockAsRumor,
    /// Lock as a false lead.
    LockAsFalseLead,
    /// Edit this artifact's fields.
    Mutate,
    /// Fork a new branch from here.
    Branch,
    /// Disclose to the player.
    Reveal,
    /// Hide again after being revealed.
    Suppress,
    /// Reset the preview.
    ResetPreview,
    /// Reset the active run.
    ResetRun,
    /// Fork the whole world.
    ForkWorld,
    /// Archive a branch.
    ArchiveBranch,
    /// Start a new item set.
    CreateItemSet,
    /// Add a piece to an item set.
    AddPieceToSet,
    /// Lock a set bonus.
    LockSetBonus,
    /// Bind a set to a faction.
    BindSetToFaction,
    /// Bind a set to a secret.
    BindSetToSecret,
    /// Mark an item taboo for a faction.
    MarkItemAsFactionTaboo,
    /// Fork a set variant.
    ForkSetVariant,
    /// Archive a set's branch.
    ArchiveSetBranch,
    /// Ask why this artifact is the way it is.
    ExplainWhy,
    /// Ask what breaks if this artifact changes.
    WhatBreaks,
}

/// One toybox-face card.
#[derive(Debug, Clone)]
pub struct ToyboxCard {
    /// The artifact it represents.
    pub id: ArtifactId,
    /// Icon lookup key.
    pub icon_key: &'static str,
    /// Display title.
    pub title: String,
    /// Human-readable kind label.
    pub kind_label: String,
    /// Lifecycle state.
    pub status: ArtifactStatus,
    /// Visibility.
    pub visibility: Visibility,
    /// Small descriptive tags.
    pub tiny_tags: Vec<String>,
}

/// One line of an "explain why" trace.
#[derive(Debug, Clone)]
pub struct ExplainWhyLine {
    /// Human-readable explanation.
    pub text: String,
    /// The fact this line traces back to, if any.
    pub source_fact: Option<LoreFactId>,
}

/// A report of what breaks if an artifact changes.
#[derive(Debug, Clone)]
pub struct WhatBreaksReport {
    /// The artifact under consideration.
    pub target: ArtifactId,
    /// Other artifacts affected.
    pub affected_artifacts: Vec<ArtifactId>,
    /// Relations affected.
    pub affected_relations: Vec<RelationKind>,
    /// Human-readable warning.
    pub warning: String,
}

/// Icon lookup key for a kind's toybox card.
pub fn icon_key_for(kind: CreationKind) -> &'static str {
    match kind {
        CreationKind::Npc => "person",
        CreationKind::Item => "thing",
        CreationKind::ItemSet => "collection",
        CreationKind::FactionItem => "faction_thing",
        CreationKind::Zone => "place",
        CreationKind::Secret => "secret",
        CreationKind::Scene => "choice",
        CreationKind::Faction => "group",
        CreationKind::Rumor => "rumor",
        CreationKind::Motif => "song",
        CreationKind::Quest => "quest",
        CreationKind::WorldEvent => "event",
    }
}

// ── generation (donor generation.rs; hash.rs -> crate::checksum, adaptation 5) ──

/// Inputs steering one generation call.
#[derive(Debug, Clone)]
pub struct GenerationContext {
    /// The run's root seed.
    pub root_seed: u64,
    /// Parent artifact, if any.
    pub parent: Option<ArtifactId>,
    /// Owning zone, if any.
    pub zone: Option<ArtifactId>,
    /// Owning faction, if any.
    pub faction: Option<ArtifactId>,
    /// Related motif, if any.
    pub motif: Option<ArtifactId>,
}

/// A weighted pick input.
#[derive(Debug, Clone)]
pub struct GenerationWeight {
    /// Base weight.
    pub base: i32,
    /// Modifiers applied on top of the base.
    pub modifiers: Vec<WeightModifier>,
}

/// One weight adjustment tied to a fact tag.
#[derive(Debug, Clone)]
pub struct WeightModifier {
    /// The tag this modifier applies for.
    pub tag: FactTag,
    /// The weight delta.
    pub delta: i32,
}

/// Authored word lists generation picks from.
#[derive(Debug, Clone)]
pub struct WordTable {
    /// NPC role names.
    pub roles: Vec<&'static str>,
    /// NPC wound phrases.
    pub wounds: Vec<&'static str>,
    /// NPC behavior quirks.
    pub behaviors: Vec<&'static str>,
    /// NPC secret phrases.
    pub secrets: Vec<&'static str>,
    /// Item kind names.
    pub item_kinds: Vec<&'static str>,
    /// Item material names.
    pub materials: Vec<&'static str>,
}

impl Default for WordTable {
    fn default() -> Self {
        Self {
            roles: vec!["bell-mender", "grave-tender", "road-tax clerk", "root-pruner deserter"],
            wounds: vec![
                "heard the Name-Shear and forgot their sister",
                "buried an empty coffin",
                "sold a map to the wrong saint",
                "rang a bell before the rain stopped",
            ],
            behaviors: vec![
                "counts coins by touch",
                "never walks under bells",
                "feeds birds that are not there",
                "hums when legal names are spoken",
            ],
            secrets: vec![
                "keeps a forbidden receipt",
                "remembers a name no one else can say",
                "marks cleanse routes in ash",
                "owns a bell that rings only for erased graves",
            ],
            item_kinds: vec!["bell-clapper", "grave chain", "receipt knife", "ash map"],
            materials: vec!["debt-iron", "rain glass", "root bone", "black brass"],
        }
    }
}

fn combine_stable(words: &[u64]) -> u64 {
    let mut h = crate::checksum::FNV_OFFSET_BASIS;
    for w in words {
        h = crate::checksum::fnv1a64_fold(h, *w);
    }
    h
}

fn hash_str(s: &str) -> u64 {
    crate::checksum::hash_bytes_fnv1a(s.as_bytes())
}

/// Deterministically pick one entry of `table` by `seed`.
pub fn pick<'a>(seed: u64, table: &'a [&'a str]) -> &'a str {
    let idx = (seed as usize) % table.len();
    table[idx]
}

/// Generate one NPC node, deterministic for a given `seed`.
pub fn generate_npc_node(id: ArtifactId, seed: u64, table: &WordTable, position: GraphPosition) -> GraphNode {
    let role = pick(seed, &table.roles);
    let wound = pick(seed.rotate_left(7), &table.wounds);
    let behavior = pick(seed.rotate_left(13), &table.behaviors);
    let secret = pick(seed.rotate_left(19), &table.secrets);
    let name = format!("{} {}", title_case(role), short_seed_name(seed));

    let public_backstory = format!("{} was once a {} who {}. They now {}.", name, role, wound, behavior);
    let private_backstory = format!("Privately, {} {}.", name, secret);

    let mut header = ArtifactHeader::draft(id, CreationKind::Npc, name, seed);
    header.hash = combine_stable(&[seed, hash_str(&public_backstory), hash_str(&private_backstory)]);

    GraphNode {
        id,
        kind: CreationKind::Npc,
        toybox_label: CreationKind::Npc.toybox_label().to_string(),
        position,
        header,
        toolbox_payload: ArtifactPayload::Lore(public_backstory),
    }
}

/// Generate one item node, deterministic for a given `seed`.
pub fn generate_item_node(id: ArtifactId, seed: u64, table: &WordTable, position: GraphPosition) -> GraphNode {
    let kind = pick(seed, &table.item_kinds);
    let material = pick(seed.rotate_left(11), &table.materials);
    let name = format!("{} of {}", title_case(kind), title_case(material));
    let public_lore = format!("This {} was made from {}. It carries a faint procedural resonance.", kind, material);

    let mut header = ArtifactHeader::draft(id, CreationKind::Item, name, seed);
    header.rarity = Rarity::Uncommon;
    header.hash = combine_stable(&[seed, hash_str(&public_lore)]);

    GraphNode {
        id,
        kind: CreationKind::Item,
        toybox_label: CreationKind::Item.toybox_label().to_string(),
        position,
        header,
        toolbox_payload: ArtifactPayload::Lore(public_lore),
    }
}

fn short_seed_name(seed: u64) -> String {
    const SYL: &[&str] = &["Orr", "Mael", "Vess", "Hal", "Brin", "Cael", "Sorn", "Edda"];
    pick(seed.rotate_left(3), SYL).to_string()
}

fn title_case(s: &str) -> String {
    let mut out = String::new();
    let mut cap = true;
    for ch in s.chars() {
        if cap {
            out.extend(ch.to_uppercase());
            cap = false;
        } else if ch == '-' || ch == ' ' {
            out.push(ch);
            cap = true;
        } else {
            out.push(ch);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(id: u64, kind: CreationKind) -> GraphNode {
        GraphNode {
            id: ArtifactId(id),
            kind,
            header: ArtifactHeader::draft(ArtifactId(id), kind, "x", id),
            position: GraphPosition::default(),
            toybox_label: kind.toybox_label().to_string(),
            toolbox_payload: ArtifactPayload::Empty,
        }
    }

    #[test]
    fn creation_toybox_edge_law_allows_has_and_hides() {
        assert!(is_toybox_edge_allowed(CreationKind::Zone, CreationKind::Npc, RelationKind::Has));
        assert!(is_toybox_edge_allowed(CreationKind::Zone, CreationKind::Secret, RelationKind::Hides));
    }

    #[test]
    fn creation_toybox_edge_law_refuses_illegal_pair() {
        assert!(!is_toybox_edge_allowed(CreationKind::Npc, CreationKind::Zone, RelationKind::Has));
    }

    #[test]
    fn creation_validate_graph_flags_dangling_parent() {
        let mut graph = CreationGraph::default();
        let mut orphan = node(1, CreationKind::Npc);
        orphan.header.parent_ids.push(ArtifactId(99));
        graph.add_node(orphan);
        let ledger = Ledger::default();
        let report = validate_graph(&graph, &ledger);
        assert!(report.is_blocked());
        assert!(report
            .issues
            .iter()
            .any(|i| matches!(i.error, LoreValidationError::MissingParentArtifact(ArtifactId(99)))));
    }

    #[test]
    fn creation_reset_scope_removes_draft_keeps_locked() {
        assert!(should_remove_artifact_on_reset(ArtifactStatus::Draft, ResetScope::Preview));
        assert!(!should_remove_artifact_on_reset(ArtifactStatus::Locked, ResetScope::Preview));
        assert!(!should_remove_artifact_on_reset(ArtifactStatus::Locked, ResetScope::Branch));
    }

    #[test]
    fn creation_generate_npc_node_is_deterministic_for_seed() {
        let table = WordTable::default();
        let a = generate_npc_node(ArtifactId(1), 42, &table, GraphPosition::default());
        let b = generate_npc_node(ArtifactId(1), 42, &table, GraphPosition::default());
        assert_eq!(a.header.hash, b.header.hash);
        assert_eq!(a.header.name, b.header.name);
    }
}
