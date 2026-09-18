//! forge-scan — Windows-native portfolio scanner (lifted to lib for the ONE-BIN fold, 2026-07-09).
//!
//! Reads F:\repos\.kiro\portfolio.toml for repo list.
//! Walks each repo for .rs file timestamps → velocity.
//! Reads Cargo.toml for workspace crate counts.
//! Outputs JSON to stdout.
//!
//! Ported 2026-08-17 from F:\NewRepo\crates\forge-studio\src\forge_scan.rs.
//! Adaptations: hand-rolled TOML/JSON (no serde/toml deps); bounded work-list walk (depth cap 12).
//!
//! Usage:
//!   forge-scan                          # scan F:\repos
//!   forge-scan --repos-dir D:\other     # scan different root
//!   forge-scan --json                   # machine-readable (default)
//!   forge-scan --human                  # human-readable table

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, Duration, UNIX_EPOCH};

/// Scan the portfolio and print the report. `args` is the full argv (`args[0]` = program
/// name); flags (`--repos-dir`, `--human`) are searched across the whole vec, so the
/// standalone bin and the folded `13forge-studio scan` share one code path.
///
/// Returns 0 on success, nonzero on failure.
pub fn run(args: &[String]) -> i32 {
    let repos_dir = args
        .iter()
        .position(|a| a == "--repos-dir")
        .and_then(|i| args.get(i + 1))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"F:\repos"));
    let human = args.iter().any(|a| a == "--human");

    let portfolio_path = repos_dir.join(".kiro").join("portfolio.toml");
    let portfolio = load_portfolio(&portfolio_path);
    let now = SystemTime::now();

    let mut results: Vec<RepoScan> = Vec::new();

    // Scan repos from portfolio.toml
    if let Some(repos) = &portfolio {
        for (name, entry) in &repos.repos {
            let repo_path = repos_dir.join(name);
            if !repo_path.exists() {
                continue;
            }
            results.push(scan_repo(&repo_path, name, &entry.status, now));
        }
    }

    // Also scan any directories in repos_dir not in portfolio
    if let Ok(entries) = std::fs::read_dir(&repos_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let name = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            if name.starts_with('.') {
                continue;
            }
            if results.iter().any(|r| r.name == name) {
                continue;
            }
            // Check if it's a Rust project
            if path.join("Cargo.toml").exists() {
                results.push(scan_repo(&path, &name, "UNREGISTERED", now));
            }
        }
    }

    // Sort by last_modified descending (most active first)
    results.sort_by(|a, b| b.last_modified_secs.cmp(&a.last_modified_secs));

    if human {
        print_human(&results, now);
    } else {
        print_json(&results, &repos_dir, now);
    }

    0
}

// ── Types ────────────────────────────────────────────────────────────────

/// Output of a repository scan.
#[derive(Clone, Debug)]
struct RepoScan {
    /// Repository name.
    name: String,
    /// Status from portfolio or "UNREGISTERED".
    status: String,
    /// Absolute path to repo.
    path: String,
    /// ISO-ish formatted timestamp of last .rs file modification.
    last_modified: String,
    /// Unix timestamp (seconds) of last modification.
    last_modified_secs: u64,
    /// Total .rs files in repo.
    rs_files_total: u32,
    /// .rs files modified in last 7 days.
    rs_files_7d: u32,
    /// .rs files modified in last 30 days.
    rs_files_30d: u32,
    /// Count of workspace members in Cargo.toml.
    crate_count: u32,
    /// Names of workspace members.
    workspace_members: Vec<String>,
    /// Top 5 most recently modified .rs files.
    most_recent_files: Vec<RecentFile>,
}

/// A recently modified .rs file within a repo.
#[derive(Clone, Debug)]
struct RecentFile {
    /// Relative path from repo root.
    path: String,
    /// ISO-ish formatted timestamp.
    modified: String,
    /// Age in hours.
    age_hours: u64,
}

// ── Portfolio TOML (hand-rolled) ─────────────────────────────────────────

/// Minimal TOML portfolio structure.
struct Portfolio {
    /// Map of repo name → repo entry.
    repos: HashMap<String, RepoEntry>,
}

/// A single repo entry in portfolio.toml.
struct RepoEntry {
    /// Status line (e.g., "ACTIVE", "ARCHIVED").
    status: String,
}

fn load_portfolio(path: &Path) -> Option<Portfolio> {
    let text = std::fs::read_to_string(path).ok()?;
    parse_toml_portfolio(&text)
}

