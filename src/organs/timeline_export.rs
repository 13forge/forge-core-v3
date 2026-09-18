//! `timeline_export` — the spool comes OUT THE END as a `.vixi` timeline document.
//!
//! `FrameSample` records (frame, object position) serialize to a deterministic, line-based
//! `.vixi` text — one `frame … obj … pos …` record per sample under a magic header — and
//! parses back byte-for-byte (round-trip proven). Frozen integer payload only (frame u64 /
//! id u32 / pos i32 — no float, no wall-clock), so same samples in → identical bytes out.
//!
//! Ported from `F:\NewRepo\crates\forge-studio\src\timeline_export.rs` (v2 donor, 169 LOC).
//! Ported 2026-08-17. JSON parsing/writing hand-rolled (forge-core-v3 stays zero-dep except bytemuck).

#![allow(missing_docs, dead_code)]

/// EMBED_DIM = 5 (ported from forge_ml_bqrouter::nearest_neighbor::EMBED_DIM).
/// Hardcoded here to avoid adding a dependency; forge-core-v3 is Crate Zero.
const EMBED_DIM: usize = 5;

/// An object located in the 5D field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FieldObject {
    /// Object ID.
    pub id: u32,
    /// Position in 5D field coordinates.
    pub pos: [i32; EMBED_DIM],
}

/// One `(frame, object)` moment: where an object was at a playhead frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameSample {
    /// Playhead frame index.
    pub frame: u64,
    /// The object at that frame.
    pub obj: FieldObject,
}

/// Magic header line — distinctive enough that a future `Format::sniff` arm can
/// key on it without colliding with any existing signature.
pub const VIXI_TIMELINE_MAGIC: &str = "#vixi-timeline v1";

/// What went wrong loading a `.vixi` timeline — LOUD, never a silent empty parse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExportError {
    /// The magic header line was missing or wrong.
    MissingMagic,
    /// A record line did not parse (1-indexed line number).
    BadRecord { line: usize },
}

/// Serialize a drained spool window to `.vixi` timeline text. Deterministic:
/// identical samples in → identical bytes out (stable order, integer only).
pub fn to_vixi(name: &str, samples: &[FrameSample]) -> String {
    let mut out = String::new();
    out.push_str(VIXI_TIMELINE_MAGIC);
    out.push('\n');
    out.push_str("timeline \"");
    out.push_str(&sanitize(name));
    out.push_str("\" frames ");
    out.push_str(&samples.len().to_string());
    out.push('\n');
    for s in samples {
        out.push_str("frame ");
        out.push_str(&s.frame.to_string());
        out.push_str(" obj ");
        out.push_str(&s.obj.id.to_string());
        out.push_str(" pos");
        for c in s.obj.pos {
            out.push(' ');
            out.push_str(&c.to_string());
        }
        out.push('\n');
    }
    out
}

/// The `.vixi` document as bytes — the cart artifact payload.
pub fn to_vixi_bytes(name: &str, samples: &[FrameSample]) -> Vec<u8> {
    to_vixi(name, samples).into_bytes()
}

/// Parse `.vixi` timeline text back into samples — the round-trip inverse of
/// [`to_vixi`]. The `timeline "…" frames N` header and blank lines are skipped;
/// every other line must be a well-formed record or the load is LOUD.
pub fn from_vixi(text: &str) -> Result<Vec<FrameSample>, ExportError> {
    let mut lines = text.lines().enumerate();
    match lines.next() {
        Some((_, l)) if l.trim() == VIXI_TIMELINE_MAGIC => {}
        _ => return Err(ExportError::MissingMagic),
    }
    let mut out = Vec::new();
    for (i, l) in lines {
        let l = l.trim();
        if l.is_empty() || l.starts_with("timeline ") {
            continue;
        }
        out.push(parse_record(l).ok_or(ExportError::BadRecord { line: i + 1 })?);
    }
    Ok(out)
}

