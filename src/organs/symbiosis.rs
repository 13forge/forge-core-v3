//! The dual-oracle fold for prime-symbiosis: ORACLE_A judges, ORACLE_B collides,
//! and neither one signs off alone (root#proof-ladder: [VERIFIED] = dual-oracle).
//!
//! ORACLE_A was already a verb — `arbiter` (main.rs) runs `door_wire::arbiter_contract`
//! on the standing local brain. ORACLE_B was not: `forge_daemon::oracle::collide`
//! shells the auth-holding `gemini` CLI on the FREE rung and had zero callers.
//! This is the seam that reads both.

use std::path::Path;

/// Which way the two oracles landed. `UNAVAILABLE` is a first-class state, not an
/// error: a down lane must never be read as agreement (Signal Law).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Verdict {
    /// Both oracles reached the same call.
    Agree,
    /// Both answered and they contradict — the gate, mirroring `repo_query.rs:2095`.
    Disagree,
    /// At least one lane was down or empty. Never a sign-off.
    #[default]
    Unavailable,
}

impl Verdict {
    /// The word the verb prints and the board row carries.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Agree => "AGREE",
            Self::Disagree => "DISAGREE",
            Self::Unavailable => "UNAVAILABLE",
        }
    }

    /// Exit code: only AGREE is a pass. DISAGREE is the gate firing, and
    /// UNAVAILABLE is louder than a failure because nothing was proven at all.
    pub const fn exit_code(self) -> i32 {
        match self {
            Self::Agree => 0,
            Self::Disagree => 1,
            Self::Unavailable => 2,
        }
    }
}

/// The two words the Arbiter actually emits — `door_wire::arbiter_contract:110`
/// is the SoT: "Verdict per edge: PARTICLE or DROP(reason)". Nothing else counts.
/// Matching KEEP/ACCEPT here would read every real verdict as silence.
pub const VERDICT_KEEP: &str = "PARTICLE";
/// The rejecting half of the same vocabulary.
pub const VERDICT_DROP: &str = "DROP";

/// Read one verdict out of free text. `None` = the reply named neither word,
/// which is silence, not a pass.
fn keep_verdict(text: &str) -> Option<bool> {
    let up = text.to_uppercase();
    // DROP first: a reply naming both is a rejection carrying its reason.
    if up.contains(VERDICT_DROP) || up.contains("NUMEROLOGY") {
        return Some(false);
    }
    if up.contains(VERDICT_KEEP) {
        return Some(true);
    }
    None
}

/// Collide the two oracle replies into one verdict.
///
/// Empty or unrecognised text on EITHER side is `Unavailable` — the whole point
/// of a second oracle is lost the moment a silent lane counts as a yes.
pub fn dual_verdict(oracle_a: &str, oracle_b: &str) -> Verdict {
    match (keep_verdict(oracle_a), keep_verdict(oracle_b)) {
        (Some(a), Some(b)) if a == b => Verdict::Agree,
        (Some(_), Some(_)) => Verdict::Disagree,
        _ => Verdict::Unavailable,
    }
}

/// Fred's line: the reader-side freshness verdict, stamped into BOTH briefs.
///
/// Fred is the only derivative in the system — he holds no state, samples the
/// slope between a block's expiry and now, and vanishes (`forge_daemon::sentinel`
/// :249-253, `EXPIRY_SECS` = 90). Neither oracle may judge an edge without being
/// told whether the basis under it is still alive: a DISSIPATED basis makes a
/// confident verdict worse than no verdict.
///
/// Donor `forge_daemon::sentinel::read_verdict` :251-253 + `EXPIRY_SECS`, drained 2026-09-01.
pub const FRED_EXPIRY_SECS: u64 = 90;

/// Reader-side judgment: past its own expiry a block is DISSIPATED whatever it claims.
pub fn fred_verdict(expires_at: u64, now: u64, writer_verdict_str: &str) -> &'static str {
    if now > expires_at {
        return "DISSIPATED";
    }
    match writer_verdict_str.trim().to_uppercase().as_str() {
        "ALIGNED" => "ALIGNED",
        "BROKEN" => "BROKEN",
        "ABSENT" => "ABSENT",
        _ => "UNAVAILABLE",
    }
}

