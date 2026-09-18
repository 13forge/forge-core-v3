//! The river-line grammar v1 — the corpus wire format and its ingest validator.
//!
//! ARCH000 2026-08-10: the corpus is ACTIVE — one grammar, four faces (DSL, CST,
//! AST, LSP). This module is the first face's floor: the line grammar and the
//! refusal-first parser that IS the ingest gate. A malformed line never enters
//! the river; there is no lenient mode.
//!
//! One line, four tab-separated fields, riverbed.idx's shape made strict:
//!
//! ```text
//! TERM \t path:line \t 16-hex MortonKey5D \t body
//! ```
//!
//! - `TERM` — riverbed vocabulary word, `[A-Z0-9_-]+` (ALLOC, ZPLANE, …).
//!   Terms may repeat: a term is a lane, not a primary key.
//! - `path:line` — the riverrock anchor. Every line points at real disk; a line
//!   that anchors nowhere is not corpus (raw source is never copied in, only
//!   anchored). `line` is a 1-based decimal, never 0.
//! - key — the line's 5D address, [`MortonKey5D`] as exactly 16 lower-hex
//!   digits; bits 60..64 must be zero (the axes are pinned: W=T, θ=S).
//! - `body` — the cremantics-condensed law prose. UTF-8, no tabs, no newlines,
//!   no control bytes, non-empty. Lossless: `render(parse(l)) == l`.

use crate::ramus_prime::MortonKey5D;

/// Why a line was refused. Every variant names the field and the defect —
/// these are the LSP diagnostics of the DSL face, so they are typed, not prose.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RiverRefusal {
    /// Not exactly four tab-separated fields.
    FieldCount,
    /// Term empty or containing a byte outside `[A-Z0-9_-]`.
    TermCharset,
    /// Anchor lacks the `path:line` shape, path empty, or line 0/non-decimal.
    AnchorShape,
    /// Key not exactly 16 lower-hex digits.
    KeyShape,
    /// Key decodes but bits 60..64 are set — not a 5D address.
    KeyRange,
    /// Body empty, or containing tabs, newlines, or other control bytes.
    BodyBytes,
}

/// One parsed river line. Owned, exact — rendering reproduces the input bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RiverLine {
    /// The vocabulary term (lane).
    pub term: String,
    /// The riverrock anchor path.
    pub path: String,
    /// The 1-based anchor line.
    pub line: u32,
    /// The 5D address.
    pub key: MortonKey5D,
    /// The condensed body.
    pub body: String,
}

fn term_ok(t: &str) -> bool {
    !t.is_empty()
        && t.bytes().all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_' || b == b'-')
}

fn body_ok(b: &str) -> bool {
    !b.is_empty() && !b.bytes().any(|c| c < 0x20 || c == 0x7F)
}

/// Parse one line or refuse it. The gate: `Err` never partially ingests.
pub fn parse_line(input: &str) -> Result<RiverLine, RiverRefusal> {
    let mut fields = input.split('\t');
    let (term, anchor, key_s, body) = match (fields.next(), fields.next(), fields.next(), fields.next(), fields.next())
    {
        (Some(a), Some(b), Some(c), Some(d), None) => (a, b, c, d),
        _ => return Err(RiverRefusal::FieldCount),
    };

    if !term_ok(term) {
        return Err(RiverRefusal::TermCharset);
    }

    // path:line — split on the LAST colon so Windows-style `F:\x` paths and
    // `crates/x.rs`-style both anchor; the line number is what follows it.
    let (path, line_s) = anchor.rsplit_once(':').ok_or(RiverRefusal::AnchorShape)?;
    if path.is_empty() || line_s.is_empty() || !line_s.bytes().all(|b| b.is_ascii_digit()) {
        return Err(RiverRefusal::AnchorShape);
    }
    let line: u32 = line_s.parse().map_err(|_| RiverRefusal::AnchorShape)?;
    if line == 0 {
        return Err(RiverRefusal::AnchorShape);
    }

    if key_s.len() != 16
        || !key_s.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(RiverRefusal::KeyShape);
    }
    let raw = u64::from_str_radix(key_s, 16).map_err(|_| RiverRefusal::KeyShape)?;
    if raw >> 60 != 0 {
        return Err(RiverRefusal::KeyRange);
    }

    if !body_ok(body) {
        return Err(RiverRefusal::BodyBytes);
    }

    Ok(RiverLine {
        term: term.to_owned(),
        path: path.to_owned(),
        line,
        key: MortonKey5D(raw),
        body: body.to_owned(),
    })
}

