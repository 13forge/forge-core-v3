//! `cull` — the two-tier delete ladder, compiled (Sean 07-28).
//!
//! T1 CERTIFIED_TWIN = sha256 already in the keep-set → free cull + receipt.
//! T2 SOLE COPY = queued to `.forge/safe-to-delete.tsv`, never auto-removed.
//!
//! Ported 2026-08-17 from `F:\NewRepo\crates\forge-studio\src\twin_cull.rs`
//! (C06 donor cite) with three v3 adaptations: inline SHA-256 (no sha2 crate),
//! bounded work-list loop (no recursive directory walks per CLAUDE.md), and
//! `dispatch-gap` CLI dropped (depends on massread, not yet ported).

use std::collections::{HashMap, HashSet};
use std::io::Write;
use std::path::{Path, PathBuf};

// ── INLINE SHA-256 (Crate Zero: no external hash deps) ──────────────────────
/// Compute SHA-256 of a file, streamed for multi-GB files. Returns lowercase hex.
fn sha256(path: &Path) -> std::io::Result<String> {
    let mut f = std::fs::File::open(path)?;
    let mut state = Sha256State::new();
    let mut buf = [0u8; 65536];
    loop {
        let n = std::io::Read::read(&mut f, &mut buf)?;
        if n == 0 {
            break;
        }
        state.update(&buf[..n]);
    }
    Ok(state.finalize_hex())
}

/// Minimal SHA-256 state machine.
struct Sha256State {
    h: [u32; 8],
    len: u64,
    buf: [u8; 64],
    pos: usize,
}

impl Sha256State {
    /// Create a new SHA-256 state with standard initial values.
    fn new() -> Self {
        Sha256State {
            h: [0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19],
            len: 0,
            buf: [0; 64],
            pos: 0,
        }
    }

    /// Feed data into the hash.
    fn update(&mut self, data: &[u8]) {
        for &byte in data {
            self.buf[self.pos] = byte;
            self.pos += 1;
            if self.pos == 64 {
                self.process_block();
                self.pos = 0;
            }
        }
        self.len += data.len() as u64;
    }

    /// Finalize and return lowercase hex digest.
    fn finalize_hex(mut self) -> String {
        let bits = self.len.wrapping_mul(8);
        self.buf[self.pos] = 0x80;
        self.pos += 1;

        if self.pos > 56 {
            while self.pos < 64 {
                self.buf[self.pos] = 0;
                self.pos += 1;
            }
            self.process_block();
            self.pos = 0;
        }

        while self.pos < 56 {
            self.buf[self.pos] = 0;
            self.pos += 1;
        }

        // Append length in bits as big-endian u64.
        for i in 0..8 {
            self.buf[56 + i] = ((bits >> (56 - i * 8)) & 0xff) as u8;
        }
        self.process_block();

        let mut out = String::new();
        for &h in &self.h {
            for i in 0..4 {
                let byte = ((h >> (24 - i * 8)) & 0xff) as u8;
                out.push_str(&format!("{:02x}", byte));
            }
        }
        out
    }

    /// Process one 512-bit block.
    fn process_block(&mut self) {
        #[rustfmt::skip]
        const K: [u32; 64] = [
            0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
            0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
            0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
            0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
            0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
            0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
            0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
            0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
        ];

        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([
                self.buf[i * 4],
                self.buf[i * 4 + 1],
                self.buf[i * 4 + 2],
                self.buf[i * 4 + 3],
            ]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16].wrapping_add(s0).wrapping_add(w[i - 7]).wrapping_add(s1);
        }

        let (mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h) =
            (self.h[0], self.h[1], self.h[2], self.h[3], self.h[4], self.h[5], self.h[6], self.h[7]);

        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let temp1 = h.wrapping_add(s1).wrapping_add(ch).wrapping_add(K[i]).wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(maj);

            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }

        self.h[0] = self.h[0].wrapping_add(a);
        self.h[1] = self.h[1].wrapping_add(b);
        self.h[2] = self.h[2].wrapping_add(c);
        self.h[3] = self.h[3].wrapping_add(d);
        self.h[4] = self.h[4].wrapping_add(e);
        self.h[5] = self.h[5].wrapping_add(f);
        self.h[6] = self.h[6].wrapping_add(g);
        self.h[7] = self.h[7].wrapping_add(h);
    }
}

