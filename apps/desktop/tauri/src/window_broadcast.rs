//! Multi-window state broadcasting infrastructure for Tauri.
//!
//! Synchronizes configuration, conversation mutations, and runtime events
//! across all active webview windows via Tauri's event bus.

use serde::Serialize;
use tauri::{AppHandle, Emitter, Runtime};

use crate::request_bounds::BoundedText;

const MAX_SETTING_KEY_BYTES: usize = 128;
const MAX_SETTING_VALUE_BYTES: usize = 2 * 1024 * 1024;

/// Event channel name used for broadcasting application setting updates.
pub const APP_SETTING_CHANGED_EVENT: &str = "openchat://app-setting-changed";

/// Event channel name used for broadcasting conversation collection updates.
#[allow(dead_code)]
pub const CONVERSATION_CHANGED_EVENT: &str = "openchat://conversation-changed";

/// Payload broadcast when an application setting is modified.
#[derive(Clone, Debug, Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AppSettingChangedPayload {
    /// The setting key that was modified.
    pub key: String,
    /// The serialized JSON value of the setting.
    pub value_json: String,
}

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BoundedAppSettingChangedPayload {
    key: BoundedText<MAX_SETTING_KEY_BYTES>,
    value_json: BoundedText<MAX_SETTING_VALUE_BYTES>,
}

impl From<BoundedAppSettingChangedPayload> for AppSettingChangedPayload {
    fn from(payload: BoundedAppSettingChangedPayload) -> Self {
        Self {
            key: payload.key.into_inner(),
            value_json: payload.value_json.into_inner(),
        }
    }
}

/// Payload broadcast when a conversation record is modified or deleted.
#[allow(dead_code)]
#[derive(Clone, Debug, Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct ConversationChangedPayload {
    /// The ID of the affected conversation.
    pub conversation_id: String,
    /// The action performed: "created", "updated", "archived", or "deleted".
    pub action: String,
}

/// Emits an event with a serializable payload to all active windows.
///
/// # Errors
/// Returns an error string if Tauri fails to emit the event across the bridge.
pub fn broadcast_sync_event<R: Runtime, P: Serialize + Clone>(
    app: &AppHandle<R>,
    event: &str,
    payload: P,
) -> Result<(), String> {
    app.emit(event, payload)
        .map_err(|error| format!("failed to broadcast window event '{event}': {error}"))
}

/// Broadcasts a setting modification to all open application windows.
///
/// # Errors
/// Returns an error string if event propagation fails.
#[cfg(test)]
pub fn broadcast_app_setting_change<R: Runtime>(
    app: &AppHandle<R>,
    key: &str,
    value_json: &str,
) -> Result<(), String> {
    let payload = AppSettingChangedPayload {
        key: key.to_owned(),
        value_json: value_json.to_owned(),
    };
    broadcast_sync_event(app, APP_SETTING_CHANGED_EVENT, payload)
}

/// Broadcasts a conversation update to all open application windows.
///
/// # Errors
/// Returns an error string if event propagation fails.
#[allow(dead_code)]
pub fn broadcast_conversation_change<R: Runtime>(
    app: &AppHandle<R>,
    conversation_id: &str,
    action: &str,
) -> Result<(), String> {
    let payload = ConversationChangedPayload {
        conversation_id: conversation_id.to_owned(),
        action: action.to_owned(),
    };
    broadcast_sync_event(app, CONVERSATION_CHANGED_EVENT, payload)
}

/// Broadcasts a validated setting update to all application windows.
///
/// # Errors
/// Returns an error string if event propagation fails.
#[tauri::command]
pub(crate) fn broadcast_window_sync_event(
    app: AppHandle,
    payload: BoundedAppSettingChangedPayload,
) -> Result<(), String> {
    broadcast_sync_event(
        &app,
        APP_SETTING_CHANGED_EVENT,
        AppSettingChangedPayload::from(payload),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_setting_payload_serialization() {
        let payload = AppSettingChangedPayload {
            key: "theme.mode".to_owned(),
            value_json: "\"dark\"".to_owned(),
        };
        let json = serde_json::to_string(&payload).expect("serialize payload");
        let parsed: AppSettingChangedPayload =
            serde_json::from_str(&json).expect("deserialize payload");
        assert_eq!(payload, parsed);
    }

    #[test]
    fn setting_broadcast_payload_rejects_unexpected_fields() {
        let payload = serde_json::json!({
            "key": "appearance.theme",
            "value_json": "\"dark\"",
            "extra": "unexpected",
        });

        assert!(serde_json::from_value::<BoundedAppSettingChangedPayload>(payload).is_err());
    }

    #[test]
    fn setting_broadcast_payload_rejects_oversized_key_and_value() {
        let oversized_key = serde_json::json!({
            "key": "k".repeat(MAX_SETTING_KEY_BYTES + 1),
            "value_json": "null",
        });
        assert!(serde_json::from_value::<BoundedAppSettingChangedPayload>(oversized_key).is_err());

        let oversized_value = serde_json::json!({
            "key": "appearance.theme",
            "value_json": "v".repeat(MAX_SETTING_VALUE_BYTES + 1),
        });
        assert!(
            serde_json::from_value::<BoundedAppSettingChangedPayload>(oversized_value).is_err()
        );
    }

    #[test]
    fn setting_broadcast_payload_accepts_the_service_value_limit() {
        let value_json = "v".repeat(MAX_SETTING_VALUE_BYTES);
        let payload = serde_json::json!({
            "key": "appearance.theme",
            "value_json": value_json,
        });

        let parsed = serde_json::from_value::<BoundedAppSettingChangedPayload>(payload)
            .expect("accept the local-service setting value limit");
        assert_eq!(parsed.key.into_inner(), "appearance.theme");
        assert_eq!(
            parsed.value_json.into_inner().len(),
            MAX_SETTING_VALUE_BYTES
        );
    }

    #[test]
    fn test_conversation_changed_payload_serialization() {
        let payload = ConversationChangedPayload {
            conversation_id: "conv-123".to_owned(),
            action: "created".to_owned(),
        };
        let json = serde_json::to_string(&payload).expect("serialize payload");
        let parsed: ConversationChangedPayload =
            serde_json::from_str(&json).expect("deserialize payload");
        assert_eq!(payload, parsed);
    }
}
