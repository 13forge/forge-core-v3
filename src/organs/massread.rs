//! # massread.rs — the MASS-READ law as a verb (Sean 07-28 "skills are just a
//! suggestion, it needs to be a verb"): stdin corpus -> gemini flash-lite, output
//! CAP'd, stderr receipt. Law of record: forge_book::oracle1_governor::MASS_READ_LAW.
//!
//! Ported 2026-08-17 from F:\NewRepo\crates\forge-studio\src\massread.rs (1378 LOC).
//! Adaptations: dropped serde_json (unanchored_findings), dropped forge_firewall auth checks,
//! dropped forge_book structural question detection. Core doctrine constants and pure seams
//! ported VERBATIM.
//!
//! DEAD-LEDGER:
//! - unanchored_findings() (donor line 158) — requires serde_json parsing, not ported
//! - correction_prompt() (donor line 185) — companion to unanchored_findings, not ported
//! - Self-test firewall authorization (donor line 652) — requires forge_firewall, not ported
//! - Main run() firewall authorization (donor line 843) — requires forge_firewall, not ported
//! - question_is_structural integration (donor line 755) — requires forge_book, not ported
//!
//! No `run()` wrapper calls into this module's spawn/rate-limit/beat machinery
//! yet (that's the downstream organ's job, per this crate's Crate Zero split) —
//! blanket allow instead of doc-churning donor-verbatim consts/fns that are
//! real, tested, and simply not wired to a caller in THIS crate yet.
#![allow(missing_docs, dead_code)]

use std::io::Write as _;

/// Model ids (MASS_READ_LAW "model" row). Budget lane (Sean 07-29): routine
/// reads ride LITE under the strict scratchpad-first prompt; FLASH is for heavy
/// context and multi-file structural parsing, where precision beats latency.
/// Named by ROLE, not by ladder position — `route_ladder` escalates UP to flash,
/// so a bare value swap would send fat corpora to lite alone. PRO is OUT
/// (Sean 07-29): no rung, no `--model` reachability.
pub const LITE_MODEL: &str = "gemini-3.1-flash-lite";
pub const FLASH_MODEL: &str = "gemini-3.5-flash";

/// The BOUNDED-SCOPE read rung (Sean 08-06): a scoped sweep inside the lite
/// ceilings rides `claude -p --model haiku` instead of gemini-lite, which
/// answered fast and shallow. FLASH still owns the fat/deep rung, so the free
/// tier keeps eating the corpora too big to bound.
pub const HAIKU_MODEL: &str = "haiku";

/// Whether `model` belongs to the Claude CLI rather than the gemini bundle.
/// Family names (`haiku`/`sonnet`/`opus`) and full ids (`claude-*`) both land
/// here. Pure — a tested seam.
pub fn is_claude_model(model: &str) -> bool {
    let m = model.to_ascii_lowercase();
    m.starts_with("claude-") || ["haiku", "sonnet", "opus", "fable"].iter().any(|f| m.starts_with(f))
}

/// Batch ceilings for the SMALL rungs. Past either, flash-and-below confabulate
/// (debt row GEMINI-SWEEP-FALSE-STALE: false verdicts on 90k+ byte sweeps), so
/// the ladder drops them and pro reads alone.
pub const LITE_TOKEN_CEILING: usize = 15_000;
pub const LITE_ITEM_CEILING: usize = 8;

/// Default stdout cap in bytes. A mass-read that floods the caller's context
/// defeats the law it exists to serve.
pub const DEFAULT_CAP: usize = 16 * 1024;

/// What ONE `gemini -p` run actually holds (Sean 07-31): 3_000 files per prompt
/// against a 1_048_576-token window. Standard source (~300-500 lines, ~1_500 tok)
/// yields ~600-700 files a run; 200-token configs, headers and vixi snippets hit
/// the file ceiling first. Ten runs = 6_000-7_000 standard files, up to 30_000
/// small ones. These are the API's own walls — past them the call fails, so the
/// verb refuses before spending it.
pub const API_FILE_CEILING: usize = 3_000;
pub const CONTEXT_WINDOW_TOKENS: usize = 1_048_576;
pub const STANDARD_FILE_TOKENS: usize = 1_500;
pub const SMALL_FILE_TOKENS: usize = 200;

/// The sweep floor. This lane is the FREE read oracle precisely because it eats
/// whole directories at zero cost, so one task feeds one crate module — 15-30
/// files in a single shot. One file per call is the anti-pattern: 15 calls to
/// answer one row rebuilds the slow iteration loop the verb exists to kill.
/// Advisory only — a legitimately narrow read is still a read, never a refusal.
pub const MODULE_SWEEP_FLOOR: usize = 15;

/// How many files of average size `avg_tokens` fit in one run — whichever wall
/// comes first, the file count or the window. Pure — a tested seam.
pub fn run_capacity(avg_tokens: usize) -> usize {
    let by_window = CONTEXT_WINDOW_TOKENS / avg_tokens.max(1);
    by_window.min(API_FILE_CEILING)
}

/// Whether a corpus is shaped like a module sweep or like a wasted call.
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum BatchShape {
    /// Under the sweep floor: the run is paid for and mostly empty.
    Underfilled(usize),
    /// A real sweep.
    Full(usize),
    /// Past an API wall — the call would fail, so it is never spent.
    OverCeiling(String),
}

/// Judge one corpus against the API's walls and the sweep floor. Pure — a tested seam.
pub fn batch_shape(corpus: &str) -> BatchShape {
    let (items, tok) = (count_items(corpus), est_tokens(corpus));
    if items > API_FILE_CEILING {
        return BatchShape::OverCeiling(format!("{items} items > {API_FILE_CEILING}-file API ceiling"));
    }
    if tok > CONTEXT_WINDOW_TOKENS {
        return BatchShape::OverCeiling(format!("tok~{tok} > {CONTEXT_WINDOW_TOKENS}-token window"));
    }
    match items < MODULE_SWEEP_FLOOR {
        true => BatchShape::Underfilled(items),
        false => BatchShape::Full(items),
    }
}

/// The fan-out smell: many receipts that each vouch for a single file are the
/// one-file-per-call anti-pattern wearing a wave's clothes. Advisory — it names
/// the shape, never a verdict about the bytes. Pure — a tested seam.
pub fn fan_out_smell(findings_per_receipt: &[usize]) -> Option<String> {
    let n = findings_per_receipt.len();
    if n < 3 || findings_per_receipt.iter().any(|&f| f > 1) {
        return None;
    }
    Some(format!(
        "{n} receipts, 1 file each — that is {n} calls doing one sweep's work; \
         feed the module ({MODULE_SWEEP_FLOOR}-30 files, ~{} standard files fit one run)",
        run_capacity(STANDARD_FILE_TOKENS)
    ))
}