// ── THE THREE DRAIN VERDICTS (Sean 2026-08-02) ───────────────────────────────
// "DISPATCH GAP needs a verb and gate. DRIFT same. SCAFFOLD same." All three were
// being decided the wrong way round: DISPATCH_GAP was a tested pure fn that only
// ran inside `massread` so nothing else could ask it; DRIFT was an ad-hoc
// PowerShell hash loop retyped per session; SCAFFOLD was asked of a MODEL, which
// is absurd — "does this file declare public symbols" is a byte question with an
// exact answer. They live HERE because this file already owns twin-by-sha and the
// delete ladder; a module minted to hold them would be the tracker shard the
// harness gate refuses. The model keeps only what bytes cannot decide: whether a
// differently-named live file SUPERSEDES this one.

/// A drain-tree file versus the SoT tree, decided by sha256 alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Twin {
    /// A same-named SoT file hashes identically — nothing to drain.
    Identical,
    /// A same-named SoT file exists and the bytes differ.
    Drift,
    /// No same-named file in SoT.
    Sole,
}

impl Twin {
    /// String representation for ledgers and displays.
    pub fn as_str(self) -> &'static str {
        match self {
            Twin::Identical => "IDENTICAL",
            Twin::Drift => "DRIFT",
            Twin::Sole => "SOLE",
        }
    }
}

/// The file on its own terms. NOT a reachability verdict: everything in a drain
/// tree is caller-less because it is out of tree, and calling that DEAD is how
/// real work gets deleted (root#orphan-wire — EXISTS != REACHABLE).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Life {
    /// Declares public symbols, or carries a real body — capability either way.
    Live,
    /// No public surface and no body worth the name.
    Scaffold,
}

impl Life {
    /// String representation for ledgers and displays.
    pub fn as_str(self) -> &'static str {
        match self {
            Life::Live => "LIVE",
            Life::Scaffold => "SCAFFOLD",
        }
    }
}

/// Public items a Rust source declares, in source order, deduped.
///
/// A line scan over `pub ` heads, deliberately not a parse: a drain tree holds
/// files that do not compile in place (wrong module path, absent siblings), and a
/// verdict that needs the file to build cannot judge the tree it exists for.
pub fn pub_items(src: &str) -> Vec<String> {
    const KINDS: [&str; 7] = ["fn ", "struct ", "enum ", "trait ", "const ", "type ", "mod "];
    let mut out: Vec<String> = Vec::new();
    for line in src.lines() {
        // `pub(crate)` is not public surface for drain purposes.
        let Some(rest) = line.trim_start().strip_prefix("pub ") else {
            continue;
        };
        let Some(kind) = KINDS.iter().find(|k| rest.starts_with(**k)) else {
            continue;
        };
        let name: String = rest[kind.len()..]
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
            .collect();
        if !name.is_empty() && !out.contains(&name) {
            out.push(name);
        }
    }
    out
}

/// Count non-blank, non-comment lines in a source string.
pub fn body_lines(src: &str) -> usize {
    src.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with("//") && !l.starts_with("/*") && !l.starts_with('*'))
        .count()
}

/// Count `pub use` re-export statements.
///
/// SCAFFOLD only when a file declares NO public surface AND carries almost no
/// body. Either alone is not enough: a private-only module with 400 live lines is
/// real work, and a one-line `pub use` re-export is a wire.
/// A `pub use` re-export declares no item of its own, so [`pub_items`] cannot see
/// it — and a one-line re-export file would otherwise read SCAFFOLD and be
/// dropped. Wiring is surface.
pub fn re_exports(src: &str) -> usize {
    src.lines().filter(|l| l.trim_start().starts_with("pub use ")).count()
}

