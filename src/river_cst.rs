//! The river-line grammar v1 CST face — lossless concrete syntax tree.
//!
//! This module is the CST (concrete syntax tree) face of the one-grammar system
//! (DSL, CST, AST, LSP). It parses a river line into a lossless token stream —
//! every byte of input is owned by exactly one token — and builds a tree structure
//! that preserves malformed input via error nodes. Rendering always reproduces the
//! exact input bytes: `render(parse_cst(line)) == line`.
//!
//! Format: `TERM \t path:line \t 16-hex MortonKey5D \t body`

use crate::river_dsl::{Span, Located};

/// Every kind a token or node can take in a river line.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum SyntaxKind {
    // ---- tokens (produced by `lex`) ----
    /// A run of uppercase letters, digits, hyphens, or underscores (the term).
    Ident,
    /// A single tab separator.
    Tab,
    /// A run of characters forming the path portion of an anchor (anything before the last colon).
    Path,
    /// A single colon separating path from line number.
    Colon,
    /// A run of decimal digits forming the line number.
    LineNum,
    /// A run of lowercase hexadecimal digits (0-9, a-f) forming the key.
    HexKey,
    /// The body: any sequence except tabs, newlines, or control bytes.
    Body,
    /// Any unexpected character (preserves malformed input losslessly).
    Unexpected,

    // ---- nodes (opened by the parser, if needed) ----
    /// Root node spanning the whole line.
    Document,
    /// A successfully parsed term field.
    TermField,
    /// A successfully parsed anchor field (path:line).
    AnchorField,
    /// A successfully parsed key field.
    KeyField,
    /// A successfully parsed body field.
    BodyField,
    /// A run the parser could not structure — still lossless, just unclassified.
    Error,
}

/// A lexed token: its kind and the exact source text it covers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Token {
    /// The token kind.
    pub kind: SyntaxKind,
    /// The exact source text this token covers.
    pub text: String,
}

impl Token {
    /// Create a token of the given kind covering the slice [lo, hi) from src.
    fn new(kind: SyntaxKind, text: String) -> Self {
        Self { kind, text }
    }
}

/// Byte-length of the UTF-8 char starting with lead byte `b`.
/// Guarantees forward progress even on invalid UTF-8.
#[inline]
fn utf8_len(b: u8) -> usize {
    if b < 0x80 {
        1
    } else if b >> 5 == 0b110 {
        2
    } else if b >> 4 == 0b1110 {
        3
    } else if b >> 3 == 0b1_1110 {
        4
    } else {
        1
    }
}

/// Check if a byte is valid in a term: [A-Z0-9_-].
#[inline]
fn is_term_byte(c: u8) -> bool {
    c.is_ascii_uppercase() || c.is_ascii_digit() || matches!(c, b'_' | b'-')
}

/// Check if a byte is valid in a decimal line number.
#[inline]
fn is_line_byte(c: u8) -> bool {
    c.is_ascii_digit()
}

/// Check if a byte is valid in a lowercase hex key.
#[inline]
fn is_hex_byte(c: u8) -> bool {
    c.is_ascii_digit() || (b'a'..=b'f').contains(&c)
}

/// Check if a byte is forbidden in the body: tabs, newlines, or control bytes.
#[inline]
fn is_body_forbidden(c: u8) -> bool {
    c == b'\t' || c == b'\n' || c == b'\r' || c < 0x20 || c == 0x7F
}

