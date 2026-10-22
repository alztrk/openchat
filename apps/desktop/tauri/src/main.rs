//! Tauri host entry point and registration of frontend-accessible native commands.
#![deny(missing_docs)]
#![deny(clippy::undocumented_unsafe_blocks)]
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use tauri::Manager;

mod archive_export;
mod chat_storage;
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
mod window_broadcast;
mod window_state;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .manage(local_service::LocalServiceState::default())
        .invoke_handler(tauri::generate_handler![
            archive_export::save_pending_archive_export,
            chat_storage::prepare_chat_database,
            chat_storage::has_completed_assistant_response,
            chat_storage::list_conversations,
            chat_storage::list_projects,
            chat_storage::create_project,
            project_folder::select_project_folder,
            project_folder::discard_project_folder_selection,
            chat_storage::clear_conversation_history,
            chat_storage::list_model_favorites,
            chat_storage::set_model_favorite,
            chat_storage::list_saved_output_message_keys,
            chat_storage::list_saved_outputs,
            chat_storage::search_saved_outputs,
            chat_storage::search_messages,
            chat_storage::set_saved_output,
            chat_storage::list_saved_output_versions,
            chat_storage::get_saved_output_version,
            chat_storage::save_saved_output_version,
            chat_storage::select_saved_output_version,
            local_data::export_support_diagnostics,
            notifications::read_windows_notification_status,
            notifications::open_windows_notification_settings,
            notifications::send_test_notification,
            notifications::send_assistant_response_notification,
            notifications::send_user_question_notification,
            notifications::take_pending_notification_activations,
            notifications::set_notification_frontend_unready,
            chat_storage::get_conversation,
            chat_storage::get_messages,
            chat_storage::list_conversation_attachments,
            chat_storage::read_message_attachment,
            chat_storage::get_attachment_text_index_status,
            chat_storage::rebuild_attachment_text_index,
            chat_storage::clear_attachment_text_index,
            chat_storage::create_conversation,
            chat_storage::rename_conversation,
            chat_storage::set_conversation_route,
            chat_storage::set_conversation_pinned,
            chat_storage::move_conversations,
            chat_storage::set_conversations_archived,
            chat_storage::delete_conversation,
            chat_storage::save_conversation_markdown,
            chat_storage::save_conversation_export,
            chat_storage::save_message,
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
            chat_storage::get_app_setting,
            chat_storage::get_all_app_settings,
            chat_storage::set_app_setting,
            chat_storage::set_app_settings_batch,
            chat_storage::delete_app_setting,
            chat_storage::list_personal_memory_facts,
            chat_storage::save_personal_memory_fact,
            chat_storage::delete_personal_memory_fact,
            chat_storage::clear_personal_memory_facts,
            chat_storage::get_cache_entry,
            chat_storage::set_cache_entry,
            chat_storage::migrate_localstorage_to_db,
            chat_storage::optimize_database,
            window_broadcast::broadcast_window_sync_event,
        ])
        .setup(|app| {
            chat_storage::prepare_chat_database().map_err(|error| {
                std::io::Error::other(format!("Database initialization failed: {error:?}"))
            })?;
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
            let _ = chat_storage::optimize_database();
            if let Err(error) = handle.state::<local_service::LocalServiceState>().stop() {
                eprintln!("OpenChat local service shutdown failed: {error}");
            }
        }
    });
    Ok(())
}