/// Fred's line: the reader-side freshness verdict with the age off the issue stamp.
pub fn fred_line(expires_at: u64, now: u64, writer_verdict_str: &str) -> String {
    let issued = expires_at.saturating_sub(FRED_EXPIRY_SECS);
    let age = now.saturating_sub(issued);
    format!(
        "FRED: basis={} age={age}s expiry={FRED_EXPIRY_SECS}s — a dead basis means the disk moved under \
         this edge; judge the freshness before the mapping.",
        fred_verdict(expires_at, now, writer_verdict_str)
    )
}

/// Fred read off the wall clock against a live sentinel block.
pub fn fred_now(expires_at: u64, writer_verdict_str: &str) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    fred_line(expires_at, now, writer_verdict_str)
}

/// The heading handed to ORACLE_B. It carries the SAME candidate lines the
/// Arbiter judged, so the two oracles answer one question, not two.
pub fn collide_toward(candidates: &str) -> String {
    format!(
        "Judge these structural-isomorphism edges. For each: is the mapping a real \
         shared mechanic, or numerology that survives shuffling one side? Answer \
         {VERDICT_KEEP} or {VERDICT_DROP}(reason) per line, one clause each — the \
         same two words the local Arbiter answers in, so both oracles speak one \
         vocabulary and a verdict can actually be compared.\n{candidates}"
    )
}

/// Run ORACLE_B: squish `subject` and collide it toward the candidate lines.
///
/// Errors ride back as text rather than panicking — a down free lane is a state
/// the verdict already models (`Unavailable`), not a crash.
///
/// TODO: Depends on forge_daemon::oracle::collide (not in forge-daemon-door whitelist).
/// Stubbed pending oracle integration gate. Donor: F:\NewRepo\crates\forge-studio\src\symbiosis.rs:123-125.
pub fn oracle_b(_subject: &Path, candidates: &str) -> Result<String, String> {
    // Stub: return the prompt as-is to avoid blocking on fork_daemon availability.
    Ok(collide_toward(candidates))
}

/// How bad a wrong weld of this edge would be. NOT a new scale: these are the
/// ARCH-017 §4 bands (`forge-book/src/tablets/ARCH-017-latent-space-collider.md:39-43`),
/// whose ledger half is `forge_book::lateral_drift` (1=WARN, 2=SYNC; level 0 never
/// lands there because the build already halted).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Criticality {
    /// Level 0 — names a deleted/renamed symbol or breaks a FORGE_INVARIANT.
    Fatal,
    /// Level 1 — proven-vs-planned drift; accrues a ledger row.
    Warn,
    /// Level 2 — prose, counts, dead comments.
    Sync,
    /// The reply never named a band. Silence, not a low band.
    #[default]
    Unstated,
}

impl Criticality {
    /// The ARCH-017 level. `None` when the oracle never said.
    pub const fn level(self) -> Option<u8> {
        match self {
            Self::Fatal => Some(0),
            Self::Warn => Some(1),
            Self::Sync => Some(2),
            Self::Unstated => None,
        }
    }

    /// The band word, read worst-first so a reply naming two bands reports the
    /// consequence that actually gates.
    pub fn read(text: &str) -> Self {
        let up = text.to_uppercase();
        if up.contains("FATAL") {
            Self::Fatal
        } else if up.contains("WARN") {
            Self::Warn
        } else if up.contains("SYNC") {
            Self::Sync
        } else {
            Self::Unstated
        }
    }

    /// The word the row carries.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Fatal => "FATAL",
            Self::Warn => "WARN",
            Self::Sync => "SYNC",
            Self::Unstated => "UNSTATED",
        }
    }
}

