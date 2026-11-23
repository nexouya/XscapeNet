mod collector_fallback;
mod collector_win;
mod commands;
mod firewall;
mod ip_info;
mod models;
mod startup;
mod storage;
mod tray;

use std::sync::Arc;
use tauri::window::Color;
use tauri::{Manager, WindowEvent};

#[cfg(target_os = "windows")]
fn apply_clean_windows_chrome(win: &tauri::WebviewWindow, is_popover: bool) {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::Graphics::Dwm::{
        DwmSetWindowAttribute, DWMWA_BORDER_COLOR, DWMWA_USE_IMMERSIVE_DARK_MODE,
        DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND,
    };
    use windows::Win32::Graphics::Gdi::{GetStockObject, BLACK_BRUSH};
    use windows::Win32::UI::WindowsAndMessaging::{
        GetWindowLongPtrW, SetClassLongPtrW, SetWindowLongPtrW, SetWindowPos, GCLP_HBRBACKGROUND,
        GWL_STYLE, SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER,
        WS_BORDER, WS_CAPTION, WS_DLGFRAME, WS_THICKFRAME,
    };

    if let Ok(hwnd_raw) = win.hwnd() {
        let hwnd = HWND(hwnd_raw.0);
        unsafe {
            // Paint Win32 class background pure black (never white!)
            let black_brush = GetStockObject(BLACK_BRUSH);
            let _ = SetClassLongPtrW(hwnd, GCLP_HBRBACKGROUND, black_brush.0 as isize);

            // On the non-resizable Popover, strip every Win32 border/frame style completely
            // so Windows 11 NEVER paints the 6px white non-client border around it!
            if is_popover {
                let style = GetWindowLongPtrW(hwnd, GWL_STYLE) as u32;
                let strip = WS_THICKFRAME.0 | WS_CAPTION.0 | WS_BORDER.0 | WS_DLGFRAME.0;
                let new_style = style & !strip;
                let _ = SetWindowLongPtrW(hwnd, GWL_STYLE, new_style as isize);
                let _ = SetWindowPos(
                    hwnd,
                    None,
                    0,
                    0,
                    0,
                    0,
                    SWP_FRAMECHANGED | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
                );
            }

            let dark: u32 = 1;
            let _ = DwmSetWindowAttribute(
                hwnd,
                DWMWA_USE_IMMERSIVE_DARK_MODE,
                &dark as *const _ as *const _,
                std::mem::size_of_val(&dark) as u32,
            );
            let corner_pref = DWMWCP_ROUND;
            let _ = DwmSetWindowAttribute(
                hwnd,
                DWMWA_WINDOW_CORNER_PREFERENCE,
                &corner_pref as *const _ as *const _,
                std::mem::size_of_val(&corner_pref) as u32,
            );
            // DWMWA_COLOR_NONE (0xFFFFFFFE): completely suppress Windows 11 DWM border
            let no_border: u32 = 0xFFFF_FFFE;
            let _ = DwmSetWindowAttribute(
                hwnd,
                DWMWA_BORDER_COLOR,
                &no_border as *const _ as *const _,
                std::mem::size_of_val(&no_border) as u32,
            );
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let state = Arc::new(
        commands::AppState::new().expect("Failed to initialize XscapeNet storage and telemetry"),
    );

    tauri::Builder::default()
        .manage(state.clone())
        .setup(move |app| {
            tray::setup_tray(app.handle())?;

            for label in ["popover", "advanced"] {
                if let Some(win) = app.get_webview_window(label) {
                    let _ = win.set_background_color(Some(Color(0, 0, 0, 255)));
                    #[cfg(target_os = "windows")]
                    apply_clean_windows_chrome(&win, label == "popover");
                }
            }

            if let Some(pop) = app.get_webview_window("popover") {
                let pop_clone = pop.clone();
                pop.on_window_event(move |event| {
                    if let WindowEvent::Focused(false) = event {
                        let _ = pop_clone.hide();
                    }
                });
            }

            if let Some(adv) = app.get_webview_window("advanced") {
                let adv_clone = adv.clone();
                let state_for_close = state.clone();
                adv.on_window_event(move |event| {
                    if let WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        state_for_close.storage.checkpoint_wal();
                        let _ = adv_clone.hide();
                    }
                });
            }

            commands::start_telemetry_loop(app.handle().clone(), state.clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_live_snapshot,
            commands::get_analytics,
            commands::refresh_ip,
            commands::get_settings,
            commands::update_settings,
            commands::get_blocked_apps,
            commands::toggle_app_block,
            commands::open_advanced_window,
            commands::hide_popover,
            commands::clear_history,
            commands::get_collector_mode,
            commands::relaunch_as_admin,
        ])
        .run(tauri::generate_context!())
        .expect("error while running XscapeNet");
}