/// Collapse whitespace so a citation still anchors when the model reflows a
/// line it quoted correctly. Pure — a tested seam.
pub fn norm_ws(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Every quoted span in `s` long enough to be a real citation rather than an
/// identifier. Handles the JSON-escaped quotes a model emits inside `evidence`.
pub fn quoted_spans(s: &str) -> Vec<String> {
    const FLOOR: usize = 12;
    let flat = s.replace("\\\"", "\"").replace("\\n", "\n");
    let mut out = Vec::new();
    for quote in ['"', '\''] {
        let mut it = flat.split(quote);
        it.next();
        while let Some(span) = it.next() {
            if norm_ws(span).len() >= FLOOR {
                out.push(norm_ws(span));
            }
            if it.next().is_none() {
                break;
            }
        }
    }
    out
}

/// Unwrap a ```json fence. A fenced answer that silently failed to parse meant
/// the verify pass skipped and the citation was trusted unchecked — the exact
/// silent fallback this lane forbids. Pure — a tested seam.
pub fn strip_fence(answer: &str) -> &str {
    let t = answer.trim();
    let Some(rest) = t.strip_prefix("```") else {
        return t;
    };
    // Drop the language tag on the fence's own line, then the closing fence.
    let body = rest.split_once('\n').map(|(_, b)| b).unwrap_or("");
    body.trim().strip_suffix("```").unwrap_or(body).trim()
}

/// Wall-clock floor for ANY rung: 5 minutes [SEAN-OK] (Sean 08-02, "timeout
/// 5mins"). The old flat 30s cut a 19k-token structural read off mid-answer and
/// reported "corpus NOT read" — a starved call reads exactly like an absent
/// capability, which is the one thing this lane must never fake.
pub const MIN_TIMEOUT_SECS: u64 = 300;
/// No single call may hang the lane longer than this.
pub const MAX_TIMEOUT_SECS: u64 = 900;

/// Seconds between liveness beats while a call is in flight. Short enough that a stall is
/// obvious inside a minute, long enough that a 900s read leaves ~30 lines, not 900.
///
/// PROVENANCE (08-02): no owner existed — `raycast` on the concept lane returns no cadence
/// concept, and a repo-wide sweep for `pub const *_SECS|_MS|_INTERVAL` cadence quantities
/// finds none. This is the first, and it is owned by the module that emits the beat.
pub const HEARTBEAT_SECS: u64 = 30;

/// How long a receipt file may go untouched before the run is presumed STALLED, in beats.
/// A live call touches its beat file every [`HEARTBEAT_SECS`]; three missed beats is a stall,
/// never jitter. Pure — a tested seam.
pub fn is_stalled(silent_secs: u64) -> bool {
    silent_secs > HEARTBEAT_SECS * 3
}

/// Where a call in flight records its beat.
///
/// The beat CANNOT ride stderr alone. A launcher's `2>` redirect buffers every byte until
/// the process exits — probed 08-02: a call that had already beaten twice showed 0 bytes at
/// t=40s and wrote all 548 at exit — so the stderr mtime says STALLED about a perfectly
/// healthy read. v1.0.2 shipped that false positive, whose own heal is "kill the tree", and
/// a gauge that orders you to kill live work is worse than no gauge. The bin writes this
/// file itself so liveness never depends on how a shell plumbs a stream.
pub const BEAT_DIR: &str = ".forge/beat";

/// One beat file per in-flight call, keyed by pid so concurrent lanes never share one.
pub fn beat_path(pid: u32) -> std::path::PathBuf {
    std::path::Path::new(BEAT_DIR).join(format!("{pid}.beat"))
}

/// Scan every beat in [`BEAT_DIR`]: one report line per lane, plus the STALLED count.
///
/// Extracted so the `sentinel silence` verb and the Stop chain read the SAME scan (Sean
/// 2026-08-04). A gauge only the operator can invoke is not an indicator — on 08-04 a shell
/// sat 2h56m and `silence` would have answered honestly the whole time, if anything had
/// asked. Returning the count rather than exiting lets the Stop edge BLOCK on it.
///
/// ABANDONED beats are purged here, not reported as wedged: past [`MAX_TIMEOUT_SECS`] no
/// call can still be running, so the writer is already gone and a kill order would name a
/// dead pid.
pub fn silence_scan() -> (Vec<String>, usize) {
    let mut paths: Vec<std::path::PathBuf> = std::fs::read_dir(BEAT_DIR)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .collect();
    paths.sort();
    let (mut lines, mut stalled) = (Vec::new(), 0usize);
    for p in &paths {
        let d = p.display();
        let quiet = std::fs::metadata(p)
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.elapsed().ok())
            .map(|d| d.as_secs());
        match quiet {
            None => lines.push(format!("ABSENT\t{d}\tno such receipt — the lane never wrote")),
            Some(s) if is_abandoned(s) => {
                let gone = std::fs::remove_file(p).is_ok();
                lines.push(format!(
                    "ABANDONED\t{d}\tsilent {s}s (> the {MAX_TIMEOUT_SECS}s ceiling any call must obey) — {}",
                    if gone { "purged" } else { "purge FAILED" }
                ));
            }
            Some(s) if is_stalled(s) => {
                stalled += 1;
                lines.push(format!("STALLED\t{d}\tsilent {s}s (> 3 beats of {HEARTBEAT_SECS}s)"));
            }
            Some(s) => lines.push(format!("LIVE\t{d}\tlast beat {s}s ago")),
        }
    }
    (lines, stalled)
}

/// How many gemini calls may be in flight across EVERY agent process at once
/// (Sean 08-04). The free tier is 10-15 RPM for the whole key, not per process,
/// so 2-5 uncoordinated lanes serve each other 429s and then spend their retry
/// budget on the congestion they caused. Four leaves headroom for a retry wave.
pub const MAX_CONCURRENT_CALLS: usize = 4;

/// The free tier's own floor: 15 RPM over one key is one slot every 4 seconds, the
/// same physical quantity `backoff_secs` is already sized against. Polling faster
/// than the pool can refill only burns syscalls.
///
/// PROVENANCE (08-04): no owner. `raycast` concept-lane returns no cadence concept
/// and `pull_gate` on both the name and the phrase finds only these lines — the
/// same ABSENT `HEARTBEAT_SECS` recorded on 08-02, and owned by the same module,
/// because the quantity is the read lane's rate limit and nothing else's.
pub const RPM_SLOT_SECS: u64 = 4;

