//! `qa drops` — harvest every `_ => {}` into a structured inventory.
//!
//! `_ => {}` is never UB; it is a silent logical drop (a value matched nothing and left
//! no trace). Proven cost (v2): `main.rs:1808` dropped an unknown CLI verb into the
//! GUI-boot fallthrough, so a stale deployed bin launched the window plus its door and
//! sidecar children — 3-4 processes per hook fire, exit 0, no message.
//!
//! Ported verbatim 2026-08-17 from `F:\NewRepo\crates\forge-studio\src\silent_drops.rs`
//! (C06 donor cite) with two v3 adaptations: the recursive `walk` became a bounded
//! work-list loop (CLAUDE.md forbidden_ops: no recursive directory walks), and the
//! tests dropped their `tempfile`/`ron` dev-deps (Crate Zero stays zero-dep).

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

/// Where a drop sits determines what it costs when the arm is reached.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Tier {
    /// Answers a command, verb, edict, key or socket op. A drop here = a user action
    /// that silently does nothing.
    Dispatch,
    /// Mutates or routes state. A drop here = a mutation that never lands, unlogged.
    State,
    /// Lowers or parses. A drop here = a silent render or parse gap; nothing errors,
    /// the output is just wrong.
    Lowering,
    /// The arm states its reason in place (trailing comment). Not clean, but not
    /// silent — this is the shape the others convert to.
    Annotated,
}

impl Tier {
    /// Stable display name for ledgers and the RON census.
    pub fn as_str(self) -> &'static str {
        match self {
            Tier::Dispatch => "Dispatch",
            Tier::State => "State",
            Tier::Lowering => "Lowering",
            Tier::Annotated => "Annotated",
        }
    }
}

/// One harvested site.
#[derive(Debug, Clone)]
pub struct Drop {
    /// Repo-relative path of the file holding the arm, forward slashes.
    pub file: String,
    /// 1-indexed line of the wildcard arm.
    pub line: usize,
    /// Cost lane the site classifies into.
    pub tier: Tier,
    /// The trailing comment reason, if the arm carries one.
    pub reason: String,
}

/// Modules whose job is answering a command. Substring match on the repo-relative path.
const DISPATCH_MARKERS: &[&str] = &[
    "main.rs", "edicts", "door", "serve", "beacon", "scheduler", "harness", "voice_command",
    "bridge", "driver", "zone_lens", "hook", "warden", "intel_drain", "plugins",
];

/// Modules whose job is moving state. Checked after dispatch, before lowering.
const STATE_MARKERS: &[&str] = &[
    "sieve", "unified", "ump", "score", "theory", "combat", "physics", "world", "quest",
    "ledger", "gauge", "flags", "atom", "status_effects", "cutscene", "changeset",
    "timeline", "graph", "biome", "selection", "evaluate", "playback", "uniforms",
];

fn classify(rel: &str, reason: &str) -> Tier {
    if !reason.is_empty() {
        return Tier::Annotated;
    }
    let hay = rel.replace('\\', "/");
    if DISPATCH_MARKERS.iter().any(|m| hay.contains(m)) {
        Tier::Dispatch
    } else if STATE_MARKERS.iter().any(|m| hay.contains(m)) {
        Tier::State
    } else {
        Tier::Lowering
    }
}

/// A bare wildcard arm that does nothing: `_ => {}` or `_ => ()`, any spacing.
fn drop_arm(line: &str) -> bool {
    let t = line.trim_start();
    let Some(rest) = t.strip_prefix('_') else { return false };
    let rest = rest.trim_start();
    let Some(rest) = rest.strip_prefix("=>") else { return false };
    let rest = rest.trim_start();
    rest.starts_with("{}") || rest.starts_with("{ }") || rest.starts_with("()")
}

/// The stated reason, if any: a trailing `//` comment on the same line.
fn reason_of(line: &str) -> String {
    match line.find("//") {
        Some(i) => line[i + 2..].trim().to_string(),
        None => String::new(),
    }
}

/// Bounded work-list walk (no recursion): flat `read_dir` per directory, explicit
/// depth cap so a cycle or runaway tree can never spin the scan.
const MAX_DEPTH: usize = 12;

fn walk(root: &Path, out: &mut Vec<PathBuf>) {
    let mut queue: Vec<(PathBuf, usize)> = vec![(root.to_path_buf(), 0)];
    while let Some((dir, depth)) = queue.pop() {
        if depth > MAX_DEPTH {
            continue;
        }
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                // target/ is build output, not authored code; _vault/_attic are quarry.
                let skip = matches!(
                    p.file_name().and_then(|n| n.to_str()),
                    Some("target") | Some(".git") | Some("_vault") | Some("_attic") | Some("node_modules")
                );
                if !skip {
                    queue.push((p, depth + 1));
                }
            } else if p.extension().and_then(|x| x.to_str()) == Some("rs") {
                out.push(p);
            }
        }
    }
}

/// Scan a tree and return every drop, sorted by tier then path.
pub fn harvest(root: &Path) -> Vec<Drop> {
    let mut files = Vec::new();
    walk(&root.join("crates"), &mut files);
    files.sort();

    let mut drops = Vec::new();
    for f in files {
        let Ok(body) = std::fs::read_to_string(&f) else { continue };
        let rel = f
            .strip_prefix(root)
            .unwrap_or(&f)
            .to_string_lossy()
            .replace('\\', "/");
        for (i, line) in body.lines().enumerate() {
            if !drop_arm(line) {
                continue;
            }
            let reason = reason_of(line);
            drops.push(Drop {
                tier: classify(&rel, &reason),
                file: rel.clone(),
                line: i + 1,
                reason,
            });
        }
    }
    drops.sort_by(|a, b| a.tier.cmp(&b.tier).then(a.file.cmp(&b.file)).then(a.line.cmp(&b.line)));
    drops
}