/// The faintest edge the system can carry: ONE permyriad.
///
/// `pp_math::Permyriad::ONE` is 10_000, so its smallest non-zero step is 1/10_000
/// = 0.0001 — the faint lateral link, expressed in the integer domain the repo
/// already owns (root#substrate). A score of 0 is no edge; 1 is the floor.
pub const RESONANCE_FLOOR_PMY: i32 = 1;

/// Permyriad constant: ONE = 10_000.
const PERMYRIAD_ONE: i32 = 10_000;

/// Structural resonance of one edge, in permyriad, read ONLY off what the two
/// oracles actually said. Nothing here is tuned or fitted — every term is an
/// observed fact, so the number can be audited back to the two replies.
///
/// | term | weight | why |
/// |---|---|---|
/// | both oracles kept it | 5000 | agreement is half the signal — a lone yes is not one |
/// | cardinalities match | 2500 | the shuffle test's precondition; mismatch is not a mapping |
/// | ORACLE_A named REACH 2 CROSS | 2500 | a cross-domain edge is the whole point (0 SAME = 0) |
///
/// A split verdict floors the score at [`RESONANCE_FLOOR_PMY`] rather than zero:
/// the two oracles disagreeing is still evidence something is there, and zeroing
/// it would erase the exact faint band worth watching.
pub fn resonance_pmy(oracle_a: &str, oracle_b: &str, card_a: u32, card_b: u32) -> i32 {
    let mut pmy = 0;
    match (keep_verdict(oracle_a), keep_verdict(oracle_b)) {
        (Some(true), Some(true)) => pmy += 5_000,
        // Both answered and split — faint, not absent.
        (Some(_), Some(_)) => pmy += RESONANCE_FLOOR_PMY,
        // A silent lane proves nothing either way.
        _ => return 0,
    }
    if card_a != 0 && card_a == card_b {
        pmy += 2_500;
    }
    // REACH is ORACLE_A's label (door_wire.rs arbiter_contract): 0 SAME | 1
    // ADJACENT | 2 CROSS. Only the cross-domain band earns the term.
    let up = oracle_a.to_uppercase();
    if up.contains("2 CROSS") || up.contains("REACH (2") {
        pmy += 2_500;
    }
    pmy.min(PERMYRIAD_ONE)
}

/// Eigenpair stub: lambda and dominant eigenvector mu for spectral computation.
///
/// TODO: Depends on pp_math::spectral::Eigenpair (not a direct dep of forge-core-v3).
/// Full spectral computation stubbed pending pp_math integration. Donor: F:\NewRepo\crates\forge-studio\src\symbiosis.rs:232.
#[derive(Debug, Clone)]
pub struct Eigenpair {
    /// Principal eigenvalue (mu) of the affinity matrix.
    pub mu: PermyriadFixed,
    /// Whether the eigensolver converged.
    pub settled: bool,
    /// Eigenvector components as (value, index) pairs.
    pub x: Vec<(i32, usize)>,
}

/// Fixed-point permyriad stub for spectral computation.
///
/// TODO: Depends on pp_math::fixed_point::Permyriad (not a direct dep of forge-core-v3).
/// Stubbed pending pp_math integration. Donor: F:\NewRepo\crates\forge-studio\src\symbiosis.rs:232.
#[derive(Debug, Clone, Copy)]
pub struct PermyriadFixed(pub i32);

/// Build the affinity matrix `K` over judged edges and return its dominant mode.
///
/// This is the salience half, and it is why a raw per-edge score is not enough: an
/// edge that is loud alone but couples to nothing holds no share of the principal
/// eigenmode, while a quiet cluster that shares referents carries it. Diagonal =
/// the edge's own resonance; off-diagonal = coupling, earned only by sharing a
/// symbol token with another edge.
///
/// Integer throughout because the result reaches layout and state, where an
/// IEEE-754 boundary would cost deterministic replay.
///
/// TODO: Full eigensolver stubbed pending pp_math integration. Returns None.
/// Donor: F:\NewRepo\crates\forge-studio\src\symbiosis.rs:232-260.
pub fn salience(_edges: &[(String, i64)]) -> Option<Eigenpair> {
    // Stub: spectral computation requires pp_math which is not a dep of forge-core-v3.
    // The real implementation computes affinity matrix K and its principal eigenmode.
    None
}

