//! Visual Gate — argument parsing and pixel inspection utilities.
//!
//! This is a PARTIAL port of F:\NewRepo\crates\forge-studio\src\visual_gate.rs (1108 LOC).
//! Crate Zero constraint: zero GUI/GPU deps. We keep ONLY portable primitives:
//! argument parsing (VisualGateArgs, parse_args), BMP file I/O (read_bmp, write_bmp),
//! and pixel inspection (diff_score).
//!
//! All per-panel capture functions are dropped — they require forge_studio,
//! forge_canvas, forge_vix, forge_gpu, forge_export, and other GUI libs not
//! available in Crate Zero. Visual gate ORCHESTRATION (run_anim, run_6up, etc.)
//! is dropped for the same reason.
//!
//! Headless capture in v3 is handled by `cargo xtask photon` (xtask/src/photon.rs),
//! which drives the shell binary and captures screenshots. This module is a library
//! for tools that need to inspect pixels (BMP readback, diff-scoring, etc.) without
//! GUI deps.

use std::path::{Path, PathBuf};

/// Parsed command-line arguments for visual gate (pixel inspection / baseline diffing).
pub struct VisualGateArgs {
    /// Panel name to capture/inspect.
    pub panel: String,
    /// Path to baseline image (BMP) for comparison.
    pub baseline: PathBuf,
    /// Threshold for pixel diff; capture passes if diff <= threshold.
    pub threshold: u64,
    /// If true, write the captured pixels to baseline and exit.
    pub bless: bool,
    /// `--png <path>`: dump the capture to a Read-able PNG instead of gating.
    /// Single panel → that file; 6UP/ALL → `<path>` is a dir, one PNG per tab.
    /// The headless machine-eyes path — no live window, no baseline needed.
    pub png: Option<PathBuf>,
    /// `--size WxH`: native render resolution for the kit-render path. Defaults to
    /// the studio's own window size (1280×720) so headless captures show the layout
    /// at real peaks, not a crushed 256 kiosk.
    pub w: u32,
    /// Height of render resolution.
    pub h: u32,
    /// `--frames 0,8,16,24` or `--frames N@STEP` — the tween frame indices the ANIM
    /// route drives. Empty everywhere else; a still capture is frame-less, not frame-0.
    pub frames: Vec<u64>,
}

/// Default tween sample: onset / half-period / full-period / past-full, the ADR-0008
/// discrimination points `compose_vixi_tween_readback` was built to hit.
const DEFAULT_FRAMES: [u64; 4] = [0, 8, 16, 24];

/// `--frames` → explicit indices (`0,8,16`) or a count@step shorthand (`4@8`).
/// Unparsable input falls to [`DEFAULT_FRAMES`] rather than capturing nothing.
fn parse_frames(spec: &str) -> Vec<u64> {
    if let Some((n, step)) = spec.split_once('@') {
        if let (Ok(n), Ok(step)) = (n.trim().parse::<u64>(), step.trim().parse::<u64>()) {
            if n > 0 {
                return (0..n).map(|i| i * step).collect();
            }
        }
    }
    let list: Vec<u64> = spec.split(',').filter_map(|s| s.trim().parse().ok()).collect();
    if list.is_empty() { DEFAULT_FRAMES.to_vec() } else { list }
}

