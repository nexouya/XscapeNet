use std::sync::Arc;
use tauri::image::Image;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, Runtime};

pub const TRAY_ID: &str = "xscapenet-tray";

pub fn setup_tray<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    let show_popover = MenuItem::with_id(app, "popover", "Today Summary", true, None::<&str>)?;
    let show_advanced =
        MenuItem::with_id(app, "advanced", "Advanced Analytics", true, None::<&str>)?;
    let refresh_ip = MenuItem::with_id(app, "refresh_ip", "Refresh Public IP", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit XscapeNet", true, None::<&str>)?;

    let menu = Menu::with_items(app, &[&show_popover, &show_advanced, &refresh_ip, &quit])?;

    let mut builder = TrayIconBuilder::with_id(TRAY_ID)
        .menu(&menu)
        .tooltip("XscapeNet — Starting telemetry...")
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "popover" => toggle_popover(app, None),
            "advanced" => open_advanced(app),
            "refresh_ip" => {
                let handle = app.clone();
                tauri::async_runtime::spawn(async move {
                    crate::commands::trigger_ip_refresh(&handle).await;
                });
            }
            "quit" => {
                if let Some(state) = app.try_state::<Arc<crate::commands::AppState>>() {
                    state.storage.checkpoint_wal();
                }
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                position,
                ..
            } = event
            {
                toggle_popover(tray.app_handle(), Some(position));
            }
        });

    // Use the dedicated high-contrast transparent white tray icon for crisp 16x16 visibility
    if let Ok(tray_img) = Image::from_bytes(include_bytes!("../icons/tray-icon.png")) {
        builder = builder.icon(tray_img);
    } else if let Some(icon) = app.default_window_icon().cloned() {
        builder = builder.icon(icon);
    }

    builder.build(app)?;
    Ok(())
}

pub fn toggle_popover<R: Runtime>(app: &AppHandle<R>, click_pos: Option<PhysicalPosition<f64>>) {
    let Some(win) = app.get_webview_window("popover") else {
        return;
    };
    if win.is_visible().unwrap_or(false) {
        let _ = win.hide();
        return;
    }

    // Sync latest theme/privacy settings directly into popover right before showing it
    if let Some(state) = app.try_state::<Arc<crate::commands::AppState>>() {
        let current_settings = state.settings.read().clone();
        let _ = win.emit("xscapenet://settings", &current_settings);
    }

    if let Ok(Some(monitor)) = win.current_monitor() {
        let work_size = monitor.size();
        let scale = monitor.scale_factor();
        let win_w = (384.0 * scale) as i32;
        let win_h = (560.0 * scale) as i32;
        let margin = (14.0 * scale) as i32;
        let taskbar_h = (54.0 * scale) as i32;

        let x = if let Some(pos) = click_pos {
            let centered = pos.x as i32 - (win_w / 2);
            centered.clamp(margin, (work_size.width as i32) - win_w - margin)
        } else {
            (work_size.width as i32) - win_w - margin
        };
        let y = (work_size.height as i32) - win_h - taskbar_h;
        let _ = win.set_position(PhysicalPosition::new(x, y));
    }

    let _ = win.show();
    let _ = win.set_focus();
}

pub fn open_advanced<R: Runtime>(app: &AppHandle<R>) {
    if let Some(pop) = app.get_webview_window("popover") {
        let _ = pop.hide();
    }
    if let Some(adv) = app.get_webview_window("advanced") {
        let _ = adv.unminimize();
        let _ = adv.show();
        let _ = adv.set_focus();
    }
}

pub fn update_tray_tooltip<R: Runtime>(app: &AppHandle<R>, text: &str) {
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        let _ = tray.set_tooltip(Some(text));
    }
}