/// `frame <f> obj <id> pos <p0> … <p4>` — exactly `EMBED_DIM` coordinates, no more.
fn parse_record(l: &str) -> Option<FrameSample> {
    let mut it = l.split_whitespace();
    if it.next()? != "frame" {
        return None;
    }
    let frame: u64 = it.next()?.parse().ok()?;
    if it.next()? != "obj" {
        return None;
    }
    let id: u32 = it.next()?.parse().ok()?;
    if it.next()? != "pos" {
        return None;
    }
    let mut pos = [0i32; EMBED_DIM];
    for slot in pos.iter_mut() {
        *slot = it.next()?.parse().ok()?;
    }
    if it.next().is_some() {
        return None; // trailing garbage past the 5 coordinates
    }
    Some(FrameSample { frame, obj: FieldObject { id, pos } })
}

/// Keep the timeline name on one quoted line — escape quotes and newlines.
fn sanitize(name: &str) -> String {
    name.chars()
        .map(|c| if c == '"' || c == '\n' || c == '\r' { '_' } else { c })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(frame: u64, id: u32, x: i32) -> FrameSample {
        FrameSample {
            frame,
            obj: FieldObject {
                id,
                pos: [x, -1, 2, 3, 4],
            },
        }
    }

    #[test]
    fn round_trip_is_lossless() {
        let samples = vec![
            sample(0, 1, 100),
            sample(4, 1, 200),
            sample(4, 2, -50),
            sample(8, 1, 300),
        ];
        let vixi = to_vixi("clip", &samples);
        let back = from_vixi(&vixi).expect("valid .vixi");
        assert_eq!(back, samples, "spool → .vixi → spool is byte-faithful");
    }

    #[test]
    fn serialization_is_deterministic() {
        let samples = vec![sample(0, 1, 7), sample(1, 2, 9)];
        assert_eq!(
            to_vixi("a", &samples),
            to_vixi("a", &samples),
            "same in → same bytes out"
        );
    }

    #[test]
    fn header_carries_magic_and_count() {
        let vixi = to_vixi("my clip", &[sample(0, 1, 1), sample(1, 1, 2)]);
        let mut lines = vixi.lines();
        assert_eq!(lines.next().unwrap(), VIXI_TIMELINE_MAGIC);
        assert_eq!(lines.next().unwrap(), "timeline \"my clip\" frames 2");
    }

    #[test]
    fn empty_spool_exports_a_valid_empty_timeline() {
        let vixi = to_vixi("empty", &[]);
        assert_eq!(from_vixi(&vixi), Ok(Vec::new()));
    }

    #[test]
    fn missing_magic_is_loud() {
        assert_eq!(
            from_vixi("frame 0 obj 1 pos 0 0 0 0 0"),
            Err(ExportError::MissingMagic)
        );
    }

    #[test]
    fn malformed_record_is_loud_with_line_number() {
        let bad = format!("{VIXI_TIMELINE_MAGIC}\nframe 0 obj 1 pos 0 0 0\n"); // only 3 coords
        assert_eq!(from_vixi(&bad), Err(ExportError::BadRecord { line: 2 }));
    }

    #[test]
    fn trailing_garbage_past_the_five_coords_is_loud() {
        let bad = format!("{VIXI_TIMELINE_MAGIC}\nframe 0 obj 1 pos 0 0 0 0 0 99\n");
        assert!(matches!(
            from_vixi(&bad),
            Err(ExportError::BadRecord { .. })
        ));
    }

    #[test]
    fn round_trip_with_sanitized_name() {
        let samples = vec![sample(0, 1, 100)];
        let vixi = to_vixi("my \"clip\" with\nnewline", &samples);
        let lines: Vec<&str> = vixi.lines().collect();
        assert!(lines[1].contains("my _clip_ with_newline"));
        let back = from_vixi(&vixi).expect("valid .vixi");
        assert_eq!(back, samples);
    }
}
