//! `story-beats` — whisper transcript JSON -> canonical scene-map.json
//! (the exact shape `story_render::load_scene_map` eats). Sections are
//! assigned mechanically from the source-compiler time constants.
//!
//! Ported from `F:\NewRepo\crates\forge-studio\src\story_beats.rs` (v2 donor).
//! JSON parsing/writing hand-rolled (forge-core-v3 stays zero-dep except bytemuck).

use std::fmt::Write as _;
use std::fs;
use std::path::PathBuf;

/// Represents one segment from the Whisper JSON.
#[derive(Debug, Clone)]
struct WhisperSegment {
    start: f64,
    end: f64,
    text: String,
}

/// Parse a minimal Whisper JSON format (no serde — hand-rolled).
/// Expected shape: `{"segments": [{"start": X, "end": Y, "text": "..."}, ...]}`
fn parse_whisper_json(raw: &str) -> Result<Vec<WhisperSegment>, String> {
    let mut segments = Vec::new();

    // Locate the segments array.
    let segments_key = "\"segments\"";
    let seg_idx = raw
        .find(segments_key)
        .ok_or_else(|| "no \"segments\" key".to_string())?;
    let after_key = &raw[seg_idx + segments_key.len()..];
    let bracket_idx = after_key
        .find('[')
        .ok_or_else(|| "no [ after \"segments\"".to_string())?;
    let array_start = seg_idx + segments_key.len() + bracket_idx + 1;

    // Find the matching ].
    let mut depth = 0;
    let mut array_end = None;
    for (i, ch) in raw[array_start..].chars().enumerate() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
            }
            ']' if depth == 0 => {
                array_end = Some(array_start + i);
                break;
            }
            _ => {}
        }
    }
    let array_end = array_end.ok_or_else(|| "no matching ]".to_string())?;
    let array_str = &raw[array_start..array_end];

    // Parse individual segment objects.
    let mut in_obj = false;
    let mut depth = 0;
    let mut current = String::new();
    for ch in array_str.chars() {
        match ch {
            '{' => {
                in_obj = true;
                depth += 1;
                current.push(ch);
            }
            '}' => {
                current.push(ch);
                depth -= 1;
                if in_obj && depth == 0 {
                    let seg = parse_segment(&current)?;
                    segments.push(seg);
                    current.clear();
                    in_obj = false;
                }
            }
            _ => {
                if in_obj {
                    current.push(ch);
                }
            }
        }
    }

    Ok(segments)
}

/// Parse one segment JSON object: `{"start": ..., "end": ..., "text": "..."}`
fn parse_segment(obj: &str) -> Result<WhisperSegment, String> {
    let start = extract_f64(obj, "start")?;
    let end = extract_f64(obj, "end")?;
    let text = extract_string(obj, "text")?;

    Ok(WhisperSegment {
        start,
        end,
        text: text.trim().to_string(),
    })
}

/// Extract a f64 value from key: `"key": <number>`
fn extract_f64(obj: &str, key: &str) -> Result<f64, String> {
    let key_str = format!("\"{}\"", key);
    let pos = obj
        .find(&key_str)
        .ok_or_else(|| format!("no key \"{}\"", key))?;
    let after_key = &obj[pos + key_str.len()..];
    let colon_pos = after_key
        .find(':')
        .ok_or_else(|| "no : after key".to_string())?;
    let num_part = after_key[colon_pos + 1..].trim_start();

    // Find the end of the number (comma or closing brace).
    let mut num_end = 0;
    for (i, ch) in num_part.chars().enumerate() {
        if ch == ',' || ch == '}' {
            num_end = i;
            break;
        }
    }
    if num_end == 0 {
        return Err("no number found".to_string());
    }

    num_part[..num_end]
        .trim()
        .parse()
        .map_err(|_| format!("parse f64 failed for {}", key))
}

/// Extract a string value from key: `"key": "value"`
fn extract_string(obj: &str, key: &str) -> Result<String, String> {
    let key_str = format!("\"{}\"", key);
    let pos = obj
        .find(&key_str)
        .ok_or_else(|| format!("no key \"{}\"", key))?;
    let after_key = &obj[pos + key_str.len()..];
    let colon_pos = after_key
        .find(':')
        .ok_or_else(|| "no : after key".to_string())?;
    let after_colon = after_key[colon_pos + 1..].trim_start();

    // Expect opening quote.
    let quote_pos = after_colon
        .find('"')
        .ok_or_else(|| "no opening quote".to_string())?;
    let string_body = &after_colon[quote_pos + 1..];

    // Find closing quote (naive; doesn't handle escaped quotes, but Whisper output is simple).
    let close_pos = string_body
        .find('"')
        .ok_or_else(|| "no closing quote".to_string())?;

    Ok(string_body[..close_pos].to_string())
}