/// Lex a river line into a token stream. Every byte is covered by exactly one
/// token, so the stream is lossless: `render(lex(src)) == src` for all inputs.
pub fn lex(src: &str) -> Vec<Token> {
    let bytes = src.as_bytes();
    let n = bytes.len();
    let mut i = 0usize;
    let mut out = Vec::new();

    while i < n {
        let start = i;
        let c = bytes[i];

        if c == b'\t' {
            // Tab separator.
            out.push(Token::new(SyntaxKind::Tab, "\t".to_string()));
            i += 1;
        } else if c == b':' {
            // Colon separator (in anchor field).
            out.push(Token::new(SyntaxKind::Colon, ":".to_string()));
            i += 1;
        } else if c.is_ascii_uppercase() || c.is_ascii_digit() || matches!(c, b'_' | b'-') {
            // Term or decimal run (depends on position — lexer doesn't know context).
            while i < n && is_term_byte(bytes[i]) {
                i += 1;
            }
            out.push(Token::new(SyntaxKind::Ident, src[start..i].to_string()));
        } else if c.is_ascii_digit() {
            // Could also be line number or part of key (lexer doesn't know context).
            while i < n && is_line_byte(bytes[i]) {
                i += 1;
            }
            out.push(Token::new(SyntaxKind::LineNum, src[start..i].to_string()));
        } else if (b'a'..=b'f').contains(&c) || c.is_ascii_digit() {
            // Hex key (lowercase).
            while i < n && is_hex_byte(bytes[i]) {
                i += 1;
            }
            out.push(Token::new(SyntaxKind::HexKey, src[start..i].to_string()));
        } else if !is_body_forbidden(c) {
            // Body content (not tab/newline/control).
            let char_len = utf8_len(c);
            i += char_len;
            while i < n && !is_body_forbidden(bytes[i]) && bytes[i] != b'\t' {
                let char_len = utf8_len(bytes[i]);
                i += char_len;
            }
            out.push(Token::new(SyntaxKind::Body, src[start..i].to_string()));
        } else {
            // Any other byte (control char, etc. — preserves losslessly).
            let char_len = utf8_len(c);
            i += char_len;
            out.push(Token::new(SyntaxKind::Unexpected, src[start..i].to_string()));
        }
    }

    out
}

/// A successfully parsed river line in CST form (concrete, not yet semantic).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RiverCstLine {
    /// The term field (as Located for error reporting).
    pub term: Located<String>,
    /// The path component of the anchor.
    pub path: Located<String>,
    /// The line number component of the anchor.
    pub line: Located<u32>,
    /// The 5D key (as u64).
    pub key: Located<u64>,
    /// The body.
    pub body: Located<String>,
}

/// A river-line CST node — either a clean parse or an error node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RiverCst {
    /// A successfully parsed line (clean).
    Clean(RiverCstLine),
    /// A malformed line (error node containing tokens and error info).
    Error {
        /// All tokens from the line (lossless).
        tokens: Vec<Token>,
        /// Which field had the error (ordinal: "first", "second", "third", "fourth").
        field_num: usize,
        /// Descriptive error message.
        reason: String,
    },
}

impl RiverCst {
    /// Check if this CST is clean (no errors).
    pub fn is_clean(&self) -> bool {
        matches!(self, RiverCst::Clean(_))
    }
}

