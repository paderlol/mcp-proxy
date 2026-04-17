//! System tray / menu-bar icon.
//!
//! Gives users a quick way to show/hide the main window, lock the vault,
//! and quit — without needing the main window to be visible. On macOS this
//! shows in the menu bar; on Windows it's the taskbar notification area.

use mcp_proxy_common::local_backend;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, Runtime, WindowEvent};

const MENU_SHOW: &str = "tray_show";
const MENU_LOCK_VAULT: &str = "tray_lock_vault";
const MENU_QUIT: &str = "tray_quit";

/// Build the tray icon + menu and attach it to the app. Called once from
/// `lib.rs` setup.
pub fn setup<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    let show_window = MenuItem::with_id(app, MENU_SHOW, "Show MCP Proxy", true, None::<&str>)?;
    let lock_vault = MenuItem::with_id(
        app,
        MENU_LOCK_VAULT,
        "Lock Vault",
        // Disable the menu item on macOS since Keychain has no process-scoped
        // lock. Elsewhere it's enabled whether or not a session exists —
        // clicking lock when already locked is harmless.
        !matches!(
            local_backend::default_backend(),
            local_backend::LocalBackend::Keychain
        ),
        None::<&str>,
    )?;
    let separator = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, MENU_QUIT, "Quit MCP Proxy", true, None::<&str>)?;

    let menu = Menu::with_items(app, &[&show_window, &lock_vault, &separator, &quit])?;

    let icon = app
        .default_window_icon()
        .cloned()
        .ok_or_else(|| tauri::Error::AssetNotFound("default window icon".into()))?;

    let _tray = TrayIconBuilder::with_id("mcp-proxy-tray")
        .tooltip("MCP Proxy")
        .icon(icon)
        .menu(&menu)
        // Don't auto-popup the menu on left-click; we want left-click to
        // toggle the main window, right-click shows the menu (default).
        .show_menu_on_left_click(false)
        .on_menu_event(on_menu_event)
        .on_tray_icon_event(on_tray_icon_event)
        .build(app)?;

    Ok(())
}

fn on_menu_event<R: Runtime>(app: &AppHandle<R>, event: tauri::menu::MenuEvent) {
    match event.id.as_ref() {
        MENU_SHOW => show_main_window(app),
        MENU_LOCK_VAULT => {
            local_backend::lock_vault();
            tracing::info!("vault locked via tray");
        }
        MENU_QUIT => {
            app.exit(0);
        }
        _ => {}
    }
}

fn on_tray_icon_event<R: Runtime>(tray: &tauri::tray::TrayIcon<R>, event: TrayIconEvent) {
    // Left-click on the tray icon toggles the main window. Right-click
    // still shows the menu (default behavior, already handled by Tauri).
    if let TrayIconEvent::Click {
        button: MouseButton::Left,
        button_state: MouseButtonState::Up,
        ..
    } = event
    {
        let app = tray.app_handle();
        toggle_main_window(app);
    }
}

fn show_main_window<R: Runtime>(app: &AppHandle<R>) {
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.show();
        let _ = win.unminimize();
        let _ = win.set_focus();
    }
}

fn toggle_main_window<R: Runtime>(app: &AppHandle<R>) {
    let Some(win) = app.get_webview_window("main") else {
        return;
    };
    match win.is_visible() {
        Ok(true) => {
            let _ = win.hide();
        }
        _ => {
            let _ = win.show();
            let _ = win.unminimize();
            let _ = win.set_focus();
        }
    }
}

/// Handle the main window's close event: hide instead of quit, so the app
/// keeps running in the tray. Call this from the app-level `on_window_event`
/// wiring in `lib.rs`.
pub fn handle_window_event<R: Runtime>(window: &tauri::Window<R>, event: &WindowEvent) {
    if let WindowEvent::CloseRequested { api, .. } = event {
        // Keep the app alive in the tray when the user closes the main window.
        // Users can still quit via the tray menu's "Quit MCP Proxy" item or
        // ⌘Q on macOS (which bypasses this handler).
        api.prevent_close();
        let _ = window.hide();
    }
}