/// Render a line back to its wire form. `render(parse(l)) == l` for every
/// accepted `l` — the CST face's lossless guarantee starts here.
pub fn render_line(l: &RiverLine) -> String {
    format!("{}\t{}:{}\t{:016x}\t{}", l.term, l.path, l.line, l.key.0, l.body)
}

/// The result of flowing a corpus through the banks. Both halves are reported —
/// no line is ever silently dropped (the telemetry discipline of the substrate).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IngestReport {
    /// Lines the grammar accepted, in file order.
    pub accepted: Vec<RiverLine>,
    /// Refusals, each paired with its 1-based source line number and diagnostic.
    pub refused: Vec<(usize, RiverRefusal)>,
}

impl IngestReport {
    /// True when every non-blank, non-comment line was accepted.
    pub fn all_accepted(&self) -> bool {
        self.refused.is_empty()
    }
}

/// Flow a multi-line corpus through the banks. Blank lines and `#` comment/marker
/// lines are river breathing space and are skipped; every other line is parsed or
/// refused with its 1-based line number. One bad line never sinks its neighbours —
/// the batch discipline of `commit_many`, applied to the corpus.
pub fn ingest(text: &str) -> IngestReport {
    let mut accepted = Vec::new();
    let mut refused = Vec::new();
    for (i, raw) in text.lines().enumerate() {
        let line = raw.strip_suffix('\r').unwrap_or(raw);
        let trimmed = line.trim_start();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        match parse_line(line) {
            Ok(l) => accepted.push(l),
            Err(e) => refused.push((i + 1, e)),
        }
    }
    IngestReport { accepted, refused }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ramus_prime::AXIS_MASK;

    const GOOD: &str =
        "ZPLANE\tcrates/forge-canvas/src/compositor.rs:11\t0000000000000000\tCompositorLayer fixed 8-stack, z ascending, z=4 magic materialID plane.";

    #[test]
    fn a_good_line_parses_and_round_trips_losslessly() {
        let l = parse_line(GOOD).unwrap();
        assert_eq!(l.term, "ZPLANE");
        assert_eq!(l.path, "crates/forge-canvas/src/compositor.rs");
        assert_eq!(l.line, 11);
        assert_eq!(l.key, MortonKey5D(0));
        assert_eq!(render_line(&l), GOOD);
    }

    #[test]
    fn a_windows_path_anchor_splits_on_the_last_colon() {
        let s = "LAW\tF:\\v3\\CLAUDE.md:5\t0000000000000abc\tL01 law-is-test.";
        let l = parse_line(s).unwrap();
        assert_eq!(l.path, "F:\\v3\\CLAUDE.md");
        assert_eq!(l.line, 5);
        assert_eq!(render_line(&l), s);
    }

    #[test]
    fn every_encoded_key_survives_the_wire() {
        let mut s = 0xD1CE_D00D_FEED_5EEDu64;
        for _ in 0..1_000 {
            s = s.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
            let axes = [
                (s as u16) & AXIS_MASK,
                ((s >> 12) as u16) & AXIS_MASK,
                ((s >> 24) as u16) & AXIS_MASK,
                ((s >> 36) as u16) & AXIS_MASK,
                ((s >> 48) as u16) & AXIS_MASK,
            ];
            let key = MortonKey5D::encode(axes);
            let line = RiverLine {
                term: "MAP".into(),
                path: "a/b.rs".into(),
                line: 1,
                key,
                body: "x".into(),
            };
            assert_eq!(parse_line(&render_line(&line)).unwrap().key, key);
        }
    }

    // ---- the refusals, one per diagnostic --------------------------------------

    #[test]
    fn each_defect_is_refused_with_its_own_name() {
        let cases: &[(&str, RiverRefusal)] = &[
            ("ZPLANE\ta.rs:1\t0000000000000000", RiverRefusal::FieldCount),
            ("A\tB\tC\tD\tE", RiverRefusal::FieldCount),
            ("zplane\ta.rs:1\t0000000000000000\tx", RiverRefusal::TermCharset),
            ("\ta.rs:1\t0000000000000000\tx", RiverRefusal::TermCharset),
            ("Z PLANE\ta.rs:1\t0000000000000000\tx", RiverRefusal::TermCharset),
            ("ZPLANE\ta.rs\t0000000000000000\tx", RiverRefusal::AnchorShape),
            ("ZPLANE\t:1\t0000000000000000\tx", RiverRefusal::AnchorShape),
            ("ZPLANE\ta.rs:0\t0000000000000000\tx", RiverRefusal::AnchorShape),
            ("ZPLANE\ta.rs:x\t0000000000000000\tx", RiverRefusal::AnchorShape),
            ("ZPLANE\ta.rs:1\t00000000000000\tx", RiverRefusal::KeyShape),
            ("ZPLANE\ta.rs:1\t000000000000000G\tx", RiverRefusal::KeyShape),
            ("ZPLANE\ta.rs:1\t00000000000000AB\tx", RiverRefusal::KeyShape),
            ("ZPLANE\ta.rs:1\tf000000000000000\tx", RiverRefusal::KeyRange),
            ("ZPLANE\ta.rs:1\t0000000000000000\t", RiverRefusal::BodyBytes),
            ("ZPLANE\ta.rs:1\t0000000000000000\ta\u{7F}b", RiverRefusal::BodyBytes),
        ];
        for (input, want) in cases {
            assert_eq!(parse_line(input), Err(*want), "input: {input:?}");
        }
    }

    // Bits 60..64 are the axis pin's enforcement point on the wire: a key that
    // uses them was not produced by encode() and is refused before decode.
    #[test]
    fn the_wire_refuses_what_encode_cannot_produce() {
        for hi in 1u64..=15 {
            let raw = hi << 60;
            let s = format!("MAP\ta.rs:1\t{raw:016x}\tx");
            assert_eq!(parse_line(&s), Err(RiverRefusal::KeyRange));
        }
    }

    // ---- ingest: the water flows, and one bad line does not sink the rest -------

    #[test]
    fn ingest_skips_blank_and_comment_lines_and_accepts_the_rest() {
        let text = "# header marker\n\nZPLANE\ta.rs:1\t0000000000000000\tone\n   \nATOM\tb.rs:2\t0000000000000001\ttwo\n";
        let report = ingest(text);
        assert!(report.all_accepted());
        assert_eq!(report.accepted.len(), 2);
        assert_eq!(report.accepted[0].term, "ZPLANE");
        assert_eq!(report.accepted[1].term, "ATOM");
    }

    #[test]
    fn a_bad_line_is_refused_by_its_number_while_neighbours_flow() {
        // line 1 good, line 2 bad key, line 3 good.
        let text = "A\ta.rs:1\t0000000000000000\tone\nB\tb.rs:2\tGGGG\ttwo\nC\tc.rs:3\t0000000000000002\tthree\n";
        let report = ingest(text);
        assert_eq!(report.accepted.len(), 2, "the two good lines still flow");
        assert_eq!(report.refused, vec![(2, RiverRefusal::KeyShape)], "line 2 named and refused");
    }

    // OPEN THE WATER (ARCH000 2026-08-10): the on-disk headwater is the river's
    // first water, and it must flow clean through its own grammar. This test binds
    // the file to the banks — a stray space where a tab belongs fails here.
    #[test]
    fn the_headwater_flows_clean() {
        const HEADWATER: &str = include_str!("../river/headwater.river");
        let report = ingest(HEADWATER);
        assert!(report.all_accepted(), "headwater refused: {:?}", report.refused);
        assert_eq!(report.accepted.len(), 14, "fourteen lines of first water");
        for l in &report.accepted {
            // Headwater sits at the origin cell until the conductor addresses it in 5D.
            assert_eq!(l.key, MortonKey5D(0), "{} is unplaced headwater", l.term);
            // Every line is real riverrock — anchored to a disk path.
            assert!(l.path.contains('/'), "{} must anchor to a path", l.term);
            assert!(!l.body.is_empty());
        }
        // The first water names the banks: the substrate describes itself.
        let terms: Vec<&str> = report.accepted.iter().map(|l| l.term.as_str()).collect();
        for expected in [
            "ATOM", "SENTINEL", "MERSENNE", "RAMUS", "AXISPIN", "RIVER", "ATTENTION", "TRANSDUCE", "WARDEN", "TRIPLEBUF",
        ] {
            assert!(terms.contains(&expected), "headwater missing {expected}");
        }
    }
}
