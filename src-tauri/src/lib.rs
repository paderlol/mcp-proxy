mod commands;
pub mod store;
pub mod tray;

use store::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt::init();

    let app = tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_opener::init())
        .manage(AppState::new())
        .setup(|app| {
            tray::setup(app.handle())?;
            Ok(())
        })
        .on_window_event(|window, event| {
            tray::handle_window_event(window, event);
        })
        .invoke_handler(tauri::generate_handler![
            commands::secrets::list_secrets,
            commands::secrets::get_secret,
            commands::secrets::set_secret,
            commands::secrets::delete_secret,
            commands::servers::list_servers,
            commands::servers::get_server,
            commands::servers::add_server,
            commands::servers::update_server,
            commands::servers::delete_server,
            commands::config::generate_config,
            commands::client_write::get_client_config_info,
            commands::client_write::write_client_config,
            commands::vault::vault_status,
            commands::vault::unlock_vault,
            commands::vault::lock_vault,
            commands::vault::change_vault_password,
            commands::vault::reset_vault,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    app.run(|_app_handle, event| {
        // Clean up the vault session file when the app exits normally.
        // A force-kill or crash can still leave it behind; that's handled
        // on the next launch via salt-mismatch detection in
        // `unlock_from_session`.
        if matches!(event, tauri::RunEvent::Exit) {
            mcp_proxy_common::local_backend::lock_vault();
        }
    });
}