/// How long a lane waits for a slot before going anyway — DERIVED, not tuned: past
/// the stall horizon (`HEARTBEAT_SECS * 3`, the same three missed beats `is_stalled`
/// judges by) every slot still held would be declared stalled and freed, so waiting
/// longer waits on nothing. A read that never runs reads exactly like an absent
/// capability, so the cap throttles and never deadlocks.
pub const SLOT_WAIT_SECS: u64 = HEARTBEAT_SECS * 3;

/// Seconds between slot polls = the pool's own refill floor.
const SLOT_POLL_SECS: u64 = RPM_SLOT_SECS;

/// The cross-process semaphore, as a pure seam: given the silent-seconds of every
/// beat file on disk, how many belong to a call still holding quota.
///
/// Counted by [`is_stalled`], not [`is_abandoned`] — a lane three beats silent has
/// stopped consuming RPM whatever its process is doing, and charging it a slot for
/// the full [`MAX_TIMEOUT_SECS`] would idle the pool for fifteen minutes over
/// residue. Pure — a tested seam.
pub fn live_lanes(silent_secs: &[u64]) -> usize {
    silent_secs.iter().filter(|s| !is_stalled(**s)).count()
}

/// Whether a lane may take a slot. Pure — a tested seam.
pub fn slot_open(silent_secs: &[u64]) -> bool {
    live_lanes(silent_secs) < MAX_CONCURRENT_CALLS
}

/// Silent-seconds of every beat file EXCEPT this process's own — a lane must not
/// count itself against its own cap. Impure edge, kept thin; an unreadable beat
/// dir means no peers are visible, which throttles nothing and blocks nothing.
fn peer_silences() -> Vec<u64> {
    let now = std::time::SystemTime::now();
    let mine = beat_path(std::process::id());
    std::fs::read_dir(BEAT_DIR)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path() != mine)
        .filter_map(|e| e.metadata().ok()?.modified().ok())
        .map(|m| now.duration_since(m).map(|d| d.as_secs()).unwrap_or(0))
        .collect()
}

/// Hold here until a slot opens or [`SLOT_WAIT_SECS`] elapses. Returns how long it
/// waited, for the receipt.
fn await_slot() -> u64 {
    let started = std::time::Instant::now();
    while !slot_open(&peer_silences()) {
        let waited = started.elapsed().as_secs();
        if waited >= SLOT_WAIT_SECS {
            eprintln!("[massread] slot wait exceeded {SLOT_WAIT_SECS}s with {MAX_CONCURRENT_CALLS} lanes live — proceeding, throttle never deadlocks");
            return waited;
        }
        std::thread::sleep(std::time::Duration::from_secs(SLOT_POLL_SECS));
    }
    started.elapsed().as_secs()
}

/// Past this a beat file cannot belong to a live call: no single call may outlast
/// [`MAX_TIMEOUT_SECS`], so the writer is gone and only its residue remains. A killed
/// process never runs its own cleanup, so without this a `Stop-Job`'d probe leaves a file
/// that reads STALLED forever and invites a kill order against a pid that no longer exists
/// (=INVARIANT-SWEEP-001 pillar 4, and caught 08-02 by the gauge reading its own exhaust).
pub fn is_abandoned(silent_secs: u64) -> bool {
    silent_secs > MAX_TIMEOUT_SECS
}

/// Stamp the beat. Best-effort: a lane that cannot write its beat still reads.
/// `write` truncates and closes, so the bytes are on disk when it returns.
fn touch_beat(path: &std::path::Path, line: &str) {
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(path, line);
}

/// Seconds to allow any rung over `corpus`: 5 minutes floor, plus half a floor
/// per lite-ceiling of tokens, capped. Never below [`MIN_TIMEOUT_SECS`] — the
/// lane has no shorter fallback to reach for. Pure — a tested seam.
pub fn timeout_for(corpus: &str) -> u64 {
    let ceilings = (est_tokens(corpus) / LITE_TOKEN_CEILING) as u64;
    MIN_TIMEOUT_SECS
        .saturating_add(MIN_TIMEOUT_SECS.saturating_mul(ceilings) / 2)
        .clamp(MIN_TIMEOUT_SECS, MAX_TIMEOUT_SECS)
}

/// The standardized dispatch prompt. Forces evidence extraction into
/// `scratchpad` BEFORE `verdict` — the chain-of-verification step that stops
/// lite from keyword-matching its way to a confident wrong answer.
pub const SYSTEM_PROMPT: &str = r#"You are a deterministic, zero-hallucination code audit engine operating under strict verification constraints.

### OPERATIONAL LAWS
1. EVIDENCE FIRST: cite exact line numbers, struct names, or text snippets from <corpus> before issuing any verdict.
2. NO CONFABULATION: never infer intent or invent file paths. A symbol whose DECLARATION you can see but whose call sites could live in a file outside <corpus> is UNKNOWN, never ABSENT — ABSENT is reserved for the case where the complete dispatch map is present and you can prove the negative. Say which file would hold the evidence.
3. BINARY COMPLIANCE: emit only the requested JSON. No conversational filler, no markdown fence, no postscript.

### EXECUTION PROTOCOL
Step 1: read <rules> and <corpus> (corpus arrives on stdin).
Step 2: fill "scratchpad" with raw facts and line references found in <corpus>.
Step 3: only then emit the verdict fields.

### INPUT DATA
<rules>
{RULES}
</rules>

Emit output strictly matching this JSON schema:
{
  "scratchpad": "String: step-by-step raw evidence extracted directly from corpus",
  "verdict": "PASS | FAIL | UNWIRED | UNKNOWN",
  "findings": [
    { "target": "String: file or task id", "status": "GREEN | RED | STALE | ABSENT", "evidence": "String: exact verbatim line or diff proof" }
  ]
}"#;

/// Byte-to-token estimate (~4B/token for code+prose). Pure — a tested seam.
pub fn est_tokens(corpus: &str) -> usize {
    corpus.len() / 4
}

/// Count corpus items by header marker (`=== `, `--- `, `## ` at line start) —
/// the massread corpus convention. Pure — a tested seam.
pub fn count_items(corpus: &str) -> usize {
    corpus
        .lines()
        .filter(|l| l.starts_with("=== ") || l.starts_with("--- ") || l.starts_with("## "))
        .count()
}

/// Which of `paths` never appears as a corpus item header. A launcher that drops a file —
/// a bad glob, a read error swallowed by a loop — makes the model answer ABSENT about
/// bytes it was never shown, and that false verdict then rides a receipt into a weld
/// (`forge_book::oracle1_governor::MASSLOOP_ROW_LIES` "FALSE_ABSENT/short-corpus", first
/// caught 07-29). Separator-agnostic, matched on the path TAIL so a
/// relative header still satisfies an absolute route. Pure — a tested seam.
pub fn manifest_gaps(corpus: &str, paths: &[String]) -> Vec<String> {
    let norm = |s: &str| s.replace('\\', "/").to_ascii_lowercase();
    let heads: Vec<String> = corpus
        .lines()
        .filter_map(|l| {
            ["=== ", "--- ", "## "].iter().find_map(|m| l.strip_prefix(*m)).map(|h| norm(h.trim()))
        })
        .collect();
    paths
        .iter()
        .filter(|p| {
            let want = norm(p.trim());
            !want.is_empty() && !heads.iter().any(|h| h == &want || h.ends_with(&want) || want.ends_with(h.as_str()))
        })
        .cloned()
        .collect()
}