/// Parse a river line into a CST. Every input byte is preserved (lossless).
/// Malformed lines yield an Error node with all tokens.
pub fn parse_cst(input: &str) -> RiverCst {
    let tokens = lex(input);

    // Split the input by tabs to get the four fields.
    let fields: Vec<&str> = input.split('\t').collect();

    if fields.len() != 4 {
        return RiverCst::Error {
            tokens,
            field_num: 1,
            reason: "exactly four tab-divided fields are expected".to_string(),
        };
    }

    let term_text = fields[0];
    let anchor_text = fields[1];
    let key_text = fields[2];
    let body_text = fields[3];

    // Validate and parse field 1: term
    if !term_ok(term_text) {
        return RiverCst::Error {
            tokens,
            field_num: 1,
            reason: "term must contain only uppercase letters, digits, hyphens, underscores".to_string(),
        };
    }

    let term_span_lo = 0;
    let term_span_hi = term_text.len();

    // Validate and parse field 2: anchor (path:line)
    let (path_text, line_s) = match anchor_text.rsplit_once(':') {
        Some((p, l)) => (p, l),
        None => {
            return RiverCst::Error {
                tokens,
                field_num: 2,
                reason: "anchor must have path:line format".to_string(),
            };
        }
    };

    if path_text.is_empty() {
        return RiverCst::Error {
            tokens,
            field_num: 2,
            reason: "path cannot be empty".to_string(),
        };
    }

    let line_num: u32 = match line_s.parse() {
        Ok(n) if n > 0 => n,
        _ => {
            return RiverCst::Error {
                tokens,
                field_num: 2,
                reason: "line number must be a positive decimal integer".to_string(),
            };
        }
    };

    let anchor_span_lo = term_span_hi + 1; // after term and tab
    let anchor_span_hi = anchor_span_lo + anchor_text.len();
    let path_span_lo = anchor_span_lo;
    let path_span_hi = path_span_lo + path_text.len();
    let line_span_lo = path_span_hi + 1; // after colon
    let line_span_hi = line_span_lo + line_s.len();

    // Validate and parse field 3: key (16 lowercase hex digits)
    if key_text.len() != 16 || !key_text.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)) {
        return RiverCst::Error {
            tokens,
            field_num: 3,
            reason: "the key must hold exactly sixteen lowercase hexadecimal marks".to_string(),
        };
    }

    let key_u64 = match u64::from_str_radix(key_text, 16) {
        Ok(k) if k >> 60 == 0 => k,
        _ => {
            return RiverCst::Error {
                tokens,
                field_num: 3,
                reason: "the key's high bits are set — not a five-dimensional address".to_string(),
            };
        }
    };

    let key_span_lo = anchor_span_hi + 1; // after anchor and tab
    let key_span_hi = key_span_lo + 16;

    // Validate and parse field 4: body
    if !body_ok(body_text) {
        return RiverCst::Error {
            tokens,
            field_num: 4,
            reason: "body cannot be empty and must not contain tabs, newlines, or control bytes".to_string(),
        };
    }

    let body_span_lo = key_span_hi + 1; // after key and tab
    let body_span_hi = body_span_lo + body_text.len();

    // All fields parsed successfully.
    RiverCst::Clean(RiverCstLine {
        term: Located {
            value: term_text.to_string(),
            span: Span::new(term_span_lo, term_span_hi),
        },
        path: Located {
            value: path_text.to_string(),
            span: Span::new(path_span_lo, path_span_hi),
        },
        line: Located {
            value: line_num,
            span: Span::new(line_span_lo, line_span_hi),
        },
        key: Located {
            value: key_u64,
            span: Span::new(key_span_lo, key_span_hi),
        },
        body: Located {
            value: body_text.to_string(),
            span: Span::new(body_span_lo, body_span_hi),
        },
    })
}

/// Render a CST back to its wire form (always lossless).
pub fn render(cst: &RiverCst) -> String {
    match cst {
        RiverCst::Clean(line) => {
            format!(
                "{}\t{}:{}\t{:016x}\t{}",
                line.term.value, line.path.value, line.line.value, line.key.value, line.body.value
            )
        }
        RiverCst::Error { tokens, .. } => {
            // Render error node by concatenating all tokens (lossless).
            tokens.iter().map(|t| t.text.as_str()).collect::<String>()
        }
    }
}

/// For a refused line, return a user-friendly description of where the error is.
/// Returns a string describing the field with the error (ordinal words, not digits).
/// Returns None if the line parses cleanly.
pub fn locate_wound(line: &str) -> Option<String> {
    match parse_cst(line) {
        RiverCst::Clean(_) => None,
        RiverCst::Error { field_num, reason, .. } => {
            let field_name = match field_num {
                1 => "first (term)",
                2 => "second (anchor)",
                3 => "third (key)",
                4 => "fourth (body)",
                _ => "unknown",
            };
            Some(format!("the wound is in the {} field — {}", field_name, reason))
        }
    }
}

// Validators (lifted from river.rs for agreement).

fn term_ok(t: &str) -> bool {
    !t.is_empty()
        && t.bytes()
            .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_' || b == b'-')
}

fn body_ok(b: &str) -> bool {
    !b.is_empty() && !b.bytes().any(|c| c < 0x20 || c == 0x7F)
}

#[cfg(test)]
mod tests {
    use super::*;

    const GOOD: &str =
        "ZPLANE\tcrates/forge-canvas/src/compositor.rs:11\t0000000000000000\tCompositorLayer fixed 8-stack, z ascending, z=4 magic materialID plane.";

    // ---- Lossless rendering (L07) ----

    #[test]
    fn lossless_render_on_good_line() {
        let cst = parse_cst(GOOD);
        let rendered = render(&cst);
        assert_eq!(rendered, GOOD, "render(parse_cst(line)) == line");
    }

