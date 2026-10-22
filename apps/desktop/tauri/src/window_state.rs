//! Persists and restores window state (position, size, maximized) using SQLite `app_settings`.
//!
//! Replaces `tauri-plugin-window-state` and eliminates `.window-state.json`.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tauri::{PhysicalPosition, PhysicalSize, Position, Size, WebviewWindow, Window};

const WINDOW_STATE_SETTING_KEY: &str = "window.state";

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
/// Persisted window geometry and state.
pub struct WindowStateRecord {
    /// Window width in physical pixels.
    pub width: u32,
    /// Window height in physical pixels.
    pub height: u32,
    /// Window X coordinate in physical pixels.
    pub x: i32,
    /// Window Y coordinate in physical pixels.
    pub y: i32,
    /// Whether the window was maximized.
    pub maximized: bool,
}

impl Default for WindowStateRecord {
    fn default() -> Self {
        Self {
            width: 1200,
            height: 800,
            x: 100,
            y: 100,
            maximized: false,
        }
    }
}

/// Restores window size, position, and maximized state from the database.
pub fn restore_window_state(window: &WebviewWindow) {
    migrate_legacy_window_state_file();

    if let Some(state) = load_window_state() {
        if state.maximized {
            let _ = window.maximize();
        } else {
            if state.width >= 400 && state.height >= 300 {
                let _ = window.set_size(Size::Physical(PhysicalSize {
                    width: state.width,
                    height: state.height,
                }));
            }
            let _ = window.set_position(Position::Physical(PhysicalPosition {
                x: state.x,
                y: state.y,
            }));
        }
    }
}

/// Persists the current window state to `app_settings` in SQLite.
pub fn save_window_state(window: &Window) {
    let Ok(is_maximized) = window.is_maximized() else {
        return;
    };
    let Ok(is_minimized) = window.is_minimized() else {
        return;
    };
    if is_minimized {
        return;
    }

    let mut state = load_window_state().unwrap_or_default();
    state.maximized = is_maximized;

    if !is_maximized {
        if let Ok(size) = window.outer_size() {
            if size.width >= 400 && size.height >= 300 {
                state.width = size.width;
                state.height = size.height;
            }
        }
        if let Ok(pos) = window.outer_position() {
            state.x = pos.x;
            state.y = pos.y;
        }
    }

    if let Ok(json_str) = serde_json::to_string(&state) {
        let _ = crate::chat_storage::set_app_setting_raw(WINDOW_STATE_SETTING_KEY, &json_str);
    }
}

fn load_window_state() -> Option<WindowStateRecord> {
    let json_str = crate::chat_storage::get_app_setting_raw(WINDOW_STATE_SETTING_KEY).ok()??;
    serde_json::from_str(&json_str).ok()
}

#[allow(dead_code)]
fn migrate_legacy_window_state_file() {
    let app_data = std::env::var_os("APPDATA");
    let Some(app_data) = app_data else { return };
    let legacy_paths = [
        PathBuf::from(&app_data)
            .join("com.openchat.desktop")
            .join(".window-state.json"),
        PathBuf::from(&app_data)
            .join("OpenChat")
            .join(".window-state.json"),
    ];

    for path in legacy_paths {
        if path.is_file() {
            if let Ok(bytes) = fs::read(&path) {
                if let Ok(val) = serde_json::from_slice::<serde_json::Value>(&bytes) {
                    let target = val.get("main").unwrap_or(&val);
                    let width = target
                        .get("width")
                        .and_then(|v| v.as_u64())
                        .map(|v| v as u32)
                        .unwrap_or(1200);
                    let height = target
                        .get("height")
                        .and_then(|v| v.as_u64())
                        .map(|v| v as u32)
                        .unwrap_or(800);
                    let x = target
                        .get("x")
                        .and_then(|v| v.as_i64())
                        .map(|v| v as i32)
                        .unwrap_or(100);
                    let y = target
                        .get("y")
                        .and_then(|v| v.as_i64())
                        .map(|v| v as i32)
                        .unwrap_or(100);
                    let maximized = target
                        .get("maximized")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);
                    let record = WindowStateRecord {
                        width,
                        height,
                        x,
                        y,
                        maximized,
                    };
                    if let Ok(json_str) = serde_json::to_string(&record) {
                        let _ = crate::chat_storage::set_app_setting_raw(
                            WINDOW_STATE_SETTING_KEY,
                            &json_str,
                        );
                    }
                }
            }
            let _ = fs::remove_file(path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::WindowStateRecord;

    #[test]
    fn window_state_serializes_and_deserializes() {
        let record = WindowStateRecord {
            width: 1400,
            height: 900,
            x: 200,
            y: 150,
            maximized: true,
        };
        let json_str = serde_json::to_string(&record).expect("serialize");
        let deserialized: WindowStateRecord = serde_json::from_str(&json_str).expect("deserialize");
        assert_eq!(record, deserialized);
    }
}
