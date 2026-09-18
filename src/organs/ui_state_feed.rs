//! UI state feed — zero-dependency parser for the `ui-state` stdout IPC document
//! (the 4th emitter, `forge_vix::emit_state`).
//!
//! A typed scanner for the deterministic JSON schema only. Parse allocates once per
//! document; the frame loop reads the cached struct — never per-frame allocation.
//!
//! Entry: `run_query(&[String]) -> i32` — the `vixi-query` verb.

/// One node row in the UI state document: pixel rect + tile cell + z-depth + bind channel.
///
/// The `z` field is the depth for reverse-z hit-testing (topmost-painted node answers).
/// `bind` is optional and names the S-channel subscription (e.g., `session.status`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UiNode {
    /// Stable key identifier for this UI node.
    pub key: String,
    /// Pixel x-coordinate of the rect origin.
    pub x: i64,
    /// Pixel y-coordinate of the rect origin.
    pub y: i64,
    /// Pixel width of the rect.
    pub w: i64,
    /// Pixel height of the rect.
    pub h: i64,
    /// Tile grid x0 coordinate.
    pub tx0: i32,
    /// Tile grid y0 coordinate.
    pub ty0: i32,
    /// Tile grid x1 coordinate.
    pub tx1: i32,
    /// Tile grid y1 coordinate.
    pub ty1: i32,
    /// Z-depth for reverse-z hit-testing (higher = painted later = topmost).
    pub z: i32,
    /// Optional S-channel bind name (subscription target).
    pub bind: Option<String>,
}

/// The parsed feed document: viewport + tile grid + plane + phase + node list.
///
/// This is the top-level structure produced by `parse_ui_state`. It caches the entire
/// layout for the frame loop to query without re-parsing or allocating.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct UiStateDoc {
    /// Viewport width in pixels.
    pub vp_w: i64,
    /// Viewport height in pixels.
    pub vp_h: i64,
    /// Tile size in pixels (square).
    pub tile: i64,
    /// Number of tiles horizontally.
    pub tiles_x: i32,
    /// Number of tiles vertically.
    pub tiles_y: i32,
    /// Plane z-depth ceiling.
    pub z: i32,
    /// Plane scale identifier.
    pub s: u32,
    /// Frame phase timestamp.
    pub t: u32,
    /// All UI nodes in the document.
    pub nodes: Vec<UiNode>,
}

/// Parse a UI state document from JSON text.
///
/// This is a strict, deterministic scanner for the exact schema emitted by
/// `emit_ui_state_json` (the 4th emitter in forge-vix). Any deviation from that
/// schema returns `None` — a typed miss, never a guess.
///
/// # Returns
/// `Some(UiStateDoc)` if the input is a valid ui-state document, `None` otherwise.
pub fn parse_ui_state(doc: &str) -> Option<UiStateDoc> {
    let mut c = Cur { b: doc.as_bytes(), i: 0 };
    c.lit("{\"vixi_ui_state\":1,\"vp\":{\"w\":")?;
    let vp_w = c.int()?;
    c.lit(",\"h\":")?;
    let vp_h = c.int()?;
    c.lit("},\"tile\":")?;
    let tile = c.int()?;
    c.lit(",\"tiles\":{\"x\":")?;
    let tiles_x = c.int()? as i32;
    c.lit(",\"y\":")?;
    let tiles_y = c.int()? as i32;
    c.lit("},\"plane\":{\"z\":")?;
    let z = c.int()? as i32;
    c.lit(",\"s\":")?;
    let s = c.int()? as u32;
    c.lit("},\"t\":")?;
    let t = c.int()? as u32;
    c.lit(",\"nodes\":[")?;
    let mut nodes = Vec::new();
    if !c.peek_lit("]") {
        loop {
            c.lit("{\"key\":\"")?;
            let key = c.string()?;
            c.lit(",\"px\":{\"x\":")?;
            let x = c.int()?;
            c.lit(",\"y\":")?;
            let y = c.int()?;
            c.lit(",\"w\":")?;
            let w = c.int()?;
            c.lit(",\"h\":")?;
            let h = c.int()?;
            c.lit("},\"tile\":{\"x0\":")?;
            let tx0 = c.int()? as i32;
            c.lit(",\"y0\":")?;
            let ty0 = c.int()? as i32;
            c.lit(",\"x1\":")?;
            let tx1 = c.int()? as i32;
            c.lit(",\"y1\":")?;
            let ty1 = c.int()? as i32;
            c.lit("},\"z\":")?;
            let nz = c.int()? as i32;
            let bind = if c.peek_lit(",\"bind\":\"") {
                c.lit(",\"bind\":\"")?;
                Some(c.string()?)
            } else {
                None
            };
            c.lit("}")?;
            nodes.push(UiNode { key, x, y, w, h, tx0, ty0, tx1, ty1, z: nz, bind });
            if c.peek_lit(",") {
                c.lit(",")?;
            } else {
                break;
            }
        }
    }
    c.lit("]}")?;
    Some(UiStateDoc { vp_w, vp_h, tile, tiles_x, tiles_y, z, s, t, nodes })
}