fn parse_toml_portfolio(text: &str) -> Option<Portfolio> {
    let mut repos = HashMap::new();
    let mut current_section: Option<String> = None;

    for line in text.lines() {
        let trimmed = line.trim();

        // Skip comments and empty lines
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        // Section header: [repos] or [repos.name]
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            let section_name = &trimmed[1..trimmed.len() - 1];
            if section_name == "repos" {
                current_section = Some("repos".to_string());
            } else if section_name.starts_with("repos.") {
                let repo_name = &section_name[6..]; // Skip "repos."
                current_section = Some(repo_name.to_string());
                repos.insert(
                    repo_name.to_string(),
                    RepoEntry {
                        status: "UNKNOWN".to_string(),
                    },
                );
            }
            continue;
        }

        // Key=value within a section
        if let Some(ref section) = current_section {
            if section != "repos" {
                if let Some(eq_pos) = trimmed.find('=') {
                    let key = trimmed[..eq_pos].trim();
                    let value_str = trimmed[eq_pos + 1..].trim();
                    let value = unquote(value_str);

                    if key == "status" {
                        if let Some(entry) = repos.get_mut(section) {
                            entry.status = value;
                        }
                    }
                }
            }
        }
    }

    Some(Portfolio { repos })
}

fn unquote(s: &str) -> String {
    if (s.starts_with('"') && s.ends_with('"')) || (s.starts_with('\'') && s.ends_with('\'')) {
        s[1..s.len() - 1].to_string()
    } else {
        s.to_string()
    }
}

// ── Scanner ──────────────────────────────────────────────────────────────

fn scan_repo(path: &Path, name: &str, status: &str, now: SystemTime) -> RepoScan {
    let seven_days = Duration::from_secs(7 * 86400);
    let thirty_days = Duration::from_secs(30 * 86400);

    let mut rs_total = 0u32;
    let mut rs_7d = 0u32;
    let mut rs_30d = 0u32;
    let mut latest = UNIX_EPOCH;
    let mut recent: Vec<(PathBuf, SystemTime)> = Vec::new();

    walk_rs_files_bounded(path, &mut |file_path, modified| {
        rs_total += 1;
        if let Ok(age) = now.duration_since(modified) {
            if age < seven_days {
                rs_7d += 1;
            }
            if age < thirty_days {
                rs_30d += 1;
            }
        }
        if modified > latest {
            latest = modified;
        }
        recent.push((file_path.to_path_buf(), modified));
    });

    // Sort recent by time, take top 5
    recent.sort_by(|a, b| b.1.cmp(&a.1));
    recent.truncate(5);

    let most_recent_files: Vec<RecentFile> = recent
        .iter()
        .map(|(p, m)| {
            let rel = p.strip_prefix(path).unwrap_or(p);
            let age_hrs = now
                .duration_since(*m)
                .map(|d| d.as_secs() / 3600)
                .unwrap_or(0);
            RecentFile {
                path: rel.to_string_lossy().to_string(),
                modified: format_time(*m),
                age_hours: age_hrs,
            }
        })
        .collect();

    // Read workspace members from Cargo.toml
    let (crate_count, members) = read_workspace_members(&path.join("Cargo.toml"));

    let last_secs = latest
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    RepoScan {
        name: name.to_string(),
        status: status.to_string(),
        path: path.to_string_lossy().to_string(),
        last_modified: format_time(latest),
        last_modified_secs: last_secs,
        rs_files_total: rs_total,
        rs_files_7d: rs_7d,
        rs_files_30d: rs_30d,
        crate_count,
        workspace_members: members,
        most_recent_files,
    }
}

fn walk_rs_files_bounded(dir: &Path, cb: &mut dyn FnMut(&Path, SystemTime)) {
    // Bounded work-list to avoid deep recursion. Depth cap ~12.
    const MAX_DEPTH: usize = 12;

    let mut work: Vec<(PathBuf, usize)> = vec![(dir.to_path_buf(), 0)];

    while let Some((path, depth)) = work.pop() {
        if depth > MAX_DEPTH {
            continue;
        }

        let Ok(entries) = std::fs::read_dir(&path) else {
            continue;
        };

        for entry in entries.flatten() {
            let entry_path = entry.path();
            let name = entry_path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy();

            // Skip build artifacts and deps
            if name == "target" || name == "node_modules" || name == ".git" || name == "D:" {
                continue;
            }

            if entry_path.is_dir() {
                work.push((entry_path, depth + 1));
            } else if entry_path.extension().is_some_and(|e| e == "rs") {
                if let Ok(meta) = entry_path.metadata() {
                    if let Ok(modified) = meta.modified() {
                        cb(&entry_path, modified);
                    }
                }
            }
        }
    }
}

fn read_workspace_members(cargo_path: &Path) -> (u32, Vec<String>) {
    let Ok(text) = std::fs::read_to_string(cargo_path) else {
        return (0, vec![]);
    };

    // Hand-rolled TOML parsing for [workspace] members array
    let mut in_workspace = false;
    let mut members: Vec<String> = Vec::new();

    for line in text.lines() {
        let trimmed = line.trim();

        if trimmed == "[workspace]" {
            in_workspace = true;
            continue;
        }

        if trimmed.starts_with('[') {
            in_workspace = false;
            continue;
        }

        if in_workspace && trimmed.starts_with("members") {
            // Parse: members = [ "crates/foo", "crates/bar" ]
            if let Some(start) = trimmed.find('[') {
                if let Some(end) = trimmed.find(']') {
                    let array_str = &trimmed[start + 1..end];
                    for item in array_str.split(',') {
                        let item = item.trim();
                        let member = unquote(item);
                        if !member.is_empty() {
                            members.push(member);
                        }
                    }
                }
            }
        }
    }

    (members.len() as u32, members)
}

