//! Shows the click-through desktop indicator while the assistant controls the desktop.

use tauri::AppHandle;

#[cfg(windows)]
use tauri::{Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

#[cfg(windows)]
const OVERLAY_LABEL_PREFIX: &str = "computer-use-overlay-";

/// Shows or hides the click-through Computer Use indicator on every connected display.
#[cfg(windows)]
#[tauri::command]
pub fn set_computer_use_overlay(
    app: AppHandle,
    active: bool,
    accent: [u8; 3],
) -> Result<(), String> {
    let existing = app
        .webview_windows()
        .into_iter()
        .filter(|(label, _)| label.starts_with(OVERLAY_LABEL_PREFIX))
        .map(|(_, window)| window)
        .collect::<Vec<_>>();
    if !active {
        let errors = close_created(&existing);
        if !errors.is_empty() {
            return Err(format!(
                "Computer Use indicator windows could not all be closed: {}",
                errors.join("; ")
            ));
        }
        return Ok(());
    }
    if !existing.is_empty() {
        return Ok(());
    }

    let monitors = app
        .available_monitors()
        .map_err(|error| error.to_string())?;
    if monitors.is_empty() {
        return Err("No display is available for the Computer Use indicator.".to_owned());
    }
    let primary_position = app
        .primary_monitor()
        .map_err(|error| error.to_string())?
        .map(|monitor| monitor.position().to_logical::<f64>(monitor.scale_factor()));

    let mut created = Vec::with_capacity(monitors.len());
    for (index, monitor) in monitors.iter().enumerate() {
        let scale = monitor.scale_factor();
        let position = monitor.position().to_logical::<f64>(scale);
        let size = monitor.size().to_logical::<f64>(scale);
        let show_banner = primary_position.as_ref().map_or(index == 0, |primary| {
            primary.x == position.x && primary.y == position.y
        });
        let url = format!(
            "index.html?computerUseOverlay=1&accent={},{},{}&banner={}",
            accent[0], accent[1], accent[2], show_banner
        );
        let window = match WebviewWindowBuilder::new(
            &app,
            format!("{OVERLAY_LABEL_PREFIX}{index}"),
            WebviewUrl::App(url.into()),
        )
        .title("OpenChat Computer Use")
        .position(position.x, position.y)
        .inner_size(size.width, size.height)
        .decorations(false)
        .always_on_top(true)
        .visible_on_all_workspaces(true)
        .skip_taskbar(true)
        .shadow(false)
        .resizable(false)
        .focusable(false)
        .transparent(true)
        .visible(false)
        .build()
        {
            Ok(window) => window,
            Err(error) => {
                let cleanup_errors = close_created(&created);
                return Err(append_cleanup_errors(error.to_string(), cleanup_errors));
            }
        };
        if let Err(error) = window.set_ignore_cursor_events(true) {
            let mut cleanup_errors = close_created(&created);
            if let Err(close_error) = window.close() {
                cleanup_errors.push(close_error.to_string());
            }
            return Err(append_cleanup_errors(error.to_string(), cleanup_errors));
        }
        created.push(window);
    }
    for window in &created {
        if let Err(error) = window.show() {
            let cleanup_errors = close_created(&created);
            return Err(append_cleanup_errors(error.to_string(), cleanup_errors));
        }
    }
    Ok(())
}

fn close_created(windows: &[WebviewWindow]) -> Vec<String> {
    let mut errors = Vec::new();
    for window in windows {
        if let Err(error) = window.close() {
            errors.push(format!("{}: {error}", window.label()));
        }
    }
    errors
}

fn append_cleanup_errors(message: String, errors: Vec<String>) -> String {
    if errors.is_empty() {
        message
    } else {
        format!("{message}; overlay cleanup failed: {}", errors.join("; "))
    }
}

/// Keeps the command registry available on non-Windows builds where desktop input is unavailable.
#[cfg(not(windows))]
#[tauri::command]
pub fn set_computer_use_overlay(
    _app: AppHandle,
    active: bool,
    _accent: [u8; 3],
) -> Result<(), String> {
    if active {
        Err("Computer Use is available only on Windows.".to_owned())
    } else {
        Ok(())
    }
}