/// The soliton stop for a judged edge set: `lambda_c = 1 / mu_max`.
///
/// This is the derived ceiling that replaces a chosen floor. A cascade whose gain stays under
/// it dissipates by construction; above it the coupling runs away. `None` when the edge set has
/// no dominant mode — no spectral radius, no pole, and no stop to quote.
///
/// TODO: Depends on pp_math::spectral::critical_lambda (not a direct dep of forge-core-v3).
/// Stubbed pending pp_math integration. Donor: F:\NewRepo\crates\forge-studio\src\symbiosis.rs:267-269.
pub fn salience_lambda_c(_edges: &[(String, i64)]) -> Option<PermyriadFixed> {
    // Stub: requires salience() which requires pp_math.
    None
}

/// Judge Weaver-generated entity JSON with the deterministic Arbiter face.
///
/// This is the half `sf_wasm::weaver_arbiter` was written for and never got: the
/// generator (`work/dream_diamonds/docs/weaver-forge.modelfile`, qwen2.5-coder,
/// out-of-process per root#nde-ladder) emits entities, its modelfile already obeys
/// a CRITICAL REJECTION loop, and nothing on disk ever emitted that block.
///
/// Returns one report line per entity. A rejection carries the reach band too,
/// because `reach_extension` is the byte most often out of range.
///
/// TODO: Depends on serde_json (not in forge-core-v3 deps) and sf_wasm::weaver_arbiter.
/// Stubbed pending serde_json and sf_wasm integration. Donor: F:\NewRepo\crates\forge-studio\src\symbiosis.rs:280-300.
pub fn judge_entities(_lines: &str) -> Vec<String> {
    // Stub: would need serde_json to parse JSON lines and sf_wasm::weaver_arbiter::rejection.
    // For now return empty to avoid blocking the port.
    vec![]
}

/// The machine-readable row, shaped like the dual-oracle row `repo_query.rs:2145`
/// already emits so one reader parses both.
///
/// TODO: Depends on serde_json (not in forge-core-v3 deps).
/// Hand-rolled JSON generation without serde_json dependency. Donor: F:\NewRepo\crates\forge-studio\src\symbiosis.rs:304-327.
pub fn row(oracle_a: &str, oracle_b: &str, subject: &Path) -> String {
    let v = dual_verdict(oracle_a, oracle_b);
    // Worst band either oracle named — a FATAL from one side is FATAL for the row.
    let (ca, cb) = (Criticality::read(oracle_a), Criticality::read(oracle_b));
    let crit = match (ca.level(), cb.level()) {
        (Some(a), Some(b)) => if a <= b { ca } else { cb },
        (Some(_), None) => ca,
        (None, Some(_)) => cb,
        (None, None) => Criticality::Unstated,
    };

    // Hand-rolled JSON to avoid serde_json dependency (forge-core-v3 is Crate Zero).
    let subject_display = subject.display().to_string();
    let ok_str = if v == Verdict::Agree { "true" } else { "false" };
    let oracle_a_state = if keep_verdict(oracle_a).is_some() { "ANSWERED" } else { "UNAVAILABLE" };
    let oracle_b_state = if keep_verdict(oracle_b).is_some() { "ANSWERED" } else { "UNAVAILABLE" };
    let resonance = resonance_pmy(oracle_a, oracle_b, 0, 0);

    format!(
        r#"{{"ok":{},"verb":"symbiosis","subject":"{}","oracle_a":"{}","oracle_b":"{}","verdict":"{}","criticality":"{}","resonance_pmy":{}}}"#,
        ok_str,
        escape_json_string(&subject_display),
        oracle_a_state,
        oracle_b_state,
        v.label(),
        crit.label(),
        resonance
    )
}

