//! dockwin GUI — Tauri v2 application entry point.
//!
//! This is the thin desktop shell. All real logic lives in [`docker`] (the
//! bollard client over the named-pipe relay) and is exposed to the web frontend
//! through the `#[tauri::command]` wrappers in [`commands`]. This file only:
//!   * builds the Tauri app,
//!   * registers the command handlers + shared [`AppState`],
//!   * installs a system tray icon with a show/hide/quit menu,
//!   * keeps the app alive in the tray when the window is closed.
//!
//! Anti-bloat: no background service, no telemetry, no auto-updater.

// On Windows release builds, hide the console window that would otherwise pop up.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod docker;
mod relay;

use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager, WindowEvent,
};

use commands::AppState;

/// Show the main window and bring it to the foreground.
fn show_main_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

/// Hide the main window (app keeps running in the tray).
fn hide_main_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.hide();
    }
}

/// Build the tray icon + context menu and wire its events.
fn setup_tray(app: &tauri::AppHandle) -> tauri::Result<()> {
    // Menu items. IDs are matched in the menu event handler below.
    let show_item = MenuItem::with_id(app, "show", "Show dockwin", true, None::<&str>)?;
    let hide_item = MenuItem::with_id(app, "hide", "Hide window", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let quit_item = MenuItem::with_id(app, "quit", "Quit dockwin", true, None::<&str>)?;

    let menu = Menu::with_items(app, &[&show_item, &hide_item, &separator, &quit_item])?;

    let _tray = TrayIconBuilder::with_id("dockwin-tray")
        .tooltip("dockwin")
        // Reuse the app's default window icon for the tray.
        .icon(app.default_window_icon().cloned().expect("default icon"))
        .menu(&menu)
        // Don't toggle the menu on left click — we use left click to show the
        // window and right click for the context menu.
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "show" => show_main_window(app),
            "hide" => hide_main_window(app),
            "quit" => {
                // Explicit user-requested exit (the only path that fully exits).
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            // Left click (press+release) toggles the main window.
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let app = tray.app_handle();
                if let Some(window) = app.get_webview_window("main") {
                    match window.is_visible() {
                        Ok(true) => hide_main_window(app),
                        _ => show_main_window(app),
                    }
                }
            }
        })
        .build(app)?;

    Ok(())
}

fn main() {
    tauri::Builder::default()
        // Plugins declared in Cargo.toml.
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(log::LevelFilter::Info)
                .build(),
        )
        // Shared Docker connection state, reused across all commands.
        .manage(AppState::default())
        .setup(|app| {
            setup_tray(app.handle())?;
            // Host the Windows named-pipe -> WSL dockerd relay so bollard (and
            // the stock docker.exe via a docker context) can reach the engine.
            tauri::async_runtime::spawn(async {
                if let Err(e) = relay::run(dockwin_core::wsl::DISTRO.to_string()).await {
                    log::error!("named-pipe relay exited: {e}");
                }
            });
            Ok(())
        })
        // Closing the window hides to tray instead of exiting; "Quit" exits.
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                // Keep dockwin running in the tray rather than quitting.
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .invoke_handler(tauri::generate_handler![
            // Engine status / lifecycle
            commands::engine_status,
            commands::engine_version,
            commands::engine_start,
            commands::engine_stop,
            commands::engine_provision,
            commands::engine_teardown,
            commands::set_tcp_fallback,
            // Containers
            commands::container_list,
            commands::container_start,
            commands::container_stop,
            commands::container_restart,
            commands::container_remove,
            // Logs
            commands::container_logs,
            // Images
            commands::image_list,
            // Compose
            commands::compose_up,
            commands::compose_down,
        ])
        .run(tauri::generate_context!())
        .expect("error while running the dockwin Tauri application");
}
