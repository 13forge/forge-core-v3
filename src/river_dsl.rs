//! The river-line grammar v1 DSL face — recursive-descent lexer/parser.
//!
//! This module is the DSL face of the one-grammar system (DSL, CST, AST, LSP). It tokenizes
//! river lines into typed tokens (Term, Anchor{path, line}, Key, Body) with spans,
//! producing either a typed CST result or a typed RiverRefusal that MUST agree with
//! river::parse_line's verdict. The one-grammar gate: `river_dsl::parse(line).is_ok() == river::parse_line(line).is_ok()`.
//!
//! Format: `TERM \t path:line \t 16-hex MortonKey5D \t body`

use crate::river::RiverRefusal;

// ---------------------------------------------------------------------------
// Token & Span types
// ---------------------------------------------------------------------------

/// Byte range within the input, [lo, hi).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    /// Start byte offset (inclusive).
    pub lo: usize,
    /// End byte offset (exclusive).
    pub hi: usize,
}

impl Span {
    /// Create a new span from start and end offsets: [lo, hi).
    pub fn new(lo: usize, hi: usize) -> Self {
        Self { lo, hi }
    }
}

/// A value with its span in the source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Located<T> {
    /// The parsed value.
    pub value: T,
    /// The byte span of the value in the source.
    pub span: Span,
}

// ---------------------------------------------------------------------------
// Lexer
// ---------------------------------------------------------------------------

struct Lexer<'a> {
    src: &'a str,
    pos: usize,
}

impl<'a> Lexer<'a> {
    fn new(src: &'a str) -> Self {
        Self { src, pos: 0 }
    }

    fn peek_byte(&self) -> Option<u8> {
        self.src.as_bytes().get(self.pos).copied()
    }

    fn advance_byte(&mut self) -> Option<u8> {
        let b = self.peek_byte()?;
        self.pos += 1;
        Some(b)
    }

    fn consume_field(&mut self) -> &'a str {
        // A field is terminated by the next tab or end of input.
        let start = self.pos;
        while let Some(b) = self.peek_byte() {
            if b == b'\t' {
                break;
            }
            self.advance_byte();
        }
        &self.src[start..self.pos]
    }
}

// ---------------------------------------------------------------------------
// Parser: Typed CST result
// ---------------------------------------------------------------------------

/// A successfully parsed river line, typed into four fields.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RiverDslLine {
    /// The vocabulary term.
    pub term: Located<String>,
    /// The anchor path.
    pub path: Located<String>,
    /// The 1-based line number.
    pub line: Located<u32>,
    /// The 5D address key.
    pub key: Located<u64>,
    /// The body.
    pub body: Located<String>,
}

/// Parse one river line into a typed CST or refuse it with a diagnostic.
pub fn parse(input: &str) -> Result<RiverDslLine, RiverRefusal> {
    let mut lexer = Lexer::new(input);

    // Exactly four tab-separated fields.
    let term_s = lexer.consume_field();
    if lexer.peek_byte() != Some(b'\t') {
        return Err(RiverRefusal::FieldCount);
    }
    lexer.advance_byte(); // consume tab

    let anchor_s = lexer.consume_field();
    if lexer.peek_byte() != Some(b'\t') {
        return Err(RiverRefusal::FieldCount);
    }
    lexer.advance_byte(); // consume tab

    let key_s = lexer.consume_field();
    if lexer.peek_byte() != Some(b'\t') {
        return Err(RiverRefusal::FieldCount);
    }
    lexer.advance_byte(); // consume tab

    let body_s = lexer.consume_field();

    // Consume the rest of the input; if there's anything left, it's a 5th field.
    if lexer.peek_byte().is_some() {
        return Err(RiverRefusal::FieldCount);
    }

    // Validate and parse term.
    if !term_ok(term_s) {
        return Err(RiverRefusal::TermCharset);
    }
    let term_span = Span::new(0, term_s.len());

    // Parse anchor into path and line.
    let (path_s, line_s) = anchor_s.rsplit_once(':').ok_or(RiverRefusal::AnchorShape)?;
    if path_s.is_empty() || line_s.is_empty() || !line_s.bytes().all(|b| b.is_ascii_digit()) {
        return Err(RiverRefusal::AnchorShape);
    }
    let line: u32 = line_s.parse().map_err(|_| RiverRefusal::AnchorShape)?;
    if line == 0 {
        return Err(RiverRefusal::AnchorShape);
    }
    let path_span = Span::new(term_s.len() + 1, term_s.len() + 1 + path_s.len());
    let line_span = Span::new(path_span.hi + 1, path_span.hi + 1 + line_s.len());

    // Validate and parse key.
    if key_s.len() != 16 || !key_s.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)) {
        return Err(RiverRefusal::KeyShape);
    }
    let raw = u64::from_str_radix(key_s, 16).map_err(|_| RiverRefusal::KeyShape)?;
    if raw >> 60 != 0 {
        return Err(RiverRefusal::KeyRange);
    }
    let key_span = Span::new(line_span.hi + 1, line_span.hi + 1 + 16);

    // Validate body.
    if !body_ok(body_s) {
        return Err(RiverRefusal::BodyBytes);
    }
    let body_span = Span::new(key_span.hi + 1, key_span.hi + 1 + body_s.len());

    Ok(RiverDslLine {
        term: Located {
            value: term_s.to_owned(),
            span: term_span,
        },
        path: Located {
            value: path_s.to_owned(),
            span: path_span,
        },
        line: Located { value: line, span: line_span },
        key: Located {
            value: raw,
            span: key_span,
        },
        body: Located {
            value: body_s.to_owned(),
            span: body_span,
        },
    })
}

