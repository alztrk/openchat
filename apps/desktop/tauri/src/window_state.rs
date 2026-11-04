//! Persists and restores window state (position, size, maximized) in a separate JSON file.
//!
//! Replaces `tauri-plugin-window-state` without opening the primary application database.

use serde::{Deserialize, Serialize};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::PathBuf,
};
use tauri::{AppHandle, Manager, Monitor, PhysicalPosition, PhysicalSize, WebviewWindow, Window};

const WINDOW_STATE_FILE_NAME: &str = "window-state.json";

// Default window dimensions
const DEFAULT_WINDOW_WIDTH: u32 = 1200;
const DEFAULT_WINDOW_HEIGHT: u32 = 800;
const DEFAULT_WINDOW_X: i32 = 100;
const DEFAULT_WINDOW_Y: i32 = 100;
const MIN_WINDOW_WIDTH: u32 = 400;
const MIN_WINDOW_HEIGHT: u32 = 300;

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
            width: DEFAULT_WINDOW_WIDTH,
            height: DEFAULT_WINDOW_HEIGHT,
            x: DEFAULT_WINDOW_X,
            y: DEFAULT_WINDOW_Y,
            maximized: false,
        }
    }
}

/// Restores the previous window geometry or maximizes the first launch.
pub fn restore_window_state(window: &WebviewWindow) -> tauri::Result<()> {
    let Some(state) = load_window_state(window.app_handle()) else {
        return window.maximize();
    };

    let monitors = match window.available_monitors() {
        Ok(monitors) => monitors,
        Err(error) => {
            eprintln!("OpenChat could not read monitor geometry; maximizing the window: {error}");
            return window.maximize();
        }
    };
    let work_areas: Vec<_> = monitors
        .iter()
        .map(monitor_work_area)
        .filter(|area| area.width > 0 && area.height > 0)
        .collect();
    let primary_area = match window.primary_monitor() {
        Ok(monitor) => monitor.map(|monitor| monitor_work_area(&monitor)),
        Err(error) => {
            eprintln!(
                "OpenChat could not identify the primary monitor for window restoration; using the first available monitor: {error}"
            );
            None
        }
    };
    let Some(restored_state) = resolve_window_state(state, &work_areas, primary_area) else {
        eprintln!("OpenChat found no usable monitor work area; maximizing the window.");
        return window.maximize();
    };

    let restore_result: tauri::Result<()> = (|| {
        window.unmaximize()?;
        window.set_size(PhysicalSize::new(
            restored_state.width,
            restored_state.height,
        ))?;
        window.set_position(PhysicalPosition::new(restored_state.x, restored_state.y))?;
        if restored_state.maximized {
            window.maximize()?;
        }
        Ok(())
    })();
    if let Err(error) = restore_result {
        if let Err(maximize_error) = window.maximize() {
            eprintln!(
                "OpenChat could not restore saved window geometry ({error}) or maximize the fallback window ({maximize_error})."
            );
        } else {
            eprintln!("OpenChat could not restore saved window geometry; maximized the fallback window: {error}");
        }
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct MonitorWorkArea {
    x: i32,
    y: i32,
    width: u32,
    height: u32,
}

fn monitor_work_area(monitor: &Monitor) -> MonitorWorkArea {
    let area = monitor.work_area();
    MonitorWorkArea {
        x: area.position.x,
        y: area.position.y,
        width: area.size.width,
        height: area.size.height,
    }
}

fn resolve_window_state(
    state: WindowStateRecord,
    work_areas: &[MonitorWorkArea],
    primary_area: Option<MonitorWorkArea>,
) -> Option<WindowStateRecord> {
    let (area, _) = work_areas
        .iter()
        .copied()
        .map(|area| (area, intersection_area(state, area)))
        .filter(|(_, overlap)| *overlap > 0)
        .max_by_key(|(_, overlap)| *overlap)
        .or_else(|| {
            primary_area
                .filter(|primary| work_areas.contains(primary))
                .map(|primary| (primary, 0))
        })
        .or_else(|| work_areas.first().copied().map(|area| (area, 0)))?;

    let width = state
        .width
        .clamp(MIN_WINDOW_WIDTH.min(area.width), area.width);
    let height = state
        .height
        .clamp(MIN_WINDOW_HEIGHT.min(area.height), area.height);
    let max_x = area.x as i64 + area.width as i64 - width as i64;
    let max_y = area.y as i64 + area.height as i64 - height as i64;
    let x = (state.x as i64).clamp(area.x as i64, max_x) as i32;
    let y = (state.y as i64).clamp(area.y as i64, max_y) as i32;

    Some(WindowStateRecord {
        width,
        height,
        x,
        y,
        maximized: state.maximized,
    })
}

fn intersection_area(state: WindowStateRecord, area: MonitorWorkArea) -> u64 {
    let left = (state.x as i64).max(area.x as i64);
    let top = (state.y as i64).max(area.y as i64);
    let right = (state.x as i64 + state.width as i64).min(area.x as i64 + area.width as i64);
    let bottom = (state.y as i64 + state.height as i64).min(area.y as i64 + area.height as i64);

    if right <= left || bottom <= top {
        return 0;
    }
    (right - left) as u64 * (bottom - top) as u64
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
            if size.width >= MIN_WINDOW_WIDTH && size.height >= MIN_WINDOW_HEIGHT {
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
    use super::{resolve_window_state, MonitorWorkArea, WindowStateRecord};

    fn state(width: u32, height: u32, x: i32, y: i32, maximized: bool) -> WindowStateRecord {
        WindowStateRecord {
            width,
            height,
            x,
            y,
            maximized,
        }
    }

    fn work_area(x: i32, y: i32, width: u32, height: u32) -> MonitorWorkArea {
        MonitorWorkArea {
            x,
            y,
            width,
            height,
        }
    }

    #[test]
    fn window_state_serializes_and_deserializes() {
        let record = state(1400, 900, 200, 150, true);
        let json_str = serde_json::to_string(&record).expect("serialize");
        let deserialized: WindowStateRecord = serde_json::from_str(&json_str).expect("deserialize");
        assert_eq!(record, deserialized);
    }

    #[test]
    fn saved_geometry_is_preserved_when_it_fits_a_monitor() {
        let saved = state(1400, 900, -1500, 100, false);
        let area = work_area(-1920, 0, 1920, 1080);

        assert_eq!(resolve_window_state(saved, &[area], None), Some(saved));
    }

    #[test]
    fn largest_monitor_overlap_selects_the_restore_target() {
        let saved = state(500, 500, 1750, 100, false);
        let primary = work_area(0, 0, 1920, 1080);
        let secondary = work_area(1920, 0, 1280, 1024);

        assert_eq!(
            resolve_window_state(saved, &[primary, secondary], Some(primary)),
            Some(state(500, 500, 1920, 100, false)),
        );
    }

    #[test]
    fn offscreen_geometry_uses_the_primary_monitor_and_clamps_position() {
        let saved = state(1400, 900, 5000, 5000, true);
        let primary = work_area(-1920, 0, 1920, 1080);
        let other = work_area(0, 0, 1920, 1080);

        assert_eq!(
            resolve_window_state(saved, &[primary, other], Some(primary)),
            Some(state(1400, 900, -1400, 180, true)),
        );
    }

    #[test]
    fn oversized_geometry_is_reduced_to_fit_the_work_area() {
        let saved = state(2000, 1500, -100, -200, false);
        let area = work_area(0, 0, 900, 700);

        assert_eq!(
            resolve_window_state(saved, &[area], None),
            Some(state(900, 700, 0, 0, false)),
        );
    }

    #[test]
    fn minimum_size_is_reduced_for_a_smaller_work_area() {
        let saved = state(100, 100, 0, 0, false);
        let area = work_area(0, 0, 500, 400);

        assert_eq!(
            resolve_window_state(saved, &[area], None),
            Some(state(400, 300, 0, 0, false)),
        );
    }

    #[test]
    fn no_available_work_areas_keeps_the_default_geometry() {
        assert_eq!(
            resolve_window_state(state(1200, 800, 100, 100, false), &[], None),
            None
        );
    }
}