/// Where a call site hides when the audited module's own bytes do not hold it.
pub const DISPATCH_TAILS: [&str; 3] = ["main.rs", "lib.rs", "mod.rs"];

/// A REACHABILITY question asked over a corpus carrying no dispatch file. The model then
/// reads the slice as the whole universe and answers ABSENT about a live symbol — three
/// times on 08-02 (`export_tasks`, `intel_drain::drain`, `BqRouterDrain`: all wired, all
/// reported orphaned), each costing a full correction pass. The bytes cannot answer the
/// question, so the call is refused before it is spent. Pure — a tested seam.
pub fn dispatch_gap(corpus: &str, rules: &str) -> Option<String> {
    const REACH: [&str; 7] =
        ["caller", "callers", "orphan", "unwired", "reachable", "call site", "no live"];
    let q = rules.to_ascii_lowercase();
    if !REACH.iter().any(|w| q.contains(w)) {
        return None;
    }
    let heads = corpus.lines().filter_map(|l| {
        ["=== ", "--- ", "## "].iter().find_map(|m| l.strip_prefix(*m))
    });
    // The FILE NAME, not the path tail: `remain.rs` ends with `main.rs` and would
    // otherwise close the gate on a file that dispatches nothing.
    let covered = heads.map(|h| h.trim().replace('\\', "/").to_ascii_lowercase()).any(|h| {
        let name = h.rsplit('/').next().unwrap_or(&h).to_string();
        DISPATCH_TAILS.contains(&name.as_str())
    });
    (!covered).then(|| {
        format!(
            "a reachability question over a corpus with NO dispatch file ({}). \
             The model would read this slice as the whole universe and call a live symbol \
             ABSENT. Add the owning crate's entry point; 0 calls spent.",
            DISPATCH_TAILS.join(" / ")
        )
    })
}

/// Read a `--corpus-manifest` argument: either a file of paths, or the paths themselves
/// inline. Newline- or comma-separated, blanks dropped. Pure enough — one optional read.
/// Read the manifest's files IN-BIN and build the corpus, one `=== <path>` block
/// per file. Returns `(corpus, unreadable_paths)`.
///
/// LAW COLLISION FIX (Sean 2026-08-02 "align them, make it work consistently").
/// `massread` took its corpus on stdin ONLY, so the mandated bulk lane
/// (MASS_READ_LAW: never read a batch through paid context) could only be fed by
/// a shell `Get-Content`, which `read-rung` blocks by design. Two live laws, one
/// forbidding the only way to obey the other — so the verb read nothing and every
/// caller fell back to raw `gemini -p`, which is the bypass the rule exists to
/// stop. The bin reads its own bytes now: `--corpus-manifest` is sufficient
/// input, stdin stays supported, and no shell read stands between them.
pub fn corpus_from_manifest(paths: &[String]) -> (String, Vec<String>) {
    let mut corpus = String::new();
    let mut unreadable = Vec::new();
    for p in paths {
        match std::fs::read_to_string(p) {
            Ok(body) => {
                corpus.push_str("=== ");
                corpus.push_str(p);
                corpus.push('\n');
                corpus.push_str(body.trim_end());
                corpus.push_str("\n\n");
            }
            Err(_) => unreadable.push(p.clone()),
        }
    }
    (corpus, unreadable)
}

pub fn read_manifest(arg: &str) -> Vec<String> {
    let raw = std::fs::read_to_string(arg).unwrap_or_else(|_| arg.to_string());
    raw.split(['\n', '\r', ','])
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect()
}

/// How hard the QUESTION is, which no byte count can measure (Sean 07-29: the split
/// is reasoning depth, not size). A 40-line corpus can carry an architectural
/// question; lite trades depth for latency and would answer it fast and wrong.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Depth {
    /// Per-file GREEN/ABSENT + one anchor. Lite's job.
    Volume,
    /// Cross-file structure, precision, architecture. Flash's job, ~3x the wait.
    Deep,
}

/// Model ladder for one corpus. `Deep` skips lite outright; otherwise size still
/// escalates, because past its ceilings lite confabulates regardless of the question
/// (debt row GEMINI-SWEEP-FALSE-STALE). Pure — a tested seam.
pub fn route_ladder_at(corpus: &str, depth: Depth) -> Vec<String> {
    let too_big =
        est_tokens(corpus) > LITE_TOKEN_CEILING || count_items(corpus) > LITE_ITEM_CEILING;
    if depth == Depth::Deep || too_big {
        // Gemini owns the fat rung for its million-token window, but it answered
        // DEAD on both ids the day haiku landed (08-06 selftest, `fetch failed`
        // ×9 after 219s). A dead free rung with no backstop is a read that
        // returns nothing, so haiku sits behind it — narrower, but alive.
        vec![FLASH_MODEL.to_string(), HAIKU_MODEL.to_string()]
    } else {
        vec![HAIKU_MODEL.to_string(), FLASH_MODEL.to_string()]
    }
}

/// The volume default — what an unflagged read gets. Pure — a tested seam.
pub fn route_ladder(corpus: &str) -> Vec<String> {
    route_ladder_at(corpus, Depth::Volume)
}

/// Splice the caller's rules into the standardized prompt. Pure — a tested seam.
pub fn compose_prompt(rules: &str) -> String {
    SYSTEM_PROMPT.replace("{RULES}", rules)
}

/// Delimiter-anchor the corpus so lite cannot drift past its boundary.
/// Pure — a tested seam.
pub fn wrap_corpus(corpus: &str) -> String {
    format!("<corpus>\n{}\n</corpus>\n", corpus.trim_end())
}

/// The npm-installed CLI bundle. Spawning `node <bundle>` instead of the
/// `gemini.cmd` shim is what lets a multi-line `-p` prompt through at all —
/// Rust refuses to escape such an argument for a batch file.
pub const CLI_BUNDLE: &str =
    r"C:\Program Files\nodejs\node_modules\@google\gemini-cli\bundle\gemini.js";