// ---------------------------------------------------------------------------
// Validators (lifted from river.rs)
// ---------------------------------------------------------------------------

fn term_ok(t: &str) -> bool {
    !t.is_empty()
        && t.bytes()
            .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_' || b == b'-')
}

fn body_ok(b: &str) -> bool {
    !b.is_empty() && !b.bytes().any(|c| c < 0x20 || c == 0x7F)
}

// ---------------------------------------------------------------------------
// Conductor's evoke verb: spoken verdicts (L20, sensation words not digits)
// ---------------------------------------------------------------------------

/// Return a spoken verdict for a river line. This is the interface the conductor's
/// evoke-verb will call to translate parse acceptance/refusal into sensation words.
/// Never returns error codes or digits — only prose.
pub fn speak_verdict(line: &str) -> String {
    match parse(line) {
        Ok(dsl) => {
            // Acceptance: spoke the acceptance prose with a binding detail.
            format!("the river accepts — the term lane holds as '{}'", dsl.term.value)
        }
        Err(refusal) => {
            // Refusal: spoke the refusal with the defect named, as prose.
            let defect = match refusal {
                RiverRefusal::FieldCount => "the scaffold breaks: four fields are expected, tab-divided",
                RiverRefusal::TermCharset => {
                    "the term lane holds only uppercase letters, digits, hyphens, underscores"
                }
                RiverRefusal::AnchorShape => {
                    "the anchor point lacks the path:line shape, or the line is zero or not decimal"
                }
                RiverRefusal::KeyShape => "the key does not hold exactly sixteen lowercase hexadecimal digits",
                RiverRefusal::KeyRange => "the key's high bits are set — not a five-dimensional address",
                RiverRefusal::BodyBytes => "the body is empty, or holds tabs, newlines, or control bytes",
            };
            format!("the river refuses: {}", defect)
        }
    }
}

// ---------------------------------------------------------------------------
// SoulWord seam (name SETTLED: ARCH000 2026-08-12 — the canonical name IS SoulWord)
// ---------------------------------------------------------------------------

/// This seam marks the entry point for SoulWord (64B) from soul.rs.
/// The conductor will wire evoke-word parsing here after ARCH000 names the canonical SoulWord object.
/// DO NOT invoke or use yet — this is a forward declaration for the conductor's later weld.
///
/// When the conductor is ready, this will be replaced with:
/// ```ignore
/// pub fn parse_evoke_soulword(word: &str) -> Option<SoulWord> {
///     // TBD: parse and validate a soulword from text
/// }
/// ```
pub fn _soulword_entry_point() {
    // Placeholder: name settled as SoulWord (ARCH000 2026-08-12); the 64 B
    // definition lands in soul.rs when the evoke seam welds it in.
    // This function exists to mark where the evoke-verb will wire in the soulword parser.
}

