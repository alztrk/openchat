#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod chat_storage;
mod local_data;
mod mcp_probe;
mod mcp_storage;
mod notifications;
mod provider_storage;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .invoke_handler(tauri::generate_handler![
            chat_storage::prepare_chat_database,
            chat_storage::has_completed_assistant_response,
            chat_storage::list_conversations,
            chat_storage::list_projects,
            chat_storage::create_project,
            chat_storage::clear_conversation_history,
            chat_storage::list_model_favorites,
            chat_storage::set_model_favorite,
            chat_storage::list_saved_output_message_keys,
            chat_storage::set_saved_output,
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
            chat_storage::read_message_attachment,
            chat_storage::create_conversation,
            chat_storage::rename_conversation,
            chat_storage::set_conversation_route,
            chat_storage::set_conversation_pinned,
            chat_storage::set_conversation_bookmarked,
            chat_storage::set_conversation_tags,
            chat_storage::move_conversations,
            chat_storage::set_conversations_archived,
            chat_storage::delete_conversation,
            chat_storage::save_conversation_markdown,
            chat_storage::save_message,
            mcp_storage::load_global_mcp_servers,
            mcp_storage::save_global_mcp_server,
            mcp_storage::set_global_mcp_enabled,
            mcp_storage::remove_global_mcp_server,
            mcp_storage::get_global_mcp_secret_status,
            mcp_storage::save_global_mcp_secrets,
            mcp_storage::get_active_global_mcp_instructions,
            mcp_probe::probe_global_mcp_server,
            provider_storage::load_provider_credential_status,
            provider_storage::save_chatgpt_api_key,
            provider_storage::save_compatible_provider_api_key,
            provider_storage::save_opencode_api_key,
            provider_storage::get_provider_api_key,
            provider_storage::delete_chatgpt_api_key,
            provider_storage::delete_compatible_provider_api_key,
            provider_storage::delete_opencode_api_key,
            provider_storage::save_custom_provider,
            provider_storage::delete_custom_provider,
        ])
        .setup(|app| {
            notifications::initialize(app.handle()).map_err(std::io::Error::other)?;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("failed to start OpenChat");
}