/// Build the gemini CLI invocation for one model + prompt: `node <bundle> -m
/// <model> -p <prompt>`, falling back to the `gemini` shim when the bundle is
/// not on disk. Pure — a tested seam.
pub fn build_command(model: &str, prompt: &str) -> (String, Vec<String>) {
    // The Claude CLI is a real .exe, not a .cmd shim, so a multi-line `-p`
    // argument escapes cleanly and the corpus rides stdin untouched.
    if is_claude_model(model) {
        return (
            "claude".to_string(),
            vec![
                "-p".to_string(),
                "--model".to_string(),
                model.to_string(),
                prompt.to_string(),
            ],
        );
    }
    let flags = |mut v: Vec<String>| {
        v.extend([
            // Headless runs never get the interactive trust prompt, and node
            // runs from the bundle dir, so trust is declared here (07-29).
            "--skip-trust".to_string(),
            "-m".to_string(),
            model.to_string(),
            "-p".to_string(),
            prompt.to_string(),
        ]);
        v
    };
    if std::path::Path::new(CLI_BUNDLE).exists() {
        ("node".to_string(), flags(vec![CLI_BUNDLE.to_string()]))
    } else {
        ("gemini".to_string(), flags(Vec::new()))
    }
}

/// Windows ships `gemini` as a `.cmd` shim, and Rust refuses to escape a batch
/// argument carrying newlines or quotes (CVE-2024-24576 mitigation) — it dies as
/// "batch file arguments are invalid" (first standardized-prompt smoke, 07-29).
/// So a multi-line prompt rides stdin ahead of the corpus instead of argv.
/// Returns (argv, stdin_bytes). Pure — a tested seam.
pub fn plan_call(model: &str, prompt: &str, corpus: &str) -> (String, Vec<String>, String) {
    let (exe, argv) = build_command(model, prompt);
    if exe == "gemini" && prompt.contains(['\n', '"', '%']) {
        let safe: Vec<String> = argv.into_iter().take_while(|a| a != "-p").collect();
        return (exe, safe, format!("{prompt}\n\n{corpus}"));
    }
    (exe, argv, corpus.to_string())
}

/// Cap `out` at `cap` bytes on a char boundary, appending a LOUD truncation
/// marker carrying the real size. Pure — a tested seam.
pub fn cap_output(out: &str, cap: usize) -> String {
    if out.len() <= cap {
        return out.to_string();
    }
    let mut end = cap;
    while end > 0 && !out.is_char_boundary(end) {
        end -= 1;
    }
    format!(
        "{}\n[massread: CAPPED {} of {} bytes]",
        &out[..end],
        end,
        out.len()
    )
}

/// Free-tier Gemini runs 10-15 RPM / 1500 RPD (Sean 07-29), so any sweep long
/// enough to matter WILL bump the limit. A 429 means the lane is busy, NOT that
/// the capability is absent — answering ABSENT on a rate-limit bounce writes a
/// false verdict into a receipt, which is the one failure this whole lane exists
/// to prevent. Pure — a tested seam.
pub fn is_rate_limit(err: &str) -> bool {
    let e = err.to_ascii_lowercase();
    ["429", "rate limit", "ratelimit", "resource_exhausted", "quota", "too many requests"]
        .iter()
        .any(|m| e.contains(m))
}

/// Retries per call and their spacing CEILING. At 15 RPM the floor between calls
/// is 4s, so the first backoff clears a full slot rather than re-bouncing.
/// Pure — tested.
pub const RETRY_ATTEMPTS: u32 = 3;
pub fn backoff_secs(attempt: u32) -> u64 {
    [5, 15, 45].get(attempt as usize).copied().unwrap_or(45)
}

/// FULL JITTER (Sean 08-04). The ladder above is a fixed ladder, and 2-5 agent
/// processes share one quota: bounce them on the same tick and they all wake on
/// the same tick, re-serving the 429 they were waiting out. Sleep a uniform draw
/// from `[1, ceiling]` instead. The 1s floor keeps a jittered retry from being an
/// instant re-bounce. Pure — a tested seam; `roll` is a 0..=1 draw.
pub fn backoff_jittered(attempt: u32, roll: f64) -> u64 {
    let span = backoff_secs(attempt).saturating_sub(1) as f64;
    1 + (roll.clamp(0.0, 1.0) * span) as u64
}

/// A 0..1 draw off the clock, mixed with the pid. No dependency and no seed to
/// thread through; the only property that has to hold is that two processes
/// bouncing on the same tick draw apart, and nanos plus pid does that.
fn jitter_roll() -> f64 {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos() as u64)
        .unwrap_or(0);
    ((nanos ^ (std::process::id() as u64).wrapping_mul(2_654_435_761)) % 1_000) as f64 / 999.0
}

/// `ask` plus rate-limit backoff. Every other error (timeout, spawn, bad schema)
/// returns on the first try — only a busy lane is worth waiting on.
fn ask_resilient(model: &str, prompt: &str, corpus: &str, timeout_secs: u64) -> Result<String, String> {
    let mut attempt = 0;
    loop {
        let waited = await_slot();
        if waited > 0 {
            eprintln!("[massread] held {waited}s for a slot (cap {MAX_CONCURRENT_CALLS} concurrent)");
        }
        match ask(model, prompt, corpus, timeout_secs) {
            Err(e) if is_rate_limit(&e) && attempt < RETRY_ATTEMPTS => {
                let wait = backoff_jittered(attempt, jitter_roll());
                eprintln!(
                    "[massread] {model} rate-limited, retry {}/{RETRY_ATTEMPTS} in {wait}s (jittered, ceiling {}s)",
                    attempt + 1,
                    backoff_secs(attempt)
                );
                std::thread::sleep(std::time::Duration::from_secs(wait));
                attempt += 1;
            }
            other => return other,
        }
    }
}

