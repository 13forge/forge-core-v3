//! GLOSSARY — one word, one binding per sense, tamper-evident collision tracking.
//! Home: ARCH-020 (forge_book::arch_tablets).
//! Live gate: forge-book-v3 glossary.rs re-exports, dual-faces (const table + appendix render).

/// One sense of a glossary word: what it means, what binding it carries, who owns the definition.
#[derive(Debug, Clone, Copy)]
pub struct Sense {
    /// The meaning itself, phrased plainly.
    pub sense: &'static str,
    /// The binding domain (e.g. "forge", "vixel", "rhythm").
    pub binding: Binding,
    /// The owner of this sense (person name or crate).
    pub owner: &'static str,
    /// The expansion or explanation.
    pub means: &'static str,
}

/// Binding domains that organize senses into communities.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Binding {
    /// Core forge substrate terms.
    Forge = 0,
    /// Vixel/pixel-authoring domain terms.
    Vixel = 1,
    /// Rhythm/timing/cadence domain terms.
    Rhythm = 2,
    /// Canvas/render-surface domain terms.
    Canvas = 3,
    /// Language/grammar/lore domain terms.
    Language = 4,
    /// Ritual/ceremony domain terms.
    Ritual = 5,
    /// Evidence/proof/receipt domain terms.
    Evidence = 6,
    /// Uncategorized or cross-domain terms.
    Other = 255,
}

impl core::fmt::Display for Binding {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Binding::Forge => write!(f, "Forge"),
            Binding::Vixel => write!(f, "Vixel"),
            Binding::Rhythm => write!(f, "Rhythm"),
            Binding::Canvas => write!(f, "Canvas"),
            Binding::Language => write!(f, "Language"),
            Binding::Ritual => write!(f, "Ritual"),
            Binding::Evidence => write!(f, "Evidence"),
            Binding::Other => write!(f, "Other"),
        }
    }
}

/// One entry in the glossary: a word and all its senses.
#[derive(Debug, Clone, Copy)]
pub struct Entry {
    /// The word itself.
    pub word: &'static str,
    /// All the meanings it carries.
    pub senses: &'static [Sense],
    /// Collision cost: a tally of how many times this word collided with another,
    /// or shares a binding domain with another. Used to flag ambiguity.
    pub collision: &'static str,
}

/// The complete glossary: live, tamper-evident, gate-enforced by compile-time re-derivation
/// in forge-book-v3::golden_vixi drift tests.
pub const GLOSSARY: &[Entry] = &[
    // Starter set: one word, one sense, no collision (v1 baseline).
    Entry {
        word: "aperture",
        senses: &[Sense {
            sense: "The bounded scope of a screen group, enforced at compile time.",
            binding: Binding::Forge,
            owner: "Sean",
            means: "Never exceed four-ish items in one visual grouping (creation_dag law).",
        }],
        collision: "none",
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glossary_has_entries() {
        assert!(!GLOSSARY.is_empty(), "glossary must not be empty");
    }

    #[test]
    fn every_entry_has_senses() {
        for entry in GLOSSARY {
            assert!(!entry.senses.is_empty(), "entry '{}' has no senses", entry.word);
            for sense in entry.senses {
                assert!(!sense.sense.is_empty(), "sense is empty for '{}'", entry.word);
                assert!(!sense.owner.is_empty(), "owner is empty for '{}'", entry.word);
                assert!(!sense.means.is_empty(), "means is empty for '{}'", entry.word);
            }
        }
    }

    #[test]
    fn every_entry_has_collision_cost() {
        for entry in GLOSSARY {
            assert!(!entry.collision.is_empty(), "collision cost is empty for '{}'", entry.word);
        }
    }
}