/// Classify source by public surface and body weight.
pub fn life_of(src: &str) -> Life {
    if pub_items(src).is_empty() && re_exports(src) == 0 && body_lines(src) < 5 {
        Life::Scaffold
    } else {
        Life::Live
    }
}

/// One census row describing a drain file's status.
#[derive(Debug, Clone)]
pub struct DrainRow {
    /// File base name.
    pub name: String,
    /// Twin verdict by file name and SHA-256.
    pub twin: Twin,
    /// Life verdict by public surface and body.
    pub life: Life,
    /// SHA-256 digest (lowercase hex).
    pub sha: String,
    /// Path of the live-side twin, if any.
    pub live_twin: Option<String>,
    /// Public items the drain file declares that its live twin does not.
    pub work_only: Vec<String>,
    /// Public items the live twin declares that the drain file does not.
    pub live_only: Vec<String>,
}

impl DrainRow {
    /// The action the two axes imply — a lookup, never a judgement.
    pub fn action(&self) -> &'static str {
        match (self.twin, self.life) {
            (Twin::Identical, _) => "DROP",
            (_, Life::Scaffold) => "DROP",
            (Twin::Sole, Life::Live) => "FOLD",
            (Twin::Drift, Life::Live) if self.work_only.is_empty() && !self.live_only.is_empty() => "DROP",
            (Twin::Drift, Life::Live) => "RECONCILE",
        }
    }
}

/// Every regular file under `root` (symlinks never followed).
///
/// Uses a bounded work-list loop with depth cap ~12 to avoid recursive
/// directory traversal (CLAUDE.md forbids recursion and glob walks).
fn files(root: &Path) -> Vec<(PathBuf, u64)> {
    const DEPTH_CAP: usize = 12;
    let mut out = Vec::new();
    let mut work = vec![(root.to_path_buf(), 0)];

    while let Some((path, depth)) = work.pop() {
        if depth > DEPTH_CAP {
            continue;
        }
        match std::fs::read_dir(&path) {
            Ok(entries) => {
                for entry in entries.flatten() {
                    if let Ok(meta) = entry.metadata() {
                        if meta.is_dir() && depth + 1 <= DEPTH_CAP {
                            work.push((entry.path(), depth + 1));
                        } else if meta.is_file() && meta.len() > 0 {
                            out.push((entry.path(), meta.len()));
                        }
                    }
                }
            }
            Err(_) => continue,
        }
    }
    out
}

/// Census one drain tree against one SoT tree, by file name then by sha.
pub fn drain_census(tree: &Path, sot: &Path) -> Vec<DrainRow> {
    let mut live: HashMap<String, Vec<(PathBuf, String)>> = HashMap::new();
    for (path, _) in files(sot) {
        if path.extension().and_then(|x| x.to_str()) != Some("rs") {
            continue;
        }
        if let (Some(name), Ok(h)) = (path.file_name().and_then(|n| n.to_str()), sha256(&path)) {
            live.entry(name.to_string()).or_default().push((path.clone(), h));
        }
    }
    let mut rows = Vec::new();
    let mut entries: Vec<_> = files(tree).into_iter().map(|(p, _)| p).collect();
    entries.sort();
    for p in entries {
        if p.extension().and_then(|x| x.to_str()) != Some("rs") {
            continue;
        }
        let (Some(name), Ok(sha), Ok(src)) =
            (p.file_name().and_then(|n| n.to_str()), sha256(&p), std::fs::read_to_string(&p))
        else {
            continue;
        };
        let (twin, live_twin) = match live.get(name) {
            None => (Twin::Sole, None),
            Some(c) => match c.iter().find(|(_, s)| *s == sha) {
                Some((path, _)) => (Twin::Identical, Some(path.display().to_string())),
                None => (Twin::Drift, Some(c[0].0.display().to_string())),
            },
        };
        let (mut work_only, mut live_only) = (Vec::new(), Vec::new());
        if twin == Twin::Drift {
            if let Some(t) = &live_twin {
                let mine = pub_items(&src);
                let theirs = std::fs::read_to_string(t).map(|s| pub_items(&s)).unwrap_or_default();
                work_only = mine.iter().filter(|i| !theirs.contains(i)).cloned().collect();
                live_only = theirs.iter().filter(|i| !mine.contains(i)).cloned().collect();
            }
        }
        rows.push(DrainRow {
            name: name.to_string(),
            twin,
            life: life_of(&src),
            sha,
            live_twin,
            work_only,
            live_only,
        });
    }
    rows
}