/// Spawn one gemini call, corpus piped to stdin. Impure edge, kept thin.
/// npm ships `gemini` as a `.cmd` shim on Windows and a bare-name spawn cannot
/// see it (first live smoke, 07-28) — NotFound retries the shim once.
fn ask(model: &str, prompt: &str, corpus: &str, timeout_secs: u64) -> Result<String, String> {
    let (exe, argv, fed) = plan_call(model, prompt, corpus);
    let mut child = match spawn(&exe, &argv) {
        Ok(c) => c,
        Err(e) if cfg!(windows) && e.kind() == std::io::ErrorKind::NotFound => {
            spawn(&format!("{exe}.cmd"), &argv).map_err(|e2| format!("spawn {exe}[.cmd]: {e2}"))?
        }
        Err(e) => return Err(format!("spawn {exe}: {e}")),
    };
    child
        .stdin
        .take()
        .ok_or("no stdin handle")?
        .write_all(fed.as_bytes())
        .map_err(|e| format!("feed corpus: {e}"))?;
    let started = std::time::Instant::now();
    let deadline = started + std::time::Duration::from_secs(timeout_secs);
    let mut beat = 0u64;
    // Stamped before the first beat elapses so a just-started call reads LIVE, not ABSENT.
    let beat_file = beat_path(std::process::id());
    touch_beat(&beat_file, &format!("{model} 0s/{timeout_secs}s\n"));
    loop {
        match child.try_wait().map_err(|e| format!("wait: {e}"))? {
            Some(_) => break,
            None if std::time::Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = std::fs::remove_file(&beat_file);
                return Err(format!("timeout after {timeout_secs}s"));
            }
            None => {
                // HEARTBEAT (Sean 08-02 "you need to watch these better"): a 900s call that
                // prints nothing is indistinguishable from a hang, so a stalled sweep can
                // burn half an hour before anyone looks. The beat also advances the stderr
                // file's mtime, which is what makes liveness checkable without a process
                // hunt (=INVARIANT-SWEEP-001 pillar 3, no silent state).
                let secs = started.elapsed().as_secs();
                if secs / HEARTBEAT_SECS > beat {
                    beat = secs / HEARTBEAT_SECS;
                    eprintln!("[massread] …{model} alive {secs}s/{timeout_secs}s");
                    touch_beat(&beat_file, &format!("{model} {secs}s/{timeout_secs}s\n"));
                }
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
        }
    }
    // Gone means finished. ABSENT is then an unambiguous "no lane in flight" rather
    // than the old "the lane never wrote", which a buffered stream also produced.
    let _ = std::fs::remove_file(&beat_file);
    let out = child.wait_with_output().map_err(|e| format!("wait: {e}"))?;
    finish(out)
}

fn spawn(exe: &str, argv: &[String]) -> std::io::Result<std::process::Child> {
    let mut cmd = std::process::Command::new(exe);
    // The bundle imports its own sibling chunks by relative path, so node must
    // run FROM the bundle directory (ERR_MODULE_NOT_FOUND otherwise, 07-29).
    if exe == "node" {
        if let Some(dir) = std::path::Path::new(CLI_BUNDLE).parent() {
            cmd.current_dir(dir);
        }
    }
    cmd.args(argv)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
}