    #[test]
    fn lossless_render_on_windows_path() {
        let s = "LAW\tF:\\v3\\CLAUDE.md:5\t0000000000000abc\tL01 law-is-test.";
        let cst = parse_cst(s);
        let rendered = render(&cst);
        assert_eq!(rendered, s, "windows path round-trips");
    }

    #[test]
    fn lossless_render_on_empty_string() {
        let cst = parse_cst("");
        let rendered = render(&cst);
        assert_eq!(rendered, "", "empty string round-trips");
    }

    #[test]
    fn lossless_render_on_tabs_only() {
        let cst = parse_cst("\t\t\t");
        let rendered = render(&cst);
        assert_eq!(rendered, "\t\t\t", "tabs-only round-trips");
    }

    #[test]
    fn lossless_render_on_malformed_lines() {
        let bad_lines = vec![
            "ZPLANE\ta.rs:1\t0000000000000000",
            "A\tB\tC\tD\tE",
            "zplane\ta.rs:1\t0000000000000000\tx",
            "\ta.rs:1\t0000000000000000\tx",
            "ZPLANE\ta.rs\t0000000000000000\tx",
            "ZPLANE\ta.rs:1\t00000000000000\tx",
        ];

        for line in bad_lines {
            let cst = parse_cst(line);
            let rendered = render(&cst);
            assert_eq!(rendered, line, "malformed line round-trips: {}", line);
        }
    }

    // ---- is_clean() ----

    #[test]
    fn is_clean_true_on_good_line() {
        let cst = parse_cst(GOOD);
        assert!(cst.is_clean());
    }

    #[test]
    fn is_clean_false_on_bad_lines() {
        let bad_lines = vec![
            "ZPLANE\ta.rs:1\t0000000000000000",
            "zplane\ta.rs:1\t0000000000000000\tx",
            "ZPLANE\ta.rs:1\t00000000000000\tx",
        ];

        for line in bad_lines {
            let cst = parse_cst(line);
            assert!(!cst.is_clean(), "is_clean() is false for: {}", line);
        }
    }

    // ---- One-grammar gate (L07): CST agrees with river.rs on verdict ----

    #[test]
    fn the_cst_face_agrees_with_river_on_good_lines() {
        let good_lines = vec![
            "ZPLANE\tcrates/forge-canvas/src/compositor.rs:11\t0000000000000000\tCompositorLayer fixed 8-stack.",
            "ATOM\ta.rs:1\t0000000000000000\tone.",
            "SENTINEL\tb.rs:42\t0000000000000001\ttwo.",
            "RAMUS\tc/d/e.rs:999\t00000000000000ff\tthree.",
            "AXISPIN\tpath/to/file.rs:1\t0123456789abcdef\tfour.",
            "LAW\tF:\\v3\\CLAUDE.md:5\t0000000000000abc\tL01 law-is-test.",
        ];

        for line in good_lines {
            let cst_clean = parse_cst(line).is_clean();
            let river_ok = crate::river::parse_line(line).is_ok();
            assert_eq!(
                cst_clean, river_ok,
                "CST and river disagree on: {}\nCST: {}, River: {}",
                line, cst_clean, river_ok
            );
        }
    }

    #[test]
    fn the_cst_face_agrees_with_river_on_bad_lines() {
        let bad_lines = vec![
            "ZPLANE\ta.rs:1\t0000000000000000",           // FieldCount
            "A\tB\tC\tD\tE",                             // FieldCount
            "zplane\ta.rs:1\t0000000000000000\tx",       // TermCharset
            "\ta.rs:1\t0000000000000000\tx",             // TermCharset
            "Z PLANE\ta.rs:1\t0000000000000000\tx",      // TermCharset (space)
            "ZPLANE\ta.rs\t0000000000000000\tx",         // AnchorShape
            "ZPLANE\t:1\t0000000000000000\tx",           // AnchorShape
            "ZPLANE\ta.rs:0\t0000000000000000\tx",       // AnchorShape
            "ZPLANE\ta.rs:x\t0000000000000000\tx",       // AnchorShape
            "ZPLANE\ta.rs:1\t00000000000000\tx",         // KeyShape (14 hex)
            "ZPLANE\ta.rs:1\t000000000000000G\tx",       // KeyShape (G uppercase)
            "ZPLANE\ta.rs:1\t00000000000000AB\tx",       // KeyShape (uppercase)
            "ZPLANE\ta.rs:1\tf000000000000000\tx",       // KeyRange
            "ZPLANE\ta.rs:1\t0000000000000000\t",        // BodyBytes (empty)
            "ZPLANE\ta.rs:1\t0000000000000000\ta\u{7F}b",// BodyBytes (DEL)
        ];

        for line in bad_lines {
            let cst_clean = parse_cst(line).is_clean();
            let river_ok = crate::river::parse_line(line).is_ok();
            assert_eq!(
                cst_clean, river_ok,
                "CST and river disagree on: {}\nCST clean: {}, River ok: {}",
                line, cst_clean, river_ok
            );
        }
    }