/// Render the inventory as RON (v2 Sean 07-29 spec: `.forge/run/silent_drops.ron`).
/// Hand-written — the shape is four fields, and a serde derive would buy a dependency
/// edge for nothing.
pub fn to_ron(drops: &[Drop]) -> String {
    let mut s = String::from("// qa drops — the `_ => {}` census.\n");
    s.push_str("(\n");
    let _ = writeln!(s, "  total: {},", drops.len());
    for t in [Tier::Dispatch, Tier::State, Tier::Lowering, Tier::Annotated] {
        let n = drops.iter().filter(|d| d.tier == t).count();
        let _ = writeln!(s, "  {}: {},", t.as_str().to_lowercase(), n);
    }
    s.push_str("  drops: [\n");
    for d in drops {
        let _ = writeln!(
            s,
            "    (file: {:?}, line: {}, tier: {}, reason: {:?}),",
            d.file,
            d.line,
            d.tier.as_str(),
            d.reason
        );
    }
    s.push_str("  ],\n)\n");
    s
}

/// `qa drops [--write]` — prints the tally; `--write` lands `.forge/run/silent_drops.ron`.
pub fn run(args: &[String]) -> i32 {
    let root = std::env::current_dir().unwrap_or_default();
    let drops = harvest(&root);
    let tally = |t: Tier| drops.iter().filter(|d| d.tier == t).count();
    println!(
        "drops {} — dispatch {} · state {} · lowering {} · annotated {}",
        drops.len(),
        tally(Tier::Dispatch),
        tally(Tier::State),
        tally(Tier::Lowering),
        tally(Tier::Annotated)
    );
    if args.iter().any(|a| a == "--write") {
        let p = root.join(".forge").join("run").join("silent_drops.ron");
        if let Some(d) = p.parent() {
            let _ = std::fs::create_dir_all(d);
        }
        match std::fs::write(&p, to_ron(&drops)) {
            Ok(()) => println!("wrote {}", p.display()),
            Err(e) => {
                eprintln!("qa drops: {e}");
                return 1;
            }
        }
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_matcher_takes_every_spelling_and_nothing_else() {
        assert!(drop_arm("            _ => {}"));
        assert!(drop_arm("_=>{}"));
        assert!(drop_arm("        _   =>   { }"));
        assert!(drop_arm("            _ => (),"));
        assert!(drop_arm("        _ => {} // GPU-only, skip"));
        // An arm that DOES something is not a drop.
        assert!(!drop_arm("            _ => return Err(e),"));
        assert!(!drop_arm("            _ => other(),"));
        assert!(!drop_arm("            Ok(()) => {}"));
        assert!(!drop_arm("// _ => {} in a comment"));
    }

    #[test]
    fn a_stated_reason_is_annotated_whatever_the_lane() {
        assert_eq!(classify("crates/forge-studio/src/main.rs", ""), Tier::Dispatch);
        assert_eq!(classify("crates/forge-studio/src/main.rs", "file open"), Tier::Annotated);
        assert_eq!(classify("crates/forge-sieve/src/world.rs", ""), Tier::State);
        assert_eq!(classify("crates/forge-vix/src/parse.rs", ""), Tier::Lowering);
    }

    #[test]
    fn reasons_come_off_the_trailing_comment() {
        assert_eq!(reason_of("_ => {} // GPU-only, skip"), "GPU-only, skip");
        assert_eq!(reason_of("_ => {}"), "");
    }

    #[test]
    fn harvest_reads_a_tree_and_the_ron_tally_matches() {
        // std-only fixture: process-unique dir under the OS temp root (no tempfile dep —
        // Crate Zero). Cleaned at the end; leftovers from a killed run are harmless.
        let base = std::env::temp_dir().join(format!("organs-drops-{}", std::process::id()));
        let src = base.join("crates").join("demo").join("src");
        std::fs::create_dir_all(&src).expect("mkdir");
        std::fs::write(
            src.join("main.rs"),
            "fn f(x: u8) { match x {\n 1 => (),\n _ => {}\n } }\n",
        )
        .expect("write");
        std::fs::write(
            src.join("world.rs"),
            "fn g(x: u8) { match x {\n _ => {} // documented\n } }\n",
        )
        .expect("write");

        let drops = harvest(&base);
        assert_eq!(drops.len(), 2, "{drops:?}");
        assert_eq!(drops[0].tier, Tier::Dispatch, "main.rs answers commands");
        assert_eq!(drops[0].line, 3);
        assert_eq!(drops[1].tier, Tier::Annotated, "a stated reason outranks the lane");
        assert_eq!(drops[1].reason, "documented");

        let r = to_ron(&drops);
        assert!(r.contains("total: 2,"), "{r}");
        assert!(r.contains("dispatch: 1,"), "{r}");
        assert!(r.contains("annotated: 1,"), "{r}");
        assert!(r.contains("tier: Annotated"), "{r}");

        let _ = std::fs::remove_dir_all(&base);
    }
}
