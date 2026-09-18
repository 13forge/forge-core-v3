//! Dialogue graph words: a node, a choice, and the tag lock a choice opens on.
//! One home for forge-mud-v3 (runtime) and forge-book-v3 (persona narration);
//! lifted from forge-mud-v3 ironroot/dialogue.rs 2026-09-02.

use crate::soul::SoulId;

/// A single dialogue node: text + optional choices.
#[derive(Debug, Clone)]
pub struct DialogueNode {
    /// The node's own id — how choices and the graph name it.
    pub id: String,
    /// The line of dialogue this node speaks.
    pub text: String,
    /// Where the conversation can go from here.
    pub choices: Vec<DialogueChoice>,
    /// Who speaks this node, if anyone. None = narrator text or an inanimate source.
    pub speaker: Option<SoulId>,
}

/// A player choice that leads to another node. `lock` holds the `u64` tag
/// hashes this branch REQUIRES, tested with [`opens`]. Empty = an open door.
#[derive(Debug, Clone, Default)]
pub struct DialogueChoice {
    /// What the player reads for this option.
    pub label: String,
    /// The node id this choice arrives at.
    pub next_node: String,
    /// Tags the player must hold for this branch to open. Empty = open door.
    pub lock: Vec<u64>,
}

/// The player's live tag set — the SAME `u64` hash vocabulary
/// [`DialogueChoice::lock`] is authored against.
pub type KeyRing<'a> = &'a [u64];

/// Does `held` satisfy `required`? Every required tag must be present; extras
/// are harmless and an empty `required` is an open door.
#[inline]
pub fn opens(held: KeyRing<'_>, required: &[u64]) -> bool {
    required.iter().all(|t| held.contains(t))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_lock_is_an_open_door() {
        assert!(opens(&[], &[]));
        assert!(opens(&[1, 2], &[]));
    }

    #[test]
    fn every_required_tag_must_be_held() {
        assert!(opens(&[1, 2, 3], &[1, 3]));
        assert!(!opens(&[1, 2], &[3]));
        assert!(!opens(&[], &[1]));
    }
}
