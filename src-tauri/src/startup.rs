#[cfg(target_os = "windows")]
const TASK_NAME: &str = "XscapeNetAutoStart";

#[cfg(target_os = "windows")]
fn run_schtasks(args: &[&str]) -> bool {
    use std::os::windows::process::CommandExt;
    use std::process::Command;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    Command::new("schtasks")
        .args(args)
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

#[cfg(target_os = "windows")]
fn is_hkcu_run_enabled() -> bool {
    use windows::core::PCWSTR;
    use windows::Win32::System::Registry::{
        RegCloseKey, RegOpenKeyExW, RegQueryValueExW, HKEY, HKEY_CURRENT_USER, KEY_READ,
    };

    let subkey: Vec<u16> = "Software\\Microsoft\\Windows\\CurrentVersion\\Run"
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let val_name: Vec<u16> = "XscapeNet"
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();

    unsafe {
        let mut hkey = HKEY::default();
        if RegOpenKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(subkey.as_ptr()),
            0,
            KEY_READ,
            &mut hkey,
        )
        .is_err()
        {
            return false;
        }
        let mut size = 0u32;
        let exists = RegQueryValueExW(
            hkey,
            PCWSTR(val_name.as_ptr()),
            None,
            None,
            None,
            Some(&mut size),
        )
        .is_ok()
            && size > 0;
        let _ = RegCloseKey(hkey);
        exists
    }
}

#[cfg(target_os = "windows")]
pub fn is_run_on_startup_enabled() -> bool {
    if run_schtasks(&["/Query", "/TN", TASK_NAME]) {
        return true;
    }
    is_hkcu_run_enabled()
}

/// Configures Windows startup reliably for both elevated (Task Scheduler `/RL HIGHEST`)
/// and standard user sessions (`HKCU\...\Run`). Always cleans both when disabled.
#[cfg(target_os = "windows")]
pub fn set_run_on_startup(enable: bool) -> Result<(), String> {
    use windows::core::PCWSTR;
    use windows::Win32::System::Registry::{
        RegCloseKey, RegCreateKeyExW, RegDeleteValueW, RegSetValueExW, HKEY, HKEY_CURRENT_USER,
        KEY_SET_VALUE, REG_OPTION_NON_VOLATILE, REG_SZ,
    };

    let subkey: Vec<u16> = "Software\\Microsoft\\Windows\\CurrentVersion\\Run"
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let val_name: Vec<u16> = "XscapeNet"
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();

    if !enable {
        let _ = run_schtasks(&["/Delete", "/TN", TASK_NAME, "/F"]);
        unsafe {
            let mut hkey = HKEY::default();
            if RegCreateKeyExW(
                HKEY_CURRENT_USER,
                PCWSTR(subkey.as_ptr()),
                0,
                PCWSTR::null(),
                REG_OPTION_NON_VOLATILE,
                KEY_SET_VALUE,
                None,
                &mut hkey,
                None,
            )
            .is_ok()
            {
                let _ = RegDeleteValueW(hkey, PCWSTR(val_name.as_ptr()));
                let _ = RegCloseKey(hkey);
            }
        }
        return Ok(());
    }

    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let cmd = format!("\"{}\"", exe.to_string_lossy());

    // First try Task Scheduler with /RL HIGHEST (works seamlessly for elevated Admin/ETW mode at logon)
    if run_schtasks(&[
        "/Create",
        "/TN",
        TASK_NAME,
        "/SC",
        "ONLOGON",
        "/RL",
        "HIGHEST",
        "/TR",
        &cmd,
        "/F",
    ]) {
        return Ok(());
    }

    // Fallback for non-admin sessions: HKCU\Software\Microsoft\Windows\CurrentVersion\Run
    unsafe {
        let mut hkey = HKEY::default();
        RegCreateKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(subkey.as_ptr()),
            0,
            PCWSTR::null(),
            REG_OPTION_NON_VOLATILE,
            KEY_SET_VALUE,
            None,
            &mut hkey,
            None,
        )
        .ok()
        .map_err(|e| format!("Failed to open Run registry key: {}", e))?;

        let utf16: Vec<u16> = cmd.encode_utf16().chain(std::iter::once(0)).collect();
        let bytes = std::slice::from_raw_parts(
            utf16.as_ptr() as *const u8,
            utf16.len() * std::mem::size_of::<u16>(),
        );
        let res = RegSetValueExW(hkey, PCWSTR(val_name.as_ptr()), 0, REG_SZ, Some(bytes))
            .ok()
            .map_err(|e| format!("Failed to write startup entry: {}", e));
        let _ = RegCloseKey(hkey);
        res
    }
}

#[cfg(not(target_os = "windows"))]
pub fn is_run_on_startup_enabled() -> bool {
    false
}

#[cfg(not(target_os = "windows"))]
pub fn set_run_on_startup(_enable: bool) -> Result<(), String> {
    Ok(())
}
