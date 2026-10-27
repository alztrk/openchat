//! Tauri host entry point and registration of frontend-accessible native commands.
#![deny(missing_docs)]
#![deny(clippy::undocumented_unsafe_blocks)]
#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

use tauri::Manager;

mod archive_export;
#[cfg(test)]
// The test harness does not start the Tauri runtime that calls these commands.
#[allow(dead_code)]
mod chat_storage;
#[cfg(not(test))]
#[path = "chat_storage_runtime.rs"]
mod chat_storage;
mod computer_use_overlay;
mod file_ops;
mod local_data;
mod local_service;
mod mcp_probe;
mod mcp_storage;
mod notifications;
mod project_folder;
mod provider_storage;
mod request_bounds;
mod save_dialog;
#[cfg_attr(test, allow(dead_code))]
mod window_broadcast;
mod window_state;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .manage(local_service::LocalServiceState::default())
        .invoke_handler(tauri::generate_handler![
            archive_export::save_pending_archive_export,
            chat_storage::get_chat_schema_version,
            computer_use_overlay::set_computer_use_overlay,
            project_folder::select_project_folder,
            project_folder::discard_project_folder_selection,
            project_folder::consume_project_folder_selection_command,
            local_data::export_support_diagnostics,
            notifications::read_windows_notification_status,
            notifications::open_windows_notification_settings,
            notifications::send_test_notification,
            notifications::send_assistant_response_notification,
            notifications::send_user_question_notification,
            notifications::take_pending_notification_activations,
            notifications::set_notification_frontend_unready,
            chat_storage::save_conversation_markdown,
            chat_storage::save_conversation_export,
            chat_storage::record_chat_request_failure,
            chat_storage::record_connection_removal_diagnostic,
            chat_storage::record_context_usage_failure,
            mcp_storage::load_global_mcp_servers,
            mcp_storage::save_global_mcp_server,
            mcp_storage::set_global_mcp_enabled,
            mcp_storage::remove_global_mcp_server,
            mcp_storage::get_global_mcp_secret_status,
            mcp_storage::save_global_mcp_secrets,
            mcp_probe::probe_global_mcp_server,
            local_service::start_local_service,
            local_service::start_local_service_with_empty_storage,
            local_service::write_local_service,
            local_service::respond_local_service_permission,
            provider_storage::load_provider_credential_status,
            provider_storage::save_chatgpt_api_key,
            provider_storage::save_compatible_provider_api_key,
            provider_storage::save_opencode_api_key,
            provider_storage::delete_chatgpt_api_key,
            provider_storage::delete_compatible_provider_api_key,
            provider_storage::delete_opencode_api_key,
            provider_storage::save_custom_provider,
            provider_storage::delete_custom_provider,
            window_broadcast::broadcast_window_sync_event,
        ])
        .setup(|app| {
            if let Err(error) = local_service::start_local_service_early(app.handle()) {
                eprintln!(
                    "OpenChat local service early startup failed; the renderer will retry: {error}"
                );
            }
            let main_window = app
                .get_webview_window("main")
                .ok_or_else(|| std::io::Error::other("Main window is unavailable"))?;
            window_state::restore_window_state(&main_window);
            notifications::initialize(app.handle()).map_err(std::io::Error::other)?;
            Ok(())
        })
        .on_window_event(|window, event| {
            if matches!(
                event,
                tauri::WindowEvent::Resized(_)
                    | tauri::WindowEvent::Moved(_)
                    | tauri::WindowEvent::CloseRequested { .. }
            ) {
                window_state::save_window_state(window);
            }
        })
        .build(tauri::generate_context!())?;
    app.run(|handle, event| {
        if matches!(event, tauri::RunEvent::Exit) {
            if let Err(error) = handle.state::<local_service::LocalServiceState>().stop() {
                eprintln!("OpenChat local service shutdown failed: {error}");
            }
        }
    });
    Ok(())
}
