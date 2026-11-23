use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

/// Core Windows OS & security processes that MUST NEVER be blocked to prevent breaking networking.
const PROTECTED_PROCESSES: &[&str] = &[
    "system",
    "system idle",
    "ntoskrnl.exe",
    "svchost.exe",
    "lsass.exe",
    "csrss.exe",
    "wininit.exe",
    "services.exe",
    "explorer.exe",
    "dwm.exe",
    "winlogon.exe",
    "smss.exe",
    "spoolsv.exe",
    "xscapenet.exe",
];

pub fn is_protected_process(process_name: &str) -> bool {
    let lower = process_name.trim().to_lowercase();
    PROTECTED_PROCESSES.iter().any(|&p| p == lower)
}

fn rule_names_for_path(exe_path: &str) -> (String, String) {
    let norm = exe_path.trim().to_lowercase();
    let mut hasher = DefaultHasher::new();
    norm.hash(&mut hasher);
    let h = hasher.finish();
    (
        format!("XscapeNet_Block_Out_{:016x}", h),
        format!("XscapeNet_Block_In_{:016x}", h),
    )
}

#[cfg(target_os = "windows")]
fn run_netsh(args: &[&str]) -> Result<String, String> {
    use std::os::windows::process::CommandExt;
    use std::process::Command;

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    let output = Command::new("netsh")
        .args(args)
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| format!("Failed to invoke netsh: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if output.status.success() {
        Ok(stdout)
    } else {
        let msg = if !stderr.is_empty() { stderr } else { stdout };
        if msg.to_lowercase().contains("elevation") || msg.to_lowercase().contains("administrator") {
            Err("Administrator privileges required. Click 'Enable Admin ETW' first.".into())
        } else {
            Err(if msg.is_empty() {
                "Windows Firewall command failed (requires Administrator).".into()
            } else {
                msg
            })
        }
    }
}

/// Safely blocks or unblocks a specific executable via Windows Defender Firewall rules.
/// Uses deterministic XscapeNet-prefixed rule names so unblocking always cleanly removes both rules.
#[cfg(target_os = "windows")]
pub fn set_app_blocked(process_name: &str, exe_path: &str, block: bool) -> Result<(), String> {
    if is_protected_process(process_name) {
        return Err(format!("'{}' is a protected Windows system process and cannot be blocked.", process_name));
    }

    let trimmed = exe_path.trim();
    if trimmed.is_empty() || !std::path::Path::new(trimmed).exists() {
        return Err("Executable path could not be verified on disk (run as Admin for full path resolution).".into());
    }

    let (rule_out, rule_in) = rule_names_for_path(trimmed);

    // Always delete any existing rules with the same deterministic names first (zero duplicates!)
    let _ = run_netsh(&["advfirewall", "firewall", "delete", "rule", &format!("name={}", rule_out)]);
    let _ = run_netsh(&["advfirewall", "firewall", "delete", "rule", &format!("name={}", rule_in)]);

    if !block {
        return Ok(());
    }

    let prog_arg = format!("program={}", trimmed);
    run_netsh(&[
        "advfirewall",
        "firewall",
        "add",
        "rule",
        &format!("name={}", rule_out),
        "dir=out",
        "action=block",
        &prog_arg,
        "profile=any",
        "enable=yes",
    ])?;

    run_netsh(&[
        "advfirewall",
        "firewall",
        "add",
        "rule",
        &format!("name={}", rule_in),
        "dir=in",
        "action=block",
        &prog_arg,
        "profile=any",
        "enable=yes",
    ])?;

    Ok(())
}

#[cfg(not(target_os = "windows"))]
pub fn set_app_blocked(process_name: &str, _exe_path: &str, _block: bool) -> Result<(), String> {
    if is_protected_process(process_name) {
        return Err("Protected system process cannot be blocked.".into());
    }
    Ok(())
}