/// `cull census --tree <dir> --into <dir> [--tsv]` — the drain gate.
///
/// Exit: 0 = nothing owed · 1 = work to FOLD or RECONCILE · 2 = usage.
fn census_cli(args: &[String]) -> i32 {
    let pick = |n: &str| args.iter().position(|a| a == n).and_then(|i| args.get(i + 1));
    let (Some(tree), Some(sot)) = (pick("--tree"), pick("--into")) else {
        eprintln!("[cull census] needs --tree <dir> --into <dir> [--tsv]");
        return 2;
    };
    let rows = drain_census(Path::new(tree), Path::new(sot));
    if rows.is_empty() {
        eprintln!("[cull census] no .rs files under {tree}");
        return 2;
    }
    let tsv = args.iter().any(|a| a == "--tsv");
    let (mut fold, mut reconcile, mut drop_n) = (0, 0, 0);
    for r in &rows {
        match r.action() {
            "FOLD" => fold += 1,
            "RECONCILE" => reconcile += 1,
            _ => drop_n += 1,
        }
        if tsv {
            println!(
                "{}\t{}\t{}\t{}\t{}\t{}\t+{}/-{}",
                r.name,
                r.twin.as_str(),
                r.life.as_str(),
                r.action(),
                &r.sha[..16.min(r.sha.len())],
                r.live_twin.as_deref().unwrap_or("-"),
                r.work_only.len(),
                r.live_only.len()
            );
        }
    }
    eprintln!(
        "[cull census] {} file(s) · FOLD {fold} · RECONCILE {reconcile} · DROP {drop_n} · tree={tree} sot={sot}",
        rows.len()
    );
    // The gate: owed work is a nonzero exit, so a hook or a loop branches on it
    // without parsing a word of this output.
    i32::from(fold + reconcile > 0)
}

/// One scan root's verdict, printed as the receipt.
#[derive(Default)]
struct Tally {
    /// Count of T1 certified twins.
    t1_files: u64,
    /// Total bytes of T1 certified twins.
    t1_bytes: u64,
    /// Count of T2 sole copies.
    t2_files: u64,
    /// Total bytes of T2 sole copies.
    t2_bytes: u64,
}

/// The keep-set: sha256 → the one path that stays. Hashed in full because a
/// keep root is the live tree and is small next to the quarries it certifies.
fn keep_index(roots: &[PathBuf]) -> (HashSet<String>, HashSet<u64>) {
    let mut shas = HashSet::new();
    let mut lens = HashSet::new();
    for root in roots {
        for (path, len) in files(root) {
            lens.insert(len);
            if let Ok(h) = sha256(&path) {
                shas.insert(h);
            }
        }
    }
    (shas, lens)
}

