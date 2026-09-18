//! Organs — the 1-API registry for ported studio organs (Sean ruling 2026-08-17:
//! "organs live in core as mods", registry-first photon).
//!
//! One shape, two ranks: every organ has a headless `run` (the v2 forge-studio verb
//! signature `fn(&[String]) -> i32`, receipt: v2 main.rs dispatch ~1017-2350); organs
//! with a face register a panel DOWNSTREAM where their deps live (Crate Zero stays
//! zero-dep — the dag root never grows canvas/audio/zones edges, L06).
//!
//! Adding an organ = adding a mod + one row here, never editing a dispatch match —
//! the v2 complaint this registry retires.

pub mod create_geo;
pub mod creation_rules;
pub mod creation_spine;
pub mod dungeon_master_kit;
pub mod forge_scan;
pub mod massread;
pub mod massweld;
pub mod page_layout;
pub mod silent_drops;
pub mod song;
pub mod story_beats;
pub mod story_render;
pub mod symbiosis;
pub mod telemetry_kit;
pub mod timeline_export;
pub mod triple_loop;
pub mod twin_cull;
pub mod ui_state_feed;
pub mod visual_gate;
pub mod widgets;
pub mod win_registry;
pub mod worldgen_kit;
pub mod zone_lens;

/// One registered organ: a name and its headless entry.
#[derive(Clone, Copy)]
pub struct Organ {
    /// CLI-facing name, kebab-case, unique across the table (L05).
    pub name: &'static str,
    /// One-line purpose, shown by listings/HUD.
    pub about: &'static str,
    /// Headless entry — the v2 verb signature, exit-code out.
    pub run: fn(&[String]) -> i32,
}

/// Adapter: `win_registry::register_open_with` is fire-and-forget; the organ rank
/// needs the verb signature.
#[cfg(windows)]
fn win_registry_run(_args: &[String]) -> i32 {
    win_registry::register_open_with();
    0
}

/// Non-Windows: the row stays (listings are stable), the verb refuses with a named reason.
#[cfg(not(windows))]
fn win_registry_run(_args: &[String]) -> i32 {
    eprintln!("[win_registry] windows-only organ; no registry on this target");
    2
}

/// The registry. Append rows; never reorder (listings are stable).
pub const ORGANS: &[Organ] = &[
    Organ {
        name: "silent-drops",
        about: "harvest every `_ => {}` into a structured inventory",
        run: silent_drops::run,
    },
    Organ {
        name: "twin-cull",
        about: "two-tier delete ladder; drain census by SHA-256",
        run: twin_cull::run,
    },
    Organ {
        name: "forge-scan",
        about: "portfolio scanner: .rs velocity across repo roots, JSON out",
        run: forge_scan::run,
    },
    Organ {
        name: "ui-state",
        about: "query the UI-state IPC doc (topmost-at, node rows)",
        run: ui_state_feed::run_query,
    },
    Organ {
        name: "win-registry",
        about: "register studio in Explorer's Open With menu",
        run: win_registry_run,
    },
    Organ {
        name: "story-beats",
        about: "whisper JSON -> scene-map.json beat segmentation",
        run: story_beats::run,
    },
    Organ {
        name: "creation-spine",
        about: "print the artifact codex constants: 12 kinds, genre bounds",
        run: creation_spine::run,
    },
];

// dungeon_master_kit, worldgen_kit, zone_lens, create_geo, symbiosis landed this wave
// (wave 4, Broski board-clear) as compiling stub-adapted modules — no ORGANS row yet:
// their donor entry points are render/generate/dual_verdict fns, not the verb
// signature `fn(&[String]) -> i32`. Adapter fns (win_registry_run pattern) are a
// follow-up wire, not silently dropped (L15).

/// Look an organ up by name.
pub fn organ(name: &str) -> Option<&'static Organ> {
    ORGANS.iter().find(|o| o.name == name)
}

/// One line per organ: `name — about`.
pub fn listing() -> String {
    let mut s = String::new();
    for o in ORGANS {
        s.push_str(o.name);
        s.push_str(" — ");
        s.push_str(o.about);
        s.push('\n');
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_names_are_unique_and_kebab() {
        for (i, a) in ORGANS.iter().enumerate() {
            assert!(
                !a.name.is_empty() && a.name.chars().all(|c| c.is_ascii_lowercase() || c == '-'),
                "organ name {:?} is not kebab-case",
                a.name
            );
            for b in &ORGANS[i + 1..] {
                assert_ne!(a.name, b.name, "two organs share one name (L05)");
            }
        }
    }

    #[test]
    fn lookup_and_listing_agree_with_the_table() {
        assert!(organ("silent-drops").is_some());
        assert!(organ("no-such-organ").is_none());
        let l = listing();
        assert_eq!(l.lines().count(), ORGANS.len());
        assert!(l.contains("silent-drops — "));
    }
}