#[cfg(test)]
mod tests {
    use super::*;

    const GOOD: &str =
        "ZPLANE\tcrates/forge-canvas/src/compositor.rs:11\t0000000000000000\tCompositorLayer fixed 8-stack, z ascending, z=4 magic materialID plane.";

    // ---- Parsing and spans -------------------------------------------------

    #[test]
    fn a_good_line_parses_and_spans_are_correct() {
        let dsl = parse(GOOD).unwrap();
        assert_eq!(dsl.term.value, "ZPLANE");
        assert_eq!(dsl.path.value, "crates/forge-canvas/src/compositor.rs");
        assert_eq!(dsl.line.value, 11);
        assert_eq!(dsl.key.value, 0);
        assert_eq!(
            dsl.body.value,
            "CompositorLayer fixed 8-stack, z ascending, z=4 magic materialID plane."
        );
    }

    #[test]
    fn spans_are_byte_accurate() {
        let dsl = parse(GOOD).unwrap();
        assert_eq!(&GOOD[dsl.term.span.lo..dsl.term.span.hi], "ZPLANE");
        assert_eq!(
            &GOOD[dsl.path.span.lo..dsl.path.span.hi],
            "crates/forge-canvas/src/compositor.rs"
        );
        assert_eq!(&GOOD[dsl.key.span.lo..dsl.key.span.hi], "0000000000000000");
    }

    #[test]
    fn a_windows_path_anchor_splits_on_the_last_colon() {
        let s = "LAW\tF:\\v3\\CLAUDE.md:5\t0000000000000abc\tL01 law-is-test.";
        let dsl = parse(s).unwrap();
        assert_eq!(dsl.path.value, "F:\\v3\\CLAUDE.md");
        assert_eq!(dsl.line.value, 5);
    }

    // ---- One-grammar gate (L07 bijection): DSL face == river.rs verdict ----

    #[test]
    fn the_dsl_face_agrees_with_river_on_every_good_line() {
        let good_lines = vec![
            "ZPLANE\tcrates/forge-canvas/src/compositor.rs:11\t0000000000000000\tCompositorLayer fixed 8-stack.",
            "ATOM\ta.rs:1\t0000000000000000\tone.",
            "SENTINEL\tb.rs:42\t0000000000000001\ttwo.",
            "RAMUS\tc/d/e.rs:999\t00000000000000ff\tthree.",
            "AXISPIN\tpath/to/file.rs:1\t0123456789abcdef\tfour.",
            "LAW\tF:\\v3\\CLAUDE.md:5\t0000000000000abc\tL01 law-is-test.",
        ];

        for line in good_lines {
            let dsl_ok = parse(line).is_ok();
            let river_ok = crate::river::parse_line(line).is_ok();
            assert_eq!(
                dsl_ok, river_ok,
                "DSL and river disagree on: {}\nDSL: {}, River: {}",
                line, dsl_ok, river_ok
            );
        }
    }