fn default_baselines_dir() -> PathBuf {
    std::env::var("FORGE_VISUAL_GATE_BASELINES_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("crates/forge-studio/qa/visual_gate_baselines"))
}

/// `args[0]` is the "visual-gate" placeholder token (main.rs convention); the
/// panel name is positional at `args[1]`.
pub fn parse_args(args: &[String]) -> Option<VisualGateArgs> {
    let panel = args.get(1)?.clone();
    let bless = args.iter().any(|a| a == "--bless");
    let baseline = args
        .iter()
        .position(|a| a == "--baseline")
        .and_then(|i| args.get(i + 1))
        .map(PathBuf::from)
        .unwrap_or_else(|| default_baselines_dir().join(format!("{}.bmp", panel.to_lowercase())));
    let threshold = args
        .iter()
        .position(|a| a == "--threshold")
        .and_then(|i| args.get(i + 1))
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let png = args
        .iter()
        .position(|a| a == "--png")
        .and_then(|i| args.get(i + 1))
        .map(PathBuf::from);
    let (w, h) = args
        .iter()
        .position(|a| a == "--size")
        .and_then(|i| args.get(i + 1))
        .and_then(|s| s.split_once(['x', 'X']))
        .and_then(|(a, b)| Some((a.trim().parse().ok()?, b.trim().parse().ok()?)))
        .unwrap_or((1280, 720));
    let frames = args
        .iter()
        .position(|a| a == "--frames")
        .and_then(|i| args.get(i + 1))
        .map(|s| parse_frames(s))
        .unwrap_or_default();
    Some(VisualGateArgs { panel, baseline, threshold, bless, png, w, h, frames })
}

/// Minimal 32bpp BGRA BMP reader (54-byte header). Handles both storage
/// orders: positive height = bottom-up (this module's own writer), negative
/// height = top-down (technothesia's `write_bmp_rgba`). Returns (RGBA
/// pixels, width, height) in top-down memory order (row 0 = top).
pub fn read_bmp(path: &Path) -> Option<(Vec<u8>, u32, u32)> {
    use std::io::Read;
    let mut file = std::fs::File::open(path).ok()?;
    let mut header = [0u8; 54];
    file.read_exact(&mut header).ok()?;
    let width = u32::from_le_bytes([header[18], header[19], header[20], header[21]]);
    let height_raw = i32::from_le_bytes([header[22], header[23], header[24], header[25]]);
    let top_down = height_raw < 0;
    let height = height_raw.unsigned_abs();
    let mut bgra = vec![0u8; (width as usize) * (height as usize) * 4];
    file.read_exact(&mut bgra).ok()?;
    let mut rgba = vec![0u8; bgra.len()];
    for row in 0..height as usize {
        let src_row = if top_down { row } else { height as usize - 1 - row };
        for px in 0..width as usize {
            let s = (src_row * width as usize + px) * 4;
            let d = (row * width as usize + px) * 4;
            rgba[d] = bgra[s + 2];
            rgba[d + 1] = bgra[s + 1];
            rgba[d + 2] = bgra[s];
            rgba[d + 3] = bgra[s + 3];
        }
    }
    Some((rgba, width, height))
}

/// Minimal 32bpp BGRA BMP writer — always bottom-up (positive height),
/// readable by `read_bmp` above and any standard viewer.
pub fn write_bmp(path: &Path, rgba: &[u8], width: u32, height: u32) -> std::io::Result<()> {
    use std::io::Write;
    let row_bytes = width as usize * 4;
    let pixel_bytes = row_bytes * height as usize;
    let file_size = 54 + pixel_bytes;
    let mut header = [0u8; 54];
    header[0] = b'B';
    header[1] = b'M';
    header[2..6].copy_from_slice(&(file_size as u32).to_le_bytes());
    header[10..14].copy_from_slice(&54u32.to_le_bytes()); // pixel data offset
    header[14..18].copy_from_slice(&40u32.to_le_bytes()); // DIB header size
    header[18..22].copy_from_slice(&width.to_le_bytes());
    header[22..26].copy_from_slice(&height.to_le_bytes());
    header[26..28].copy_from_slice(&1u16.to_le_bytes()); // planes
    header[28..30].copy_from_slice(&32u16.to_le_bytes()); // bpp
    header[34..38].copy_from_slice(&(pixel_bytes as u32).to_le_bytes());

    let mut bgra = vec![0u8; pixel_bytes];
    for row in 0..height as usize {
        let src_row = height as usize - 1 - row; // write bottom-up
        for px in 0..width as usize {
            let s = (src_row * width as usize + px) * 4;
            let d = (row * width as usize + px) * 4;
            bgra[d] = rgba[s + 2];
            bgra[d + 1] = rgba[s + 1];
            bgra[d + 2] = rgba[s];
            bgra[d + 3] = rgba[s + 3];
        }
    }
    let mut f = std::fs::File::create(path)?;
    f.write_all(&header)?;
    f.write_all(&bgra)?;
    Ok(())
}

/// Pixel-by-pixel difference score: sum of absolute deltas across all channels.
/// Used to compare captured frames against baselines or detect motion.
pub fn diff_score(a: &[u8], b: &[u8]) -> u64 {
    a.iter().zip(b.iter()).map(|(&x, &y)| (x as i64 - y as i64).unsigned_abs()).sum()
}

// Per-symbol port verdicts for the dropped donor functions:
// .forge/brief-queue/DEAD-LEDGER.md, row [2026-08-25] visual_gate.

#[cfg(test)]
mod tests {
    use super::*;

    // [BOARD:VISUAL-GATE] --frames takes both spellings and never yields an empty plan:
    // a mis-typed spec captures the default strip instead of silently gating nothing.
    #[test]
    fn frames_spec_parses_both_shapes() {
        assert_eq!(parse_frames("0,8,16,24"), vec![0, 8, 16, 24]);
        assert_eq!(parse_frames("4@8"), vec![0, 8, 16, 24]);
        assert_eq!(parse_frames("3@2"), vec![0, 2, 4]);
        assert_eq!(parse_frames("garbage"), DEFAULT_FRAMES.to_vec());
        assert_eq!(parse_frames("0@8"), DEFAULT_FRAMES.to_vec());
    }

    // [BOARD:VISUAL-GATE] the ANIM route reaches parse_args with its frame plan intact,
    // and a still panel stays frame-less — frames are opt-in, not a default on every capture.
    #[test]
    fn anim_args_carry_frames_and_stills_do_not() {
        let a: Vec<String> =
            ["visual-gate", "ANIM", "--frames", "3@4"].iter().map(|s| s.to_string()).collect();
        let cfg = parse_args(&a).expect("parse");
        assert_eq!(cfg.panel, "ANIM");
        assert_eq!(cfg.frames, vec![0, 4, 8]);
        let b: Vec<String> = ["visual-gate", "PAINT"].iter().map(|s| s.to_string()).collect();
        assert!(parse_args(&b).expect("parse").frames.is_empty());
    }

    #[test]
    fn bmp_roundtrip() {
        use std::path::PathBuf;
        use std::fs;

        // Create a test image: 2x2 RGBA with distinct colors
        let mut rgba = vec![0u8; 2 * 2 * 4];
        rgba[0] = 255; rgba[1] = 0; rgba[2] = 0; rgba[3] = 255;     // Red
        rgba[4] = 0; rgba[5] = 255; rgba[6] = 0; rgba[7] = 255;     // Green
        rgba[8] = 0; rgba[9] = 0; rgba[10] = 255; rgba[11] = 255;   // Blue
        rgba[12] = 255; rgba[13] = 255; rgba[14] = 0; rgba[15] = 255; // Yellow

        let tmp = PathBuf::from(".forge/_scratch/test_bmp_roundtrip.bmp");
        if let Some(p) = tmp.parent() {
            let _ = fs::create_dir_all(p);
        }

        // Write and read back
        write_bmp(&tmp, &rgba, 2, 2).expect("write");
        let (rgba2, w, h) = read_bmp(&tmp).expect("read");
        assert_eq!(w, 2);
        assert_eq!(h, 2);
        assert_eq!(rgba, rgba2);

        let _ = fs::remove_file(&tmp);
    }

    #[test]
    fn diff_score_computes_correctly() {
        let a = vec![255u8, 0, 0, 255, 0, 255, 0, 255];
        let b = vec![0u8, 0, 0, 255, 0, 255, 0, 255];
        let diff = diff_score(&a, &b);
        assert_eq!(diff, 255); // first byte differs by 255, rest match
    }
}