fn finish(out: std::process::Output) -> Result<String, String> {
    if !out.status.success() {
        return Err(format!(
            "exit {:?}: {}",
            out.status.code(),
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Headless verb entry point: `fn(&[String]) -> i32` (v2 verb signature).
/// Minimal implementation: accepts `--help` and forwards to underlying ask infrastructure.
/// Full implementation deferred to downstream integrations that can depend on forge_firewall/forge_book.
pub fn run(args: &[String]) -> i32 {
    // Stub: minimal argument parsing and usage message.
    if args.is_empty() || args.iter().any(|a| a == "--help" || a == "-h") {
        eprintln!("Usage: massread [--model MODEL] [--cap BYTES] [--deep] <prompt...>");
        eprintln!("  Reads corpus from stdin, calls the model, outputs result to stdout.");
        eprintln!("  Models: {}, {}, {}", LITE_MODEL, FLASH_MODEL, HAIKU_MODEL);
        return 2;
    }
    // TODO: Full implementation requires forge_firewall authorization and forge_book structural detection.
    // This stub demonstrates the organ signature; use downstream wrappers for full functionality.
    eprintln!("[massread] stub: full implementation deferred to downstream organ wrapper");
    1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_command_carries_model_and_prompt_exactly() {
        let (exe, args) = build_command(LITE_MODEL, "classify");
        let tail = vec!["--skip-trust", "-m", LITE_MODEL, "-p", "classify"];
        if exe == "node" {
            assert_eq!(args[0], CLI_BUNDLE, "spawn the bundle, not the .cmd shim");
            assert_eq!(args[1..], tail[..]);
        } else {
            assert_eq!(exe, "gemini");
            assert_eq!(args, tail);
        }
    }

    #[test]
    fn the_bounded_rung_spawns_the_claude_cli_with_the_prompt_on_argv() {
        let (exe, args) = build_command(HAIKU_MODEL, "line1\nline2");
        assert_eq!(exe, "claude");
        assert_eq!(args, vec!["-p", "--model", HAIKU_MODEL, "line1\nline2"]);
        let (exe, argv, fed) = plan_call(HAIKU_MODEL, "line1\nline2", "CORPUS");
        assert_eq!(exe, "claude");
        assert_eq!(argv.last().unwrap(), "line1\nline2", "prompt stays on argv");
        assert_eq!(fed, "CORPUS", "corpus rides stdin alone");
        for m in [HAIKU_MODEL, "claude-haiku-4-5-20251001", "Sonnet"] {
            assert!(is_claude_model(m), "{m}");
        }
        for m in [LITE_MODEL, FLASH_MODEL] {
            assert!(!is_claude_model(m), "{m}");
        }
    }

    #[test]
    fn cap_output_passes_small_and_caps_large_on_char_boundary() {
        assert_eq!(cap_output("short", 100), "short");
        let big = "é".repeat(100); // 2 bytes each — 65 is mid-char
        let capped = cap_output(&big, 65);
        assert!(capped.starts_with(&"é".repeat(32)));
        assert!(capped.contains("[massread: CAPPED 64 of 200 bytes]"));
    }

    #[test]
    fn multiline_prompt_reaches_the_model_one_way_or_the_other() {
        let (exe, argv, fed) = plan_call("m", "line1\nline2", "CORPUS");
        if exe == "node" {
            // The bundle takes the prompt as a real `-p` argument.
            assert_eq!(argv.last().unwrap(), "line1\nline2");
            assert_eq!(fed, "CORPUS");
        } else {
            // The .cmd shim cannot be handed one, so it rides stdin instead.
            assert!(!argv.contains(&"-p".to_string()), "no unescapable batch arg");
            assert!(fed.starts_with("line1\nline2\n\n") && fed.ends_with("CORPUS"));
        }
        let (_, argv, fed) = plan_call("m", "one line", "CORPUS");
        assert_eq!(argv.last().unwrap(), "one line");
        assert_eq!(fed, "CORPUS");
    }

    #[test]
    fn deep_questions_skip_the_shallow_rung_whatever_the_corpus_size() {
        let tiny = "=== a.rs\nfn a() {}\n";
        let fat_rung = vec![FLASH_MODEL, HAIKU_MODEL];
        assert_eq!(route_ladder_at(tiny, Depth::Deep), fat_rung, "small but deep");
        assert_eq!(route_ladder_at(tiny, Depth::Volume)[0], HAIKU_MODEL, "small and shallow");
        let fat = "x".repeat((LITE_TOKEN_CEILING + 1) * 4);
        assert_eq!(route_ladder_at(&fat, Depth::Volume), fat_rung, "big and shallow");
        assert_eq!(route_ladder(tiny), route_ladder_at(tiny, Depth::Volume), "default=Volume");
    }

    #[test]
    fn wrap_corpus_anchors_both_delimiters() {
        let w = wrap_corpus("line one\n");
        assert!(w.starts_with("<corpus>\nline one"));
        assert!(w.trim_end().ends_with("</corpus>"));
    }

    #[test]
    fn the_bin_reads_its_own_corpus_from_a_manifest() {
        let dir = std::env::temp_dir().join("massread_manifest_selfread");
        let _ = std::fs::create_dir_all(&dir);
        let a = dir.join("alpha.rs");
        let b = dir.join("beta.rs");
        std::fs::write(&a, "pub fn alpha() {}\n").expect("write a");
        std::fs::write(&b, "pub fn beta() {}\n").expect("write b");
        let paths = vec![
            a.display().to_string(),
            b.display().to_string(),
            dir.join("missing.rs").display().to_string(),
        ];
        let (corpus, unreadable) = corpus_from_manifest(&paths);
        assert_eq!(unreadable.len(), 1, "the absent path is REPORTED, never silently dropped");
        assert!(corpus.contains("pub fn alpha()") && corpus.contains("pub fn beta()"));
        assert!(manifest_gaps(&corpus, &paths[..2]).is_empty(), "self-read corpus covers its own manifest");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn cap_boundary_is_exact_not_off_by_one() {
        let s = "x".repeat(64);
        assert_eq!(cap_output(&s, 64), s, "exactly-at-cap must pass untouched");
        let over = "x".repeat(65);
        assert!(cap_output(&over, 64).contains("CAPPED 64 of 65"));
    }

    #[test]
    fn rate_limit_bounces_retry_and_real_failures_do_not() {
        assert!(is_rate_limit("Gaxios error: 429 Too Many Requests"));
        assert!(is_rate_limit("RESOURCE_EXHAUSTED: quota exceeded"));
        assert!(!is_rate_limit("timeout after 150s"));
        assert!(!is_rate_limit("ModelNotFoundError: Requested entity was not found."));
        assert!(backoff_secs(0) >= 4, "15 RPM floor is 4s between calls");
        assert!(backoff_secs(1) > backoff_secs(0), "backoff grows");
        assert_eq!(backoff_secs(99), backoff_secs(RETRY_ATTEMPTS - 1), "capped, never unbounded");
    }

    #[test]
    fn the_jittered_wait_spreads_inside_its_ceiling_and_never_hits_zero() {
        for a in 0..RETRY_ATTEMPTS {
            let (lo, hi) = (backoff_jittered(a, 0.0), backoff_jittered(a, 1.0));
            assert_eq!(lo, 1, "the floor is a full second, not an instant re-bounce");
            assert_eq!(hi, backoff_secs(a), "the fixed ladder is now the ceiling");
            assert!(backoff_jittered(a, 0.5) > lo || backoff_secs(a) == 1, "mid draws land between");
        }
        assert_eq!(backoff_jittered(0, 9.9), backoff_secs(0), "an out-of-range roll clamps");
        assert_eq!(backoff_jittered(0, -1.0), 1, "and clamps low");
    }

    #[test]
    fn the_slot_pool_counts_live_peers_and_never_charges_a_slot_to_residue() {
        assert!(slot_open(&[]), "an empty pool is open");
        let full: Vec<u64> = vec![0; MAX_CONCURRENT_CALLS];
        assert!(!slot_open(&full), "at the cap the next lane waits");
        assert_eq!(live_lanes(&full), MAX_CONCURRENT_CALLS);
        let stale = HEARTBEAT_SECS * 3 + 1;
        assert_eq!(live_lanes(&[stale; 9]), 0, "stalled beats hold nothing");
        assert!(slot_open(&[stale, stale, 0, 0]), "residue frees the slot it is not using");
        assert!(!is_stalled(SLOT_WAIT_SECS), "the wait ends exactly AT the stall horizon");
        assert!(is_stalled(SLOT_WAIT_SECS + 1), "past it every held slot is freed anyway");
        assert!(SLOT_POLL_SECS < backoff_secs(0), "a poll is cheaper than a retry");
    }

    #[test]
    fn run_capacity_takes_whichever_wall_comes_first() {
        let std_yield = run_capacity(STANDARD_FILE_TOKENS);
        assert!((600..=700).contains(&std_yield), "standard code yields ~600-700, got {std_yield}");
        assert_eq!(run_capacity(SMALL_FILE_TOKENS), API_FILE_CEILING, "small files hit the file wall");
        assert_eq!(run_capacity(0), API_FILE_CEILING, "no div-by-zero");
    }

    #[test]
    fn batch_shape_refuses_past_the_api_walls_and_flags_a_wasted_run() {
        let one = "=== a.rs\nfn a() {}\n";
        assert_eq!(batch_shape(one), BatchShape::Underfilled(1));
        let sweep = "=== f.rs\nbody\n".repeat(MODULE_SWEEP_FLOOR);
        assert_eq!(batch_shape(&sweep), BatchShape::Full(MODULE_SWEEP_FLOOR));
        let too_many = "=== f.rs\nbody\n".repeat(API_FILE_CEILING + 1);
        assert!(matches!(batch_shape(&too_many), BatchShape::OverCeiling(_)), "file ceiling");
        let too_fat = format!("=== f.rs\n{}", "x".repeat((CONTEXT_WINDOW_TOKENS + 1) * 4));
        assert!(matches!(batch_shape(&too_fat), BatchShape::OverCeiling(_)), "context window");
    }

    #[test]
    fn fan_out_smell_names_one_file_per_call_and_leaves_real_sweeps_alone() {
        assert!(fan_out_smell(&[1, 1, 1]).is_some(), "3 calls, 3 files = the anti-pattern");
        assert!(fan_out_smell(&[1, 1]).is_none(), "two receipts is not a wave");
        assert!(fan_out_smell(&[1, 1, 18]).is_none(), "one real sweep clears the batch");
        assert!(fan_out_smell(&[]).is_none());
    }

    #[test]
    fn route_ladder_escalates_past_either_lite_ceiling() {
        let small = "=== a.rs\nfn a() {}\n";
        assert_eq!(route_ladder(small)[0], HAIKU_MODEL, "bounded rung leads");
        assert_eq!(route_ladder(small).len(), 2, "small batches keep a fallback");
        let fat_rung = vec![FLASH_MODEL, HAIKU_MODEL];
        let fat = "x".repeat((LITE_TOKEN_CEILING + 1) * 4);
        assert_eq!(route_ladder(&fat), fat_rung, "token ceiling");
        let many = "=== f.rs\nbody\n".repeat(LITE_ITEM_CEILING + 1);
        assert_eq!(route_ladder(&many), fat_rung, "item ceiling");
        assert_eq!(count_items(&many), LITE_ITEM_CEILING + 1);
    }

    #[test]
    fn compose_prompt_splices_rules_and_keeps_scratchpad_before_verdict() {
        let p = compose_prompt("TAG the rows");
        assert!(p.contains("<rules>\nTAG the rows\n</rules>"));
        assert!(!p.contains("{RULES}"));
        let (sp, vd) = (p.find("\"scratchpad\"").unwrap(), p.find("\"verdict\"").unwrap());
        assert!(sp < vd, "scratchpad must be emitted before verdict");
    }

    #[test]
    fn a_corpus_that_misses_a_routed_path_is_a_gap_not_a_verdict() {
        let corpus = "=== crates/a/src/lib.rs\nfn a() {}\n=== crates/b/src/lib.rs\nfn b() {}\n";
        let all = vec!["crates/a/src/lib.rs".to_string(), "crates/b/src/lib.rs".to_string()];
        assert!(manifest_gaps(corpus, &all).is_empty());
        assert!(manifest_gaps(corpus, &["crates\\a\\src\\lib.rs".into()]).is_empty(), "separator-blind");
        assert_eq!(
            manifest_gaps(corpus, &["crates/a/src/lib.rs".into(), "crates/c/src/lib.rs".into()]),
            vec!["crates/c/src/lib.rs".to_string()]
        );
        assert!(manifest_gaps(corpus, &["F:/NewRepo/crates/a/src/lib.rs".into()]).is_empty());
        assert!(manifest_gaps("", &["x.rs".into()]).len() == 1, "an empty corpus covers nothing");
        assert!(manifest_gaps(corpus, &[]).is_empty(), "no manifest = no assertion");
    }

    #[test]
    fn a_silent_call_is_called_stalled_only_after_three_missed_beats() {
        assert!(!is_stalled(0), "a fresh call is not a stall");
        assert!(!is_stalled(HEARTBEAT_SECS), "one beat is the normal cadence");
        assert!(!is_stalled(HEARTBEAT_SECS * 3), "exactly three beats is still jitter");
        assert!(is_stalled(HEARTBEAT_SECS * 3 + 1), "past three missed beats it is a stall");
        assert!(MAX_TIMEOUT_SECS / HEARTBEAT_SECS <= 30, "a 900s read leaves <=30 beats");
        assert!(HEARTBEAT_SECS < MIN_TIMEOUT_SECS, "at least one beat before the floor elapses");
    }

    #[test]
    fn a_beat_past_the_ceiling_is_abandoned_residue_not_a_wedged_call() {
        assert!(!is_abandoned(0), "a fresh beat is nobody's residue");
        assert!(is_stalled(HEARTBEAT_SECS * 3 + 1), "a stall is still a stall inside the ceiling");
        assert!(!is_abandoned(MAX_TIMEOUT_SECS), "at the ceiling the writer may still be closing");
        assert!(is_abandoned(MAX_TIMEOUT_SECS + 1), "past it the writer cannot exist");
        assert!(is_stalled(MAX_TIMEOUT_SECS + 1), "ordering is load-bearing, not cosmetic");
    }

    #[test]
    fn a_reachability_question_without_a_dispatch_file_is_refused_before_a_call_is_spent() {
        let module = "=== crates/a/src/board_sync.rs\npub fn export_tasks() {}\n";
        let with_main = format!("{module}=== crates/a/src/main.rs\nfn main() {{}}\n");

        let q = "name every fn with no live caller";
        assert!(dispatch_gap(module, q).is_some(), "no entry point = cannot answer");
        assert!(dispatch_gap(&with_main, q).is_none(), "main.rs closes the gap");
        assert!(dispatch_gap(module, "which fns are orphan").is_some());
        assert!(dispatch_gap(module, "list every unwired symbol").is_some());

        assert!(dispatch_gap(module, "Classify this corpus: one line per item, terse.").is_none());

        let lib = format!("{module}=== crates/a/src/lib.rs\npub mod board_sync;\n");
        assert!(dispatch_gap(&lib, q).is_none(), "lib.rs is a dispatch file");
        let win = format!("{module}=== crates\\a\\src\\mod.rs\npub mod x;\n");
        assert!(dispatch_gap(&win, q).is_none(), "separator-blind");
        let decoy = format!("{module}=== crates/a/src/remain.rs\nfn r() {{}}\n");
        assert!(dispatch_gap(&decoy, q).is_some(), "remain.rs is not main.rs");
    }

    #[test]
    fn the_system_prompt_reserves_absent_for_a_provable_negative() {
        assert!(SYSTEM_PROMPT.contains("is UNKNOWN, never ABSENT"), "law 2 must ban bare ABSENT");
        assert!(!SYSTEM_PROMPT.contains("declare it ABSENT"), "the 08-02 instruction is gone");
        assert!(SYSTEM_PROMPT.contains("dispatch map"), "names what would make ABSENT provable");
    }

    #[test]
    fn est_tokens_and_count_items_are_pure() {
        let corpus = "=== a.rs\npub fn f() {}\n=== b.rs\nfn g() {}\n";
        assert_eq!(count_items(corpus), 2);
        assert!(est_tokens(corpus) > 0 && est_tokens(corpus) < corpus.len());
    }

    #[test]
    fn norm_ws_collapses_whitespace() {
        assert_eq!(norm_ws("  a   b  c  "), "a b c");
        assert_eq!(norm_ws("a"), "a");
        assert_eq!(norm_ws(""), "");
    }

    #[test]
    fn strip_fence_unwraps_code_blocks() {
        assert_eq!(strip_fence("```json\n{\"a\":1}\n```"), "{\"a\":1}");
        assert_eq!(strip_fence("  {\"a\":1}  "), "{\"a\":1}", "unfenced passes through");
        assert_eq!(strip_fence("```\ncode\n```"), "code");
    }

    #[test]
    fn timeout_for_scales_by_corpus_size() {
        let tiny = "=== a.rs\nfn a() {}\n";
        assert_eq!(timeout_for(tiny), MIN_TIMEOUT_SECS, "floor, not a guess");
        let fat = "x".repeat(19_469 * 4);
        assert!(timeout_for(&fat) > timeout_for(tiny), "size scales past the floor");
        let absurd = "x".repeat(CONTEXT_WINDOW_TOKENS * 4);
        assert_eq!(timeout_for(&absurd), MAX_TIMEOUT_SECS, "capped, never unbounded");
        for n in [0usize, 1, 100, 5_000, 60_000, 400_000] {
            assert!(timeout_for(&"x".repeat(n)) >= MIN_TIMEOUT_SECS, "n={n}");
        }
    }
}
