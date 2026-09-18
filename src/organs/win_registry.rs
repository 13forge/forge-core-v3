//! Windows "Open With" registration for 13forge-studio.exe.
//!
//! Writes idempotent HKCU keys so Explorer's right-click → Open With menu
//! lists 13forge-studio as a candidate for PNG / GLB / image files.
//!
//! Ported 2026-08-17 from F:\NewRepo\crates\forge-studio\src\win_registry.rs.
//!
//! All writes are HKCU (user-scope) — never HKLM. No elevation required.
//! All errors are logged-only — Studio still launches on registry failure.
//! Idempotent: `reg add` opens-or-creates and overwrites.

#![cfg(target_os = "windows")]

use std::process::Command;

/// Register 13forge-studio.exe with the user's "Open With" menu.
/// Safe to call on every launch — idempotent.
pub fn register_open_with() {
    let exe = match std::env::current_exe() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("[win_registry] current_exe failed: {e}");
            return;
        }
    };
    let exe_str = exe.to_string_lossy().into_owned();
    let exe_name = exe
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("13forge-studio.exe")
        .to_owned();

    let app_base = format!(r"HKEY_CURRENT_USER\Software\Classes\Applications\{exe_name}");

    // 1. FriendlyAppName — what Explorer shows in the Open With list.
    reg_add(&app_base, "FriendlyAppName", "13Forge Studio");

    // 2. shell\open\command — the launch command Explorer invokes.
    let cmd_key = format!(r"{app_base}\shell\open\command");
    let cmd_value = format!(r#""{exe_str}" "%1""#);
    reg_add(&cmd_key, "", &cmd_value);

    // 3. SupportedTypes — extensions this app handles. Empty string values
    //    are the documented marker; the *value name* is the extension.
    let supported = format!(r"{app_base}\SupportedTypes");
    for ext in [".glb", ".png", ".jpg", ".jpeg", ".webp"] {
        reg_add(&supported, ext, "");
    }
}

fn reg_add(key: &str, value_name: &str, value: &str) {
    let mut args = vec!["add".to_string(), key.to_string()];

    if !value_name.is_empty() {
        args.push("/v".to_string());
        args.push(value_name.to_string());
    }

    args.push("/t".to_string());
    args.push("REG_SZ".to_string());
    args.push("/d".to_string());
    args.push(value.to_string());
    args.push("/f".to_string()); // Force overwrite, no prompt

    match Command::new("reg.exe").args(&args).output() {
        Ok(output) => {
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                eprintln!("[win_registry] reg add {} failed: {}", key, stderr);
            }
        }
        Err(e) => {
            eprintln!("[win_registry] reg.exe failed: {e}");
        }
    }
}
