//! Persists and restores window state (position, size, maximized) in a separate JSON file.
//!
//! Replaces `tauri-plugin-window-state` without opening the primary application database.

use serde::{Deserialize, Serialize};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::PathBuf,
};
use tauri::{
    AppHandle, Manager, PhysicalPosition, PhysicalSize, Position, Size, WebviewWindow, Window,
};

const WINDOW_STATE_FILE_NAME: &str = "window-state.json";

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

/// Restores window size, position, and maximized state from the window-state file.
pub fn restore_window_state(window: &WebviewWindow) {
    if let Some(state) = load_window_state(window.app_handle()) {
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

/// Persists the current window state to the window-state file.
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

    let mut state = load_window_state(window.app_handle()).unwrap_or_default();
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

    save_window_state_file(window.app_handle(), &state);
}

fn load_window_state(app: &AppHandle) -> Option<WindowStateRecord> {
    let path = app.path().app_data_dir().ok()?.join(WINDOW_STATE_FILE_NAME);
    let legacy_path = legacy_window_state_paths()
        .into_iter()
        .find(|legacy_path| legacy_path.is_file());
    read_window_state(&path).or_else(|| legacy_path.and_then(|path| read_window_state(&path)))
}

fn read_window_state(path: &PathBuf) -> Option<WindowStateRecord> {
    let bytes = fs::read(path).ok()?;
    let value = serde_json::from_slice::<serde_json::Value>(&bytes).ok()?;
    let target = value.get("main").unwrap_or(&value);
    Some(WindowStateRecord {
        width: target.get("width")?.as_u64()?.try_into().ok()?,
        height: target.get("height")?.as_u64()?.try_into().ok()?,
        x: target.get("x")?.as_i64()?.try_into().ok()?,
        y: target.get("y")?.as_i64()?.try_into().ok()?,
        maximized: target.get("maximized")?.as_bool()?,
    })
}

fn save_window_state_file(app: &AppHandle, state: &WindowStateRecord) {
    let Ok(directory) = app.path().app_data_dir() else {
        return;
    };
    if fs::create_dir_all(&directory).is_err() {
        return;
    }
    let path = directory.join(WINDOW_STATE_FILE_NAME);
    let temporary_path = path.with_extension("json.tmp");
    let Ok(bytes) = serde_json::to_vec(state) else {
        return;
    };
    let Ok(mut file) = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&temporary_path)
    else {
        return;
    };
    if file.write_all(&bytes).is_err() || file.sync_all().is_err() {
        let _ = fs::remove_file(&temporary_path);
        return;
    }
    drop(file);
    if fs::rename(&temporary_path, &path).is_err() {
        let _ = fs::remove_file(&temporary_path);
    }
}

fn legacy_window_state_paths() -> Vec<PathBuf> {
    let app_data = std::env::var_os("APPDATA");
    let Some(app_data) = app_data else {
        return Vec::new();
    };
    vec![
        PathBuf::from(&app_data)
            .join("com.openchat.desktop")
            .join(".window-state.json"),
        PathBuf::from(&app_data)
            .join("OpenChat")
            .join(".window-state.json"),
    ]
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