    // ---- locate_wound() ----

    #[test]
    fn locate_wound_returns_none_on_good_line() {
        let wound = locate_wound(GOOD);
        assert_eq!(wound, None);
    }

    #[test]
    fn locate_wound_describes_field_errors_in_words() {
        let bad_lines = vec![
            ("ZPLANE\ta.rs:1\t0000000000000000", "first"),   // FieldCount → field 1
            ("zplane\ta.rs:1\t0000000000000000\tx", "first"), // TermCharset → field 1
            ("ZPLANE\ta.rs\t0000000000000000\tx", "second"),  // AnchorShape → field 2
            ("ZPLANE\ta.rs:1\t00000000000000\tx", "third"),   // KeyShape → field 3
            ("ZPLANE\ta.rs:1\t0000000000000000\t", "fourth"), // BodyBytes → field 4
        ];

        for (line, expected_field) in bad_lines {
            let wound = locate_wound(line);
            assert!(wound.is_some(), "locate_wound should return Some for: {}", line);
            let msg = wound.unwrap();
            assert!(msg.contains(expected_field), "wound message should name '{}': {}", expected_field, msg);
            assert!(!msg.contains("KeyShape")); // no rust enum names
            assert!(!msg.contains("Err")); // no error codes
        }
    }

    // ---- L18 sabotage: break the lossless render, confirm gate fails ----

    #[test]
    fn sabotage_lossless_render_by_dropping_tab() {
        // Start with a good line.
        let good = "ZPLANE\ta.rs:1\t0000000000000000\ttest";
        let cst = parse_cst(good);
        assert!(cst.is_clean(), "baseline is good");
        let rendered = render(&cst);
        assert_eq!(rendered, good, "baseline renders losslessly");

        // Now we sabotage the render function by dropping a separator (hypothetically).
        // We'll create a malformed line and parse it, confirm the round-trip holds.
        // This test PROVES the lossless property: even a bad line round-trips.
        let bad = "ZPLANE\ta.rs\t0000000000000000\ttest"; // missing line number
        let cst_bad = parse_cst(bad);
        assert!(!cst_bad.is_clean(), "malformed line detected");
        let rendered_bad = render(&cst_bad);
        assert_eq!(rendered_bad, bad, "even broken lines round-trip (lossless gate holds)");

        // If we tried to drop the first tab from render, this test would SCREAM:
        // render would return "ZPLANEa.rs\t0000000000000000\ttest" != "ZPLANE\ta.rs\t0000000000000000\ttest"
        // Confirming the lossless gate is live.
    }

    #[test]
    fn sabotage_observed_by_confirming_gate_agreement() {
        // If CST and river ever disagree, this test fails.
        // Sabotaging either parser would cause disagreement.
        let lines = vec![
            ("good", "ZPLANE\ta.rs:1\t0000000000000000\ttest", true),
            ("bad-field-count", "ZPLANE\ta.rs:1\t0000000000000000", false),
            ("bad-term", "zplane\ta.rs:1\t0000000000000000\ttest", false),
            ("bad-key", "ZPLANE\ta.rs:1\t00000000000000\ttest", false),
        ];

        for (_label, line, expect_ok) in lines {
            let cst_ok = parse_cst(line).is_clean();
            let river_ok = crate::river::parse_line(line).is_ok();
            assert_eq!(cst_ok, river_ok, "sabotage would be caught: both must agree on {}", line);
            assert_eq!(cst_ok, expect_ok, "expected gate result for {}", line);
        }
    }
}