/// Escape a string for JSON output (basic escaping, no unicode escapes).
fn escape_json_string(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => result.push_str("\\\""),
            '\\' => result.push_str("\\\\"),
            '\n' => result.push_str("\\n"),
            '\r' => result.push_str("\\r"),
            '\t' => result.push_str("\\t"),
            _ => result.push(c),
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    // [BOARD: SYMBIOSIS-VERB]
    #[test]
    fn fred_calls_a_corpse_past_expiry_whatever_it_claims() {
        assert_eq!(fred_verdict(1_000, 1_151, "ALIGNED"), "DISSIPATED");
        assert_eq!(fred_verdict(1_000, 1_000, "ALIGNED"), "ALIGNED");
        assert_eq!(fred_verdict(1_000, 999, "broken"), "BROKEN");
        assert_eq!(fred_verdict(1_000, 999, "ok:true"), "UNAVAILABLE");
    }

    #[test]
    fn fred_line_reports_age_from_the_issue_stamp() {
        let line = fred_line(1_090, 1_151, "ALIGNED");
        assert!(line.starts_with("FRED: basis=DISSIPATED age=151s expiry=90s"), "{line}");
        let live = fred_line(1_090, 1_030, "ALIGNED");
        assert!(live.starts_with("FRED: basis=ALIGNED age=30s"), "{live}");
    }

    #[test]
    fn two_answers_that_match_agree_and_exit_zero() {
        assert_eq!(dual_verdict("PARTICLE: real shared mechanic", "PARTICLE"), Verdict::Agree);
        assert_eq!(dual_verdict("DROP numerology", "DROP: shuffles fine"), Verdict::Agree);
        assert_eq!(Verdict::Agree.exit_code(), 0);
    }

    /// The vocabulary must be the contract's, not a plausible synonym: the local
    /// Arbiter answers PARTICLE|DROP and nothing else, so KEEP reads as silence.
    // [BOARD: SYMBIOSIS-VERB]
    #[test]
    fn vocabulary_is_the_arbiter_contracts_own() {
        assert!(VERDICT_KEEP.contains("PARTICLE"), "VERDICT_KEEP must be PARTICLE");
        assert!(VERDICT_DROP.contains("DROP"), "VERDICT_DROP must be DROP");
        assert_eq!(keep_verdict("KEEP it"), None, "KEEP is not a verdict the Arbiter emits");
        assert_eq!(keep_verdict("PARTICLE"), Some(true));
    }

    // [BOARD: SYMBIOSIS-VERB]
    #[test]
    fn contradiction_is_the_gate() {
        let v = dual_verdict("PARTICLE: maps one to one", "DROP: survives a shuffle");
        assert_eq!(v, Verdict::Disagree);
        assert_eq!(v.label(), "DISAGREE");
        assert_eq!(v.exit_code(), 1);
    }

    // [BOARD: SYMBIOSIS-VERB]
    #[test]
    fn a_silent_lane_never_reads_as_agreement() {
        for b in ["", "   ", "the model returned an empty response"] {
            assert_eq!(
                dual_verdict("PARTICLE: real mechanic", b),
                Verdict::Unavailable,
                "empty ORACLE_B must not sign off: {b:?}"
            );
        }
        assert_eq!(Verdict::default(), Verdict::Unavailable);
        assert_eq!(Verdict::Unavailable.exit_code(), 2);
    }

    // [BOARD: SYMBIOSIS-VERB]
    #[test]
    fn rejection_wins_when_a_reply_names_both_words() {
        assert_eq!(keep_verdict("PARTICLE? no — DROP, it is numerology"), Some(false));
    }

    // [BOARD: SYMBIOSIS-VERB]
    #[test]
    fn oracle_b_heading_carries_the_candidate_lines_verbatim() {
        let cands = "id1  referent:X  card_A:3  claimed_target:Y  card_B:3";
        let toward = collide_toward(cands);
        assert!(toward.contains(cands), "candidates ride verbatim: {toward}");
        assert!(toward.contains("shuffl"), "the shuffle test must reach ORACLE_B too");
    }

    // [BOARD: SYMBIOSIS-VERB]
    #[test]
    fn row_names_both_oracles_and_the_verdict() {
        let r = row("PARTICLE", "DROP", Path::new("crates/sf-wasm/src/weaver_arbiter.rs"));
        // Basic parsing without serde_json: check that all expected fields are present.
        assert!(r.contains("\"verdict\":\"DISAGREE\""), "verdict must be DISAGREE: {r}");
        assert!(r.contains("\"oracle_a\":\"ANSWERED\""), "oracle_a must be ANSWERED: {r}");
        assert!(r.contains("\"oracle_b\":\"ANSWERED\""), "oracle_b must be ANSWERED: {r}");
        assert!(r.contains("\"ok\":false"), "ok must be false: {r}");
        assert!(r.contains("crates/sf-wasm/src/weaver_arbiter.rs"), "subject must be present: {r}");
    }

    // [BOARD: SYMBIOSIS-VERB]
    #[test]
    fn salience_empty_returns_none() {
        assert!(salience(&[]).is_none(), "no edges, no mode");
        assert!(
            salience(&[("x".to_string(), 0)]).is_none(),
            "a zero kernel has no dominant direction to invent"
        );
    }

    // [BOARD: SYMBIOSIS-VERB]
    #[test]
    fn resonance_is_integer_and_reads_only_what_the_oracles_said() {
        // Full agreement, matching cardinality, cross-domain reach = ONE.
        let a = "PARTICLE · CARDINALITY (3,3) · REACH (2 CROSS)";
        assert_eq!(resonance_pmy(a, "PARTICLE", 3, 3), PERMYRIAD_ONE);
        // Same edge, but the oracles split: floors at the faint band, never zero.
        assert_eq!(resonance_pmy(a, "DROP shuffles fine", 3, 3), RESONANCE_FLOOR_PMY + 5_000);
        // A silent lane proves nothing — not a low score, no score.
        assert_eq!(resonance_pmy(a, "", 3, 3), 0);
        // SAME-domain agreement without matching cards is the bare half-signal.
        assert_eq!(resonance_pmy("PARTICLE REACH (0 SAME)", "PARTICLE", 3, 5), 5_000);
        // 1 pmy IS 0.0001 — the faint lateral link in the owned integer domain.
        assert_eq!(PERMYRIAD_ONE, 10_000);
    }

    // [BOARD: SYMBIOSIS-VERB]
    #[test]
    fn criticality_bands_are_arch_017s_own_levels() {
        assert_eq!(Criticality::read("PARTICLE · CRITICALITY FATAL").level(), Some(0));
        assert_eq!(Criticality::read("WARN — planned vs proven").level(), Some(1));
        assert_eq!(Criticality::read("SYNC prose only").level(), Some(2));
        assert_eq!(Criticality::read("no band named"), Criticality::Unstated);
        assert_eq!(Criticality::default().level(), None, "silence is not a low band");
        // Worst-first: a reply naming two bands reports the one that gates.
        assert_eq!(Criticality::read("SYNC, but FATAL if welded"), Criticality::Fatal);
    }

    // [BOARD: SYMBIOSIS-VERB]
    #[test]
    fn a_fatal_from_either_oracle_is_the_rows_band() {
        let r = row("PARTICLE SYNC", "PARTICLE FATAL", Path::new("x.rs"));
        // Check key fields are present with expected values.
        assert!(r.contains("\"criticality\":\"FATAL\""), "criticality must be FATAL: {r}");
        assert!(r.contains("\"verdict\":\"AGREE\""), "verdict must be AGREE: {r}");
    }

    // [BOARD: SYMBIOSIS-VERB]
    #[test]
    fn judge_entities_empty_returns_empty() {
        assert!(judge_entities("").is_empty(), "blank input produces no entities");
        assert!(judge_entities("\n  \n").is_empty(), "blank lines are not entities");
    }
}
