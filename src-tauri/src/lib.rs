mod commands;
pub mod store;

use store::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt::init();

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_opener::init())
        .manage(AppState::new())
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