fn format_time(t: SystemTime) -> String {
    let secs = t.duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    if secs == 0 {
        return "never".to_string();
    }
    // Simple ISO-ish format without chrono dep
    let days_since_epoch = secs / 86400;
    let time_of_day = secs % 86400;
    let hours = time_of_day / 3600;
    let mins = (time_of_day % 3600) / 60;
    // Approximate date (good enough for display)
    let y = 1970 + (days_since_epoch / 365);
    let d = days_since_epoch % 365;
    let m = d / 30 + 1;
    let day = d % 30 + 1;
    format!("{y}-{m:02}-{day:02} {hours:02}:{mins:02}Z")
}

// ── Hand-rolled JSON output ──────────────────────────────────────────────

fn print_json(repos: &[RepoScan], repos_dir: &Path, now: SystemTime) {
    let scan_time = format_time(now);
    let repos_dir_str = repos_dir.to_string_lossy();

    // Hand-rolled JSON to avoid serde dependency
    println!("{{");
    println!(r#"  "scan_time": "{}","#, json_escape(&scan_time));
    println!(r#"  "repos_dir": "{}","#, json_escape(&repos_dir_str));
    println!(r#"  "repo_count": {},"#, repos.len());
    println!(r#"  "repos": ["#);

    for (i, r) in repos.iter().enumerate() {
        println!("    {{");
        println!(r#"      "name": "{}","#, json_escape(&r.name));
        println!(r#"      "status": "{}","#, json_escape(&r.status));
        println!(r#"      "path": "{}","#, json_escape(&r.path));
        println!(r#"      "last_modified": "{}","#, json_escape(&r.last_modified));
        println!(r#"      "last_modified_secs": {},"#, r.last_modified_secs);
        println!(r#"      "rs_files_total": {},"#, r.rs_files_total);
        println!(r#"      "rs_files_7d": {},"#, r.rs_files_7d);
        println!(r#"      "rs_files_30d": {},"#, r.rs_files_30d);
        println!(r#"      "crate_count": {},"#, r.crate_count);

        println!(r#"      "workspace_members": ["#);
        for (j, member) in r.workspace_members.iter().enumerate() {
            if j < r.workspace_members.len() - 1 {
                println!(r#"        "{}","#, json_escape(member));
            } else {
                println!(r#"        "{}""#, json_escape(member));
            }
        }
        println!(r#"      ],"#);

        println!(r#"      "most_recent_files": ["#);
        for (j, rf) in r.most_recent_files.iter().enumerate() {
            println!("        {{");
            println!(r#"          "path": "{}","#, json_escape(&rf.path));
            println!(r#"          "modified": "{}","#, json_escape(&rf.modified));
            println!(r#"          "age_hours": {}"#, rf.age_hours);
            if j < r.most_recent_files.len() - 1 {
                println!("        }},");
            } else {
                println!("        }}");
            }
        }
        println!(r#"      ]"#);

        if i < repos.len() - 1 {
            println!("    }},");
        } else {
            println!("    }}");
        }
    }

    println!("  ]");
    println!("}}");
}

fn json_escape(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => result.push_str(r#"\""#),
            '\\' => result.push_str(r"\\"),
            '\n' => result.push_str(r"\n"),
            '\r' => result.push_str(r"\r"),
            '\t' => result.push_str(r"\t"),
            _ => result.push(c),
        }
    }
    result
}

fn print_human(repos: &[RepoScan], now: SystemTime) {
    eprintln!(
        "{:<25} {:<12} {:>6} {:>5} {:>5} {:>6}  LAST MODIFIED",
        "REPO", "STATUS", ".rs", "7d", "30d", "CRATES"
    );
    eprintln!("{}", "-".repeat(90));
    for r in repos {
        let age = if r.last_modified_secs > 0 {
            let secs = now
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0)
                - r.last_modified_secs;
            if secs < 3600 {
                format!("{}m ago", secs / 60)
            } else if secs < 86400 {
                format!("{}h ago", secs / 3600)
            } else {
                format!("{}d ago", secs / 86400)
            }
        } else {
            "never".to_string()
        };
        eprintln!(
            "{:<25} {:<12} {:>6} {:>5} {:>5} {:>6}  {}",
            r.name, r.status, r.rs_files_total, r.rs_files_7d, r.rs_files_30d, r.crate_count, age
        );
    }
}