impl UiStateDoc {
    /// Find the topmost (highest z-depth) node whose pixel rect contains `(x, y)`.
    ///
    /// Performs reverse-z hit-testing in the same order the renderer paints:
    /// the last-painted (highest z) node wins. Borrows the cache with no allocation,
    /// safe for per-frame or per-press queries.
    ///
    /// # Arguments
    /// * `x` - Pixel x-coordinate to test.
    /// * `y` - Pixel y-coordinate to test.
    ///
    /// # Returns
    /// A reference to the topmost node at that point, or `None` if no node contains it.
    pub fn topmost_at(&self, x: i64, y: i64) -> Option<&UiNode> {
        self.nodes
            .iter()
            .filter(|n| x >= n.x && x < n.x + n.w && y >= n.y && y < n.y + n.h)
            .max_by_key(|n| n.z)
    }
}

/// Format a node as a query result row.
///
/// Produces one line of tab-separated text suitable for stdout, in a stable field order:
/// `key  x,y WxH  z=N  tile=x0,y0..x1,y1  bind=…`
///
/// # Arguments
/// * `n` - The UI node to format.
///
/// # Returns
/// A formatted string with the node's data.
pub fn query_row(n: &UiNode) -> String {
    format!(
        "{}\t{},{} {}x{}\tz={}\ttile={},{}..{},{}\tbind={}",
        n.key,
        n.x,
        n.y,
        n.w,
        n.h,
        n.z,
        n.tx0,
        n.ty0,
        n.tx1,
        n.ty1,
        n.bind.as_deref().unwrap_or("-")
    )
}

/// Parse the `--at` argument format: `<x>,<y>`, integers only.
///
/// This is a pure parsing function suitable for testing the CLI argument semantics.
///
/// # Arguments
/// * `s` - A string in the format `<integer>,<integer>`.
///
/// # Returns
/// `Some((x, y))` if the string parses successfully, `None` otherwise.
pub fn parse_at(s: &str) -> Option<(i64, i64)> {
    let (x, y) = s.split_once(',')?;
    Some((x.trim().parse().ok()?, y.trim().parse().ok()?))
}

/// The `vixi-query` verb: hit-test the UI state document at a point.
///
/// Reads a ui-state document from stdin, parses it, and performs a hit-test
/// at the given point. Outputs the topmost node's row, or an error message.
///
/// # Arguments
/// * `args` - Command-line arguments (e.g., `["--at", "64,32"]`).
///
/// # Exit Codes
/// * `0` — A node was found and printed.
/// * `1` — No node at the requested point.
/// * `2` — Usage error or unparseable document.
pub fn run_query(args: &[String]) -> i32 {
    use std::io::Read as _;

    let mut at: Option<(i64, i64)> = None;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--at" => match it.next().map(String::as_str).and_then(parse_at) {
                Some(p) => at = Some(p),
                None => {
                    eprintln!("[vixi query] --at needs <x>,<y>");
                    return 2;
                }
            },
            other => {
                eprintln!("[vixi query] unknown arg: {other}");
                return 2;
            }
        }
    }
    let Some((x, y)) = at else {
        eprintln!("[vixi query] --at <x>,<y> is required");
        return 2;
    };

    let mut doc = String::new();
    if std::io::stdin().read_to_string(&mut doc).is_err() {
        eprintln!("[vixi query] could not read the ui-state doc from stdin");
        return 2;
    }
    let Some(parsed) = parse_ui_state(doc.trim()) else {
        eprintln!("[vixi query] stdin is not a ui-state doc — pipe `emit_ui_state_json` output");
        return 2;
    };
    match parsed.topmost_at(x, y) {
        Some(n) => {
            println!("{}", query_row(n));
            0
        }
        None => {
            eprintln!("[vixi query] no node at {x},{y}");
            1
        }
    }
}

/// Byte cursor: strict scanner for literals, signed integers, and JSON string unescaping.
///
/// This is an internal helper for parsing the ui-state JSON document.
/// It performs the same escape handling as the emitter (`\\` and `\"`).
struct Cur<'a> {
    b: &'a [u8],
    i: usize,
}

impl<'a> Cur<'a> {
    /// Match and consume an exact literal string, or return `None`.
    fn lit(&mut self, s: &str) -> Option<()> {
        let n = s.len();
        if self.b.len() - self.i >= n && &self.b[self.i..self.i + n] == s.as_bytes() {
            self.i += n;
            Some(())
        } else {
            None
        }
    }

    /// Test whether the next bytes match a literal (without consuming).
    fn peek_lit(&self, s: &str) -> bool {
        let n = s.len();
        self.b.len() - self.i >= n && &self.b[self.i..self.i + n] == s.as_bytes()
    }

    /// Parse and consume a signed integer.
    fn int(&mut self) -> Option<i64> {
        let neg = self.peek_lit("-");
        if neg {
            self.i += 1;
        }
        let start = self.i;
        while self.i < self.b.len() && self.b[self.i].is_ascii_digit() {
            self.i += 1;
        }
        if self.i == start {
            return None;
        }
        let v: i64 = std::str::from_utf8(&self.b[start..self.i]).ok()?.parse().ok()?;
        Some(if neg { -v } else { v })
    }