/// `cull --keep <dir>… --scan <dir>… [--apply]`. Dry-run is the default: the
/// runtime bin never removes on a bare invocation (root#dev-mode).
///
/// Exit: 0 on success, 2 on usage error.
pub fn run(args: &[String]) -> i32 {
    // The drain verdicts ride the same verb as the delete ladder they feed.
    match args.first().map(String::as_str) {
        Some("census") => return census_cli(&args[1..]),
        Some("dispatch-gap") => {
            // v3: dispatch-gap dropped, depends on massread (not yet ported from C06).
            eprintln!("[cull dispatch-gap] not yet ported (needs massread module)");
            return 2;
        }
        _ => {}
    }
    let mut keep: Vec<PathBuf> = Vec::new();
    let mut scan: Vec<PathBuf> = Vec::new();
    let mut apply = false;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--keep" => keep.extend(it.next().map(PathBuf::from)),
            "--scan" => scan.extend(it.next().map(PathBuf::from)),
            "--apply" => apply = true,
            other => {
                eprintln!("cull: unknown arg {other}");
                return 2;
            }
        }
    }
    if scan.is_empty() {
        eprintln!("cull: --scan <dir> required (--keep <dir> repeatable, --apply to remove)");
        return 2;
    }

    let (keep_shas, keep_lens) = keep_index(&keep);
    println!(
        "KEEP-SET  roots={}  distinct-sha={}  distinct-len={}",
        keep.len(),
        keep_shas.len(),
        keep_lens.len()
    );

    // Sizes are the cheap sieve: a file whose length matches nothing in the
    // keep-set and nothing seen so far cannot be anyone's twin, so it is never
    // hashed. Everything below that bar is T2 by construction.
    let mut seen: HashMap<String, PathBuf> = HashMap::new();
    let mut seen_lens: HashSet<u64> = HashSet::new();
    let mut manifest: Vec<String> = Vec::new();
    let mut tally = Tally::default();

    for root in &scan {
        for (path, len) in files(root) {
            let could_twin = keep_lens.contains(&len) || !seen_lens.insert(len);
            if !could_twin {
                tally.t2_files += 1;
                tally.t2_bytes += len;
                continue;
            }
            let Ok(h) = sha256(&path) else { continue };
            let twin = keep_shas.contains(&h) || seen.contains_key(&h);
            if twin {
                if apply && std::fs::remove_file(&path).is_err() {
                    continue;
                }
                tally.t1_files += 1;
                tally.t1_bytes += len;
            } else {
                seen.insert(h.clone(), path.clone());
                tally.t2_files += 1;
                tally.t2_bytes += len;
                manifest.push(format!("{}\t{len}\t{h}", path.display()));
            }
        }
    }

    let gb = |b: u64| b as f64 / 1e9;
    println!(
        "T1 CERTIFIED-TWIN  files={}  {:.2}GB  {}",
        tally.t1_files,
        gb(tally.t1_bytes),
        if apply { "CULLED" } else { "dry-run" }
    );
    println!(
        "T2 SOLE-COPY       files={}  {:.2}GB  queued for Sean",
        tally.t2_files,
        gb(tally.t2_bytes)
    );

    let out = Path::new(".forge").join("safe-to-delete.tsv");
    if let Some(parent) = out.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    match std::fs::File::create(&out) {
        Ok(mut f) => {
            let _ = writeln!(f, "# T2 sole copies — delete stays Sean-only (root#delete)");
            for row in &manifest {
                let _ = writeln!(f, "{row}");
            }
            println!("MANIFEST {}  rows={}", out.display(), manifest.len());
        }
        Err(e) => eprintln!("cull: manifest write failed: {e}"),
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scaffold_needs_both_no_surface_and_no_body() {
        assert_eq!(life_of("// nothing\n"), Life::Scaffold);
        assert_eq!(life_of(""), Life::Scaffold);
        let private: String = (0..40).map(|i| format!("fn helper{i}() {{ let _ = {i}; }}\n")).collect();
        assert_eq!(life_of(&private), Life::Live, "private code is still code");
        assert_eq!(life_of("pub use crate::a::B;\n"), Life::Live, "a re-export is a wire");
        assert_eq!(life_of("pub fn real() {}\n"), Life::Live);
    }

    #[test]
    fn pub_items_reads_surface_not_privates_or_pub_crate() {
        let src = "pub fn a() {}\nfn b() {}\npub(crate) fn c() {}\npub struct D;\npub enum E {}\npub fn a() {}\n";
        assert_eq!(pub_items(src), vec!["a", "D", "E"], "deduped, ordered, pub-only");
    }

    #[test]
    fn the_two_axes_decide_the_action() {
        let row = |twin, life, w: &[&str], l: &[&str]| DrainRow {
            name: "x.rs".into(),
            twin,
            life,
            sha: "0".repeat(64),
            live_twin: Some("live/x.rs".into()),
            work_only: w.iter().map(|s| s.to_string()).collect(),
            live_only: l.iter().map(|s| s.to_string()).collect(),
        };
        assert_eq!(row(Twin::Sole, Life::Live, &[], &[]).action(), "FOLD");
        assert_eq!(row(Twin::Identical, Life::Live, &[], &[]).action(), "DROP");
        assert_eq!(row(Twin::Sole, Life::Scaffold, &[], &[]).action(), "DROP");
        // Live side is a strict superset: the drain file is an ancestor.
        assert_eq!(row(Twin::Drift, Life::Live, &[], &["extra"]).action(), "DROP");
        // Carries symbols SoT lacks: reconcile, never drop.
        assert_eq!(row(Twin::Drift, Life::Live, &["gold"], &[]).action(), "RECONCILE");
        assert_eq!(row(Twin::Drift, Life::Live, &["a"], &["b"]).action(), "RECONCILE");
    }

    #[test]
    fn drain_census_separates_identical_drift_and_sole_by_sha() {
        // v3: use std::env::temp_dir + process-id-unique subdir instead of tempfile.
        let pid = std::process::id();
        let base = std::env::temp_dir().join(format!("forge-drain-census-test-{pid}"));
        let (tree, sot) = (base.join("work"), base.join("sot"));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&tree).expect("create tree");
        std::fs::create_dir_all(&sot).expect("create sot");
        std::fs::write(tree.join("same.rs"), "pub fn same() {}\n").expect("write same");
        std::fs::write(sot.join("same.rs"), "pub fn same() {}\n").expect("write same");
        std::fs::write(tree.join("moved.rs"), "pub fn moved() {}\npub fn extra() {}\n").expect("write moved");
        std::fs::write(sot.join("moved.rs"), "pub fn moved() {}\n").expect("write moved");
        std::fs::write(tree.join("only.rs"), "pub fn only() {}\n").expect("write only");

        let rows = drain_census(&tree, &sot);
        let get = |n: &str| rows.iter().find(|r| r.name == n).expect("row").clone();
        assert_eq!(get("same.rs").twin, Twin::Identical);
        assert_eq!(get("same.rs").action(), "DROP");
        let moved = get("moved.rs");
        assert_eq!(moved.twin, Twin::Drift);
        assert_eq!(moved.work_only, vec!["extra"], "the symbol SoT lacks is NAMED");
        assert!(moved.live_only.is_empty());
        assert_eq!(moved.action(), "RECONCILE");
        assert_eq!(get("only.rs").twin, Twin::Sole);
        assert_eq!(get("only.rs").action(), "FOLD");
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn twin_is_t1_and_unique_is_t2() {
        // v3: use std::env::temp_dir + process-id-unique subdir instead of tempfile.
        let pid = std::process::id();
        let tmp = std::env::temp_dir().join(format!("forge-cull-test-{pid}"));
        let (keep, scan) = (tmp.join("keep"), tmp.join("scan"));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&keep).expect("create keep");
        std::fs::create_dir_all(&scan).expect("create scan");
        std::fs::write(keep.join("a.bin"), b"same-bytes").expect("write a");
        std::fs::write(scan.join("a.bin"), b"same-bytes").expect("write a");
        std::fs::write(scan.join("b.bin"), b"unique-bytes").expect("write b");

        let (shas, lens) = keep_index(&[keep.clone()]);
        assert_eq!(shas.len(), 1);
        assert!(lens.contains(&10));
        let twin = sha256(&scan.join("a.bin")).expect("hash a");
        assert!(shas.contains(&twin), "identical bytes must certify");
        let lone = sha256(&scan.join("b.bin")).expect("hash b");
        assert!(!shas.contains(&lone), "unique bytes must stay T2");
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