/// HOOK 15s / PROBLEM 60s / CTA 30s / RESULT 60s-before-CTA, BUILD between —
/// mirrors `source_compiler` section budgets.
fn section_for(start: f64, total: f64) -> &'static str {
    if start < 15.0 {
        "HOOK"
    } else if start < 75.0 {
        "PROBLEM"
    } else if start >= total - 30.0 {
        "CTA"
    } else if start >= total - 90.0 {
        "RESULT"
    } else {
        "BUILD"
    }
}

fn print_help() {
    eprintln!(
        "story-beats — whisper transcript json -> scene-map.json for render-story.\n\
         \n\
         \x20 story-beats --whisper <whisper.json> --out <scene-map.json>\n\
         \x20   [--merge-under <secs>]   merge adjacent segments until each beat >= secs"
    );
}

/// Entry point matching the Organ signature: `fn(&[String]) -> i32`.
/// Args start at index 0 (no wrapper prefix here, unlike v2's shift by 2).
pub fn run(args: &[String]) -> i32 {
    let mut whisper: Option<PathBuf> = None;
    let mut out: Option<PathBuf> = None;
    let mut merge_under = 0.0f64;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--whisper" => {
                i += 1;
                whisper = args.get(i).map(PathBuf::from);
            }
            "--out" => {
                i += 1;
                out = args.get(i).map(PathBuf::from);
            }
            "--merge-under" => {
                i += 1;
                merge_under = args
                    .get(i)
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0.0);
            }
            "--help" | "-h" => {
                print_help();
                return 0;
            }
            other => {
                eprintln!("story-beats: unknown flag {other}");
                print_help();
                return 2;
            }
        }
        i += 1;
    }

    let (whisper, out) = match (whisper, out) {
        (Some(w), Some(o)) => (w, o),
        _ => {
            eprintln!("story-beats: --whisper and --out are required");
            print_help();
            return 2;
        }
    };

    let raw = match fs::read_to_string(&whisper) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("story-beats FAILED (read {}): {e}", whisper.display());
            return 1;
        }
    };

    let segments = match parse_whisper_json(&raw) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("story-beats FAILED (parse {}): {e}", whisper.display());
            return 1;
        }
    };

    if segments.is_empty() {
        eprintln!(
            "story-beats FAILED: {} contains zero segments",
            whisper.display()
        );
        return 1;
    }

    // Merge adjacent segments until each beat spans at least `merge_under` seconds.
    let mut merged: Vec<(f64, f64, String)> = Vec::new();
    for seg in &segments {
        let text = seg.text.trim();
        match merged.last_mut() {
            Some(last) if last.1 - last.0 < merge_under => {
                last.1 = seg.end;
                last.2.push(' ');
                last.2.push_str(text);
            }
            _ => merged.push((seg.start, seg.end, text.to_string())),
        }
    }

    let total = merged.last().map(|b| b.1).unwrap_or(0.0);

    // Hand-rolled JSON output: array of beat objects.
    let mut body = String::from("[\n");
    for (idx, (start, end, text)) in merged.iter().enumerate() {
        if idx > 0 {
            body.push_str(",\n");
        }
        let duration = end - start;
        let section = section_for(*start, total);

        // Escape text for JSON: backslash and double-quote.
        let text_escaped = text.replace('\\', "\\\\").replace('"', "\\\"");

        let _ = writeln!(
            body,
            "  {{\n    \"beat\": {},\n    \"start\": {},\n    \"end\": {},\n    \"duration\": {},\n    \"text\": \"{}\",\n    \"asset\": null,\n    \"section\": \"{}\"\n  }}",
            idx, start, end, duration, text_escaped, section
        );
    }
    body.push_str("\n]\n");

    if let Some(parent) = out.parent() {
        if let Err(e) = fs::create_dir_all(parent) {
            eprintln!("story-beats FAILED (mkdir {}): {e}", parent.display());
            return 1;
        }
    }
    if let Err(e) = fs::write(&out, body) {
        eprintln!("story-beats FAILED (write {}): {e}", out.display());
        return 1;
    }

    println!(
        "story-beats :: {} segments -> {} beats ({:.1}s) -> {}",
        segments.len(),
        merged.len(),
        total,
        out.display()
    );
    0
}