    /// Parse and consume a JSON string, unescaping `\\` and `\"`.
    ///
    /// Reads up to and consumes the closing unescaped `"`.
    fn string(&mut self) -> Option<String> {
        let mut out = String::new();
        loop {
            match *self.b.get(self.i)? {
                b'"' => {
                    self.i += 1;
                    return Some(out);
                }
                b'\\' => {
                    let esc = *self.b.get(self.i + 1)?;
                    if esc != b'"' && esc != b'\\' {
                        return None;
                    }
                    out.push(esc as char);
                    self.i += 2;
                }
                _ => {
                    let start = self.i;
                    while self.i < self.b.len() && self.b[self.i] != b'"' && self.b[self.i] != b'\\'
                    {
                        self.i += 1;
                    }
                    out.push_str(std::str::from_utf8(&self.b[start..self.i]).ok()?);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Parse `--at <x>,<y>` format correctly, rejecting malformed inputs.
    #[test]
    fn parse_at_accepts_valid_coordinates_and_whitespace() {
        assert_eq!(parse_at("64,32"), Some((64, 32)));
        assert_eq!(parse_at(" 8 , 9 "), Some((8, 9)));
        assert_eq!(parse_at("64"), None, "missing y fails");
        assert_eq!(parse_at("64,y"), None, "non-numeric y fails");
        assert_eq!(parse_at(""), None, "empty fails");
        assert_eq!(parse_at(","), None, "empty coordinates fail");
    }

    /// Query row formatting produces the correct tab-separated output.
    #[test]
    fn query_row_formats_nodes_correctly() {
        let node = UiNode {
            key: "test.node".to_string(),
            x: 10,
            y: 20,
            w: 100,
            h: 50,
            tx0: 0,
            ty0: 0,
            tx1: 3,
            ty1: 1,
            z: 7,
            bind: Some("session.status".into()),
        };
        let row = query_row(&node);
        assert!(row.contains("test.node"), "key is in output");
        assert!(row.contains("10,20"), "coordinates in output");
        assert!(row.contains("100x50"), "dimensions in output");
        assert!(row.contains("z=7"), "z-depth in output");
        assert!(row.contains("0,0..3,1"), "tile bounds in output");
        assert!(row.contains("session.status"), "bind in output");

        let unbound = UiNode { bind: None, ..node };
        let row_unbound = query_row(&unbound);
        assert!(row_unbound.contains("bind=-"), "no bind shows as dash");
    }

    /// Reverse-z hit-testing: highest z-depth node at a point wins.
    #[test]
    fn topmost_at_returns_highest_z_under_point() {
        let node = |key: &str, z: i32| UiNode {
            key: key.to_string(),
            x: 0,
            y: 0,
            w: 100,
            h: 50,
            tx0: 0,
            ty0: 0,
            tx1: 3,
            ty1: 1,
            z,
            bind: None,
        };
        let doc = UiStateDoc {
            nodes: vec![node("under", 1), UiNode { z: 7, bind: Some("s".into()), ..node("over", 7) }],
            ..UiStateDoc::default()
        };

        let hit = doc.topmost_at(10, 10).expect("point is inside both rects");
        assert_eq!(hit.key, "over", "reverse-Z: the painted-last node answers");
        let row = query_row(hit);
        assert_eq!(row, "over\t0,0 100x50\tz=7\ttile=0,0..3,1\tbind=s");

        assert!(doc.topmost_at(500, 500).is_none(), "outside every rect");
        assert!(doc.topmost_at(100, 100).is_none(), "far edge is exclusive");
    }

    /// Run query validates its arguments and returns the correct exit codes.
    #[test]
    fn run_query_rejects_missing_or_invalid_arguments() {
        // No --at argument: usage error (exit 2).
        assert_eq!(run_query(&[]), 2, "missing --at is exit 2");

        // Unparseable coordinate: usage error (exit 2).
        let args = vec!["--at".to_string(), "nope".to_string()];
        assert_eq!(run_query(&args), 2, "unparseable point is exit 2");

        // Unknown argument: usage error (exit 2).
        let args = vec!["--unknown".to_string()];
        assert_eq!(run_query(&args), 2, "unknown arg is exit 2");
    }

    // v3: dropped tests that needed `forge_vix`:
    // - `the_query_verb_is_reachable_from_the_one_bin` (requires reading main.rs)
    // - `feed_round_trips_the_fourth_emitter_doc` (needs forge_vix::emit_state)
    // - `truncated_or_foreign_docs_return_none_not_garbage` (needs forge_vix::emit_state)
    // - `topmost_at_reads_the_cache_reverse_z` (needs forge_vix::ir types)
    //
    // These tests verify the round-trip with the actual emitter and prove
    // reachability from main.rs; they belong in the organ's downstream, which
    // is where its deps live (L06, Sean 2026-08-17).
}