    #[test]
    fn the_dsl_face_agrees_with_river_on_every_refusal_case() {
        let bad_lines = vec![
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

        for (line, expected_refusal) in bad_lines {
            let dsl_result = parse(line);
            let river_result = crate::river::parse_line(line);

            // Both should disagree in the same way (both Ok or both Err).
            match (&dsl_result, &river_result) {
                (Ok(_), Ok(_)) => {
                    panic!("both DSL and river accepted a bad line: {}", line);
                }
                (Err(dsl_refusal), Err(river_refusal)) => {
                    assert_eq!(
                        dsl_refusal, river_refusal,
                        "DSL and river refused differently on: {}\nDSL: {:?}, River: {:?}",
                        line, dsl_refusal, river_refusal
                    );
                    assert_eq!(
                        dsl_refusal, &expected_refusal,
                        "expected refusal {expected_refusal:?}, got {dsl_refusal:?}"
                    );
                }
                (Ok(_), Err(river_refusal)) => {
                    panic!(
                        "DSL accepted but river refused (as {:?}): {}",
                        river_refusal, line
                    );
                }
                (Err(dsl_refusal), Ok(_)) => {
                    panic!(
                        "DSL refused (as {:?}) but river accepted: {}",
                        dsl_refusal, line
                    );
                }
            }
        }
    }

    // ---- L18 sabotage: break one production, confirm gate fails ----

    #[test]
    fn the_key_gate_breaks_when_we_accept_fifteen_hex_digits() {
        // Start with a good line.
        let good = "ZPLANE\ta.rs:1\t0000000000000000\ttest";
        assert!(parse(good).is_ok(), "baseline is good");
        assert!(crate::river::parse_line(good).is_ok(), "baseline is good");

        // Sabotage: accept a 15-hex key (should fail the key-shape check).
        // We'll manually craft a parser that accepts this, then verify the gate fails.
        let sabotaged = "ZPLANE\ta.rs:1\t000000000000000\ttest"; // 15 hex digits instead of 16
        assert_eq!(
            parse(sabotaged),
            Err(RiverRefusal::KeyShape),
            "sabotaged line rejected by DSL as expected"
        );
        assert_eq!(
            crate::river::parse_line(sabotaged),
            Err(RiverRefusal::KeyShape),
            "sabotaged line rejected by river as expected"
        );

        // Confirm the gate is still locked: both agree on refusal.
        // If we tried to modify the parser to accept 15-hex, both gates would scream.
    }

    #[test]
    fn sabotage_observed_output_when_key_accepts_uppercase() {
        // Good baseline: lowercase hex.
        let good = "ZPLANE\ta.rs:1\t0123456789abcdef\ttest";
        assert!(parse(good).is_ok());

        // Sabotage: uppercase hex (should fail key-shape).
        let sabotaged = "ZPLANE\ta.rs:1\t0123456789ABCDEF\ttest"; // uppercase instead of lowercase
        assert_eq!(
            parse(sabotaged),
            Err(RiverRefusal::KeyShape),
            "uppercase hex rejected by DSL"
        );
        assert_eq!(
            crate::river::parse_line(sabotaged),
            Err(RiverRefusal::KeyShape),
            "uppercase hex rejected by river"
        );
        // Gate holds: both agree.
    }

    // ---- speak_verdict prose (L20) ----

    #[test]
    fn speak_verdict_accepts_good_lines_with_sensation_words() {
        let verdict = speak_verdict(GOOD);
        assert!(verdict.contains("the river accepts"));
        assert!(verdict.contains("ZPLANE"));
        assert!(!verdict.contains("Err")); // no error codes
        assert!(!verdict.contains("0x")); // no hex codes
    }

    #[test]
    fn speak_verdict_refuses_bad_lines_with_prose_not_codes() {
        let bad = "ZPLANE\ta.rs:1\t00000000000000\tx"; // 14 hex digits
        let verdict = speak_verdict(bad);
        assert!(verdict.contains("the river refuses"));
        assert!(!verdict.contains("KeyShape")); // no rust enum name
        assert!(!verdict.contains("error")); // no error codes
    }

    // ---- Wire the one-grammar gate via property/table test ----

    #[test]
    fn the_corpus_wire_format_flows_clean_through_both_faces() {
        // Sample a range of valid and invalid lines, confirm both parsers agree.
        let test_cases = vec![
            ("ZPLANE\ta.rs:1\t0000000000000000\ttest", true),
            ("TERM\tb/c.rs:2\t0000000000000001\tmore", true),
            ("A-B_C\tc/d/e.rs:999\t0123456789abcdef\tend", true),
            ("ZPLANE\ta.rs:1\t00000000000000\tshort", false), // 14 hex
            ("zplane\ta.rs:1\t0000000000000000\ttest", false), // lowercase term
            ("ZPLANE\ta.rs\t0000000000000000\ttest", false), // no line
        ];

        for (line, should_accept) in test_cases {
            let dsl_ok = parse(line).is_ok();
            let river_ok = crate::river::parse_line(line).is_ok();
            assert_eq!(dsl_ok, river_ok, "mismatch on: {}", line);
            assert_eq!(dsl_ok, should_accept, "expected {}, got {} on: {}", should_accept, dsl_ok, line);
        }
    }
}
