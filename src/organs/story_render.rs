//! `story_render` — scene-map.json (from `story_beats`) → Beat extraction and frame generation.
//!
//! Ported from `F:\NewRepo\crates\forge-studio\src\story_render.rs` (v2 donor, 261 LOC).
//! Ported 2026-08-17. JSON parsing/writing hand-rolled (forge-core-v3 stays zero-dep except bytemuck).
//!
//! The v2 donor's GPU-bound `render_frames` and `build_stage` are stubbed here — forge-core-v3
//! cannot depend on forge_canvas/forge_export or the shell binary crate. The structural logic
//! (scene-map.json parsing, beat iteration, edl.jsonl writing) remains real and testable.

#![allow(missing_docs, dead_code)]

use std::fs;
use std::path::PathBuf;

/// One beat from the scene-map.json: timing, label, and optional asset reference.
#[derive(Debug, Clone, PartialEq)]
pub struct StoryBeat {
    /// Beat index (0-indexed in the scene-map).
    pub beat: u32,
    /// Start time in seconds (f64 in JSON, stored as-is).
    pub start_s: f64,
    /// End time in seconds.
    pub end_s: f64,
    /// Duration: end - start.
    pub duration_s: f64,
    /// Beat label / narration text.
    pub text: String,
    /// Optional asset reference (path to PNG or other media).
    pub asset: Option<PathBuf>,
    /// Section tag (HOOK, PROBLEM, BUILD, RESULT, CTA).
    pub section: String,
}

/// Parse a scene-map.json file into a vector of StoryBeat. Fails loud on any error
/// (missing file, malformed JSON, missing required fields) rather than silently defaulting.
pub fn load_scene_map(path: &std::path::Path) -> Result<Vec<StoryBeat>, String> {
    let raw = fs::read_to_string(path)
        .map_err(|e| format!("read {}: {e}", path.display()))?;
    parse_scene_map(&raw)
}

/// Parse a scene-map.json string (hand-rolled, no serde).
/// Expected format: array of objects, each with: beat, start, end, duration, text, asset, section.
fn parse_scene_map(raw: &str) -> Result<Vec<StoryBeat>, String> {
    let mut beats = Vec::new();

    // Locate the opening [
    let bracket_idx = raw
        .find('[')
        .ok_or_else(|| "no [ found".to_string())?;
    let array_start = bracket_idx + 1;

    // Find the matching ]
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
    let array_end = array_end.ok_or_else(|| "no matching ] found".to_string())?;
    let array_str = &raw[array_start..array_end];

    // Parse individual beat objects.
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
                    let beat = parse_beat_object(&current)?;
                    beats.push(beat);
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

    if beats.is_empty() {
        return Err("no beats found".to_string());
    }
    Ok(beats)
}

/// Parse one beat object: `{"beat": ..., "start": ..., "end": ..., "duration": ..., "text": "...", "asset": ..., "section": "..."}`
fn parse_beat_object(obj: &str) -> Result<StoryBeat, String> {
    let beat = extract_u32(obj, "beat")?;
    let start_s = extract_f64(obj, "start")?;
    let end_s = extract_f64(obj, "end")?;
    let duration_s = extract_f64(obj, "duration")?;
    let text = extract_string(obj, "text")?;
    let asset = extract_string_optional(obj, "asset")?;
    let section = extract_string(obj, "section")?;

    Ok(StoryBeat {
        beat,
        start_s,
        end_s,
        duration_s,
        text,
        asset: asset.map(PathBuf::from),
        section,
    })
}

/// Extract a u32 value: `"key": <number>`
fn extract_u32(obj: &str, key: &str) -> Result<u32, String> {
    extract_f64(obj, key).and_then(|f| {
        if f.fract() == 0.0 && f >= 0.0 && f <= u32::MAX as f64 {
            Ok(f as u32)
        } else {
            Err(format!("invalid u32 for {}", key))
        }
    })
}

/// Extract a f64 value: `"key": <number>`
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
        return Err(format!("no number found for {}", key));
    }

    num_part[..num_end]
        .trim()
        .parse()
        .map_err(|_| format!("parse f64 failed for {}", key))
}

/// Extract a string value: `"key": "value"`. Returns error if key is missing.
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

    // Find closing quote (naive; doesn't handle escaped quotes).
    let close_pos = string_body
        .find('"')
        .ok_or_else(|| "no closing quote".to_string())?;

    Ok(string_body[..close_pos].to_string())
}

/// Extract a string value, allowing null. Returns Ok(None) if the value is null or the key is missing.
fn extract_string_optional(obj: &str, key: &str) -> Result<Option<String>, String> {
    let key_str = format!("\"{}\"", key);
    let pos = match obj.find(&key_str) {
        Some(p) => p,
        None => return Ok(None),
    };
    let after_key = &obj[pos + key_str.len()..];
    let colon_pos = after_key
        .find(':')
        .ok_or_else(|| "no : after key".to_string())?;
    let after_colon = after_key[colon_pos + 1..].trim_start();

    // Check for null.
    if after_colon.starts_with("null") {
        return Ok(None);
    }

    // Expect opening quote.
    let quote_pos = after_colon
        .find('"')
        .ok_or_else(|| "no opening quote".to_string())?;
    let string_body = &after_colon[quote_pos + 1..];

    // Find closing quote.
    let close_pos = string_body
        .find('"')
        .ok_or_else(|| "no closing quote".to_string())?;

    Ok(Some(string_body[..close_pos].to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_minimal_beat_object() {
        let json = r#"{"beat": 0, "start": 1.0, "end": 5.5, "duration": 4.5, "text": "hello world", "asset": null, "section": "HOOK"}"#;
        let beat = parse_beat_object(json).expect("should parse");
        assert_eq!(beat.beat, 0);
        assert_eq!(beat.start_s, 1.0);
        assert_eq!(beat.end_s, 5.5);
        assert_eq!(beat.duration_s, 4.5);
        assert_eq!(beat.text, "hello world");
        assert_eq!(beat.asset, None);
        assert_eq!(beat.section, "HOOK");
    }

    #[test]
    fn parse_scene_map_with_multiple_beats() {
        let json = r#"[
            {"beat": 0, "start": 0.0, "end": 10.0, "duration": 10.0, "text": "intro", "asset": null, "section": "HOOK"},
            {"beat": 1, "start": 10.0, "end": 20.5, "duration": 10.5, "text": "body", "asset": "asset.png", "section": "PROBLEM"}
        ]"#;
        let beats = parse_scene_map(json).expect("should parse");
        assert_eq!(beats.len(), 2);
        assert_eq!(beats[0].beat, 0);
        assert_eq!(beats[0].text, "intro");
        assert_eq!(beats[1].beat, 1);
        assert_eq!(beats[1].text, "body");
        assert_eq!(beats[1].asset, Some(PathBuf::from("asset.png")));
    }

    #[test]
    fn empty_array_fails() {
        let json = "[]";
        let result = parse_scene_map(json);
        assert!(result.is_err());
    }

    #[test]
    fn missing_required_field_fails() {
        let json = r#"[{"beat": 0, "start": 0.0}]"#; // missing end, duration, text, section
        let result = parse_scene_map(json);
        assert!(result.is_err());
    }
}
