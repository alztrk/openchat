//! Persists and restores window state (position, size, maximized) in a separate JSON file.
//!
//! Replaces `tauri-plugin-window-state` without opening the primary application database.
//! Startup maximizes the main window when no readable saved geometry is available. Saving is
//! best-effort; failures are reported with fixed stage codes that omit filesystem paths.

use serde::{Deserialize, Serialize};
#[cfg(test)]
use std::sync::atomic::{AtomicUsize, Ordering};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::{mpsc, Arc, Condvar, Mutex, MutexGuard},
    thread,
    time::{Duration, Instant},
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
const WINDOW_STATE_SAVE_DEBOUNCE: Duration = Duration::from_millis(300);
const WINDOW_STATE_FLUSH_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WindowStateWriteFailure {
    AppDataDirectory,
    MissingParent,
    CreateDirectory,
    Serialize,
    OpenTemporaryFile,
    WriteTemporaryFile,
    SyncTemporaryFile,
    RemoveTemporaryFile,
    ReplaceStateFile,
    FlushDeadline,
    WriterStopped,
}

impl WindowStateWriteFailure {
    fn diagnostic_code(self) -> &'static str {
        match self {
            Self::AppDataDirectory => "app_data_directory",
            Self::MissingParent => "missing_parent",
            Self::CreateDirectory => "create_directory",
            Self::Serialize => "serialize",
            Self::OpenTemporaryFile => "open_temporary_file",
            Self::WriteTemporaryFile => "write_temporary_file",
            Self::SyncTemporaryFile => "sync_temporary_file",
            Self::RemoveTemporaryFile => "remove_temporary_file",
            Self::ReplaceStateFile => "replace_state_file",
            Self::FlushDeadline => "flush_deadline",
            Self::WriterStopped => "writer_stopped",
        }
    }
}

fn report_window_state_failure(failure: WindowStateWriteFailure) {
    eprintln!(
        "openchat_window_state_failure code={}",
        failure.diagnostic_code()
    );
}

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

/// Coalesces window geometry events and persists their latest state off the event thread.
pub struct WindowStateWriter {
    shared: Arc<WindowStateWriterShared>,
    debounce: Duration,
}

struct WindowStateWriterShared {
    state: Mutex<WindowStateWriterState>,
    changed: Condvar,
    #[cfg(test)]
    write_attempts: AtomicUsize,
}

struct WindowStateWriterState {
    current: WindowStateRecord,
    revision: u64,
    processed_revision: u64,
    last_write_result: Result<(), WindowStateWriteFailure>,
    debounce_deadline: Option<Instant>,
    flush_requested: bool,
    flush_replies: Vec<mpsc::Sender<Result<(), WindowStateWriteFailure>>>,
    shutdown: bool,
}

impl WindowStateWriter {
    /// Starts the background writer using the app's current persisted geometry.
    pub fn start(app: &AppHandle) -> std::io::Result<Self> {
        let path = app
            .path()
            .app_data_dir()
            .map_err(|_| std::io::Error::other("application data directory unavailable"))?
            .join(WINDOW_STATE_FILE_NAME);
        let initial = load_window_state(app).unwrap_or_default();
        Self::start_for_path(path, initial, WINDOW_STATE_SAVE_DEBOUNCE)
    }

    fn start_for_path(
        path: PathBuf,
        initial: WindowStateRecord,
        debounce: Duration,
    ) -> std::io::Result<Self> {
        let shared = Arc::new(WindowStateWriterShared {
            state: Mutex::new(WindowStateWriterState {
                current: initial,
                revision: 0,
                processed_revision: 0,
                last_write_result: Ok(()),
                debounce_deadline: None,
                flush_requested: false,
                flush_replies: Vec::new(),
                shutdown: false,
            }),
            changed: Condvar::new(),
            #[cfg(test)]
            write_attempts: AtomicUsize::new(0),
        });
        let worker_shared = Arc::clone(&shared);
        thread::Builder::new()
            .name("openchat-window-state".to_owned())
            .spawn(move || run_window_state_writer(path, worker_shared, debounce))?;

        Ok(Self { shared, debounce })
    }

    /// Captures the latest non-minimized geometry and schedules a trailing-edge write.
    pub fn schedule_from_window(&self, window: &Window) {
        let previous = lock_writer_state(&self.shared).current;
        if let Some(current) = capture_window_state(window, previous) {
            self.schedule(current);
        }
    }

    /// Captures and writes the current geometry before allowing the window to close.
    pub fn flush_from_window(&self, window: &Window) {
        let previous = lock_writer_state(&self.shared).current;
        self.flush(capture_window_state(window, previous));
    }

    fn schedule(&self, record: WindowStateRecord) {
        let mut state = lock_writer_state(&self.shared);
        state.current = record;
        state.revision = state.revision.saturating_add(1);
        state.debounce_deadline = Some(Instant::now() + self.debounce);
        self.shared.changed.notify_one();
    }

    fn flush(&self, record: Option<WindowStateRecord>) {
        let _ = self.request_flush(record, false);
    }

    fn shutdown_and_flush(&self) {
        let _ = self.request_flush(None, true);
    }

    fn request_flush(
        &self,
        record: Option<WindowStateRecord>,
        shutdown: bool,
    ) -> Result<(), WindowStateWriteFailure> {
        let (reply, response) = mpsc::channel();
        {
            let mut state = lock_writer_state(&self.shared);
            if let Some(record) = record {
                state.current = record;
                state.revision = state.revision.saturating_add(1);
            }
            state.flush_requested = true;
            state.shutdown |= shutdown;
            state.flush_replies.push(reply);
            self.shared.changed.notify_one();
        }

        match response.recv_timeout(WINDOW_STATE_FLUSH_TIMEOUT) {
            Ok(result) => result,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                report_window_state_failure(WindowStateWriteFailure::FlushDeadline);
                Err(WindowStateWriteFailure::FlushDeadline)
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                report_window_state_failure(WindowStateWriteFailure::WriterStopped);
                Err(WindowStateWriteFailure::WriterStopped)
            }
        }
    }
}

impl Drop for WindowStateWriter {
    fn drop(&mut self) {
        let mut state = lock_writer_state(&self.shared);
        state.shutdown = true;
        state.flush_requested = true;
        self.shared.changed.notify_one();
    }
}

fn lock_writer_state(shared: &WindowStateWriterShared) -> MutexGuard<'_, WindowStateWriterState> {
    shared
        .state
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn run_window_state_writer(
    path: PathBuf,
    shared: Arc<WindowStateWriterShared>,
    debounce: Duration,
) {
    loop {
        let (record, revision) = {
            let mut state = lock_writer_state(&shared);
            loop {
                if state.revision == state.processed_revision {
                    if state.flush_requested || state.shutdown {
                        state.flush_requested = false;
                        let replies = std::mem::take(&mut state.flush_replies);
                        let should_exit = state.shutdown;
                        let result = state.last_write_result;
                        drop(state);
                        for reply in replies {
                            let _ = reply.send(result);
                        }
                        if should_exit {
                            return;
                        }
                        state = lock_writer_state(&shared);
                        continue;
                    }
                    state = shared
                        .changed
                        .wait(state)
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                    continue;
                }

                if !state.flush_requested && !state.shutdown {
                    let deadline = state
                        .debounce_deadline
                        .unwrap_or_else(|| Instant::now() + debounce);
                    let wait = deadline.saturating_duration_since(Instant::now());
                    if !wait.is_zero() {
                        let (next_state, _) = shared
                            .changed
                            .wait_timeout(state, wait)
                            .unwrap_or_else(std::sync::PoisonError::into_inner);
                        state = next_state;
                        continue;
                    }
                }

                break (state.current, state.revision);
            }
        };

        let result = save_window_state_path(&path, &record);
        if let Err(failure) = result {
            report_window_state_failure(failure);
        }
        #[cfg(test)]
        shared.write_attempts.fetch_add(1, Ordering::Relaxed);

        let mut state = lock_writer_state(&shared);
        state.processed_revision = state.processed_revision.max(revision);
        state.last_write_result = result;
        if state.revision == state.processed_revision {
            state.debounce_deadline = None;
            if state.flush_requested {
                state.flush_requested = false;
                let replies = std::mem::take(&mut state.flush_replies);
                let should_exit = state.shutdown;
                let result = state.last_write_result;
                drop(state);
                for reply in replies {
                    let _ = reply.send(result);
                }
                if should_exit {
                    return;
                }
            }
        }
    }
}

/// Schedules a coalesced geometry save, falling back to the original direct write if startup
/// could not create the background writer.
pub fn schedule_window_state_save(window: &Window) {
    if let Some(writer) = window.app_handle().try_state::<WindowStateWriter>() {
        writer.schedule_from_window(window);
    } else {
        save_window_state(window);
    }
}

/// Flushes the current geometry before a normal window close.
pub fn flush_window_state_save(window: &Window) {
    if let Some(writer) = window.app_handle().try_state::<WindowStateWriter>() {
        writer.flush_from_window(window);
    } else {
        save_window_state(window);
    }
}

/// Stops the background writer after flushing its latest accepted geometry.
pub fn shutdown_window_state_writer(app: &AppHandle) {
    if let Some(writer) = app.try_state::<WindowStateWriter>() {
        writer.shutdown_and_flush();
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
    let previous = load_window_state(window.app_handle()).unwrap_or_default();
    if let Some(current) = capture_window_state(window, previous) {
        if let Err(failure) = save_window_state_file(window.app_handle(), &current) {
            report_window_state_failure(failure);
        }
    }
}

fn capture_window_state(
    window: &Window,
    mut state: WindowStateRecord,
) -> Option<WindowStateRecord> {
    let is_maximized = window.is_maximized().ok()?;
    if window.is_minimized().ok()? {
        return None;
    }
    state.maximized = is_maximized;

    if !is_maximized {
        if let Ok(size) = window.outer_size() {
            if size.width >= MIN_WINDOW_WIDTH && size.height >= MIN_WINDOW_HEIGHT {
                state.width = size.width;
                state.height = size.height;
            }
        }
        if let Ok(position) = window.outer_position() {
            state.x = position.x;
            state.y = position.y;
        }
    }

    Some(state)
}

fn load_window_state(app: &AppHandle) -> Option<WindowStateRecord> {
    let path = window_state_file_path(app)?;
    let legacy_path = legacy_window_state_paths()
        .into_iter()
        .find(|legacy_path| legacy_path.is_file());
    read_window_state(&path).or_else(|| legacy_path.and_then(|path| read_window_state(&path)))
}

fn window_state_file_path(app: &AppHandle) -> Option<PathBuf> {
    Some(app.path().app_data_dir().ok()?.join(WINDOW_STATE_FILE_NAME))
}

fn read_window_state(path: &Path) -> Option<WindowStateRecord> {
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

fn save_window_state_file(
    app: &AppHandle,
    state: &WindowStateRecord,
) -> Result<(), WindowStateWriteFailure> {
    let path = window_state_file_path(app).ok_or(WindowStateWriteFailure::AppDataDirectory)?;
    save_window_state_path(&path, state)
}

fn save_window_state_path(
    path: &Path,
    state: &WindowStateRecord,
) -> Result<(), WindowStateWriteFailure> {
    let directory = path
        .parent()
        .ok_or(WindowStateWriteFailure::MissingParent)?;
    fs::create_dir_all(directory).map_err(|_| WindowStateWriteFailure::CreateDirectory)?;
    let temporary_path = path.with_extension("json.tmp");
    let bytes = serde_json::to_vec(state).map_err(|_| WindowStateWriteFailure::Serialize)?;
    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&temporary_path)
        .map_err(|_| WindowStateWriteFailure::OpenTemporaryFile)?;
    if file.write_all(&bytes).is_err() {
        drop(file);
        remove_temporary_file(&temporary_path);
        return Err(WindowStateWriteFailure::WriteTemporaryFile);
    }
    if file.sync_all().is_err() {
        drop(file);
        remove_temporary_file(&temporary_path);
        return Err(WindowStateWriteFailure::SyncTemporaryFile);
    }
    drop(file);
    if fs::rename(&temporary_path, path).is_err() {
        remove_temporary_file(&temporary_path);
        return Err(WindowStateWriteFailure::ReplaceStateFile);
    }
    Ok(())
}

fn remove_temporary_file(path: &Path) {
    if fs::remove_file(path).is_err() {
        report_window_state_failure(WindowStateWriteFailure::RemoveTemporaryFile);
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
    use std::{fs, path::PathBuf, thread, time::Duration};

    use super::{
        read_window_state, resolve_window_state, MonitorWorkArea, WindowStateRecord,
        WindowStateWriteFailure, WindowStateWriter,
    };

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn create() -> Self {
            let path = std::env::temp_dir().join(format!(
                "openchat-window-state-test-{}",
                uuid::Uuid::new_v4()
            ));
            fs::create_dir(&path).expect("create isolated window-state directory");
            Self(path)
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).expect("remove isolated window-state directory");
        }
    }

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

    #[test]
    fn writer_coalesces_geometry_bursts_and_flushes_latest_state() {
        let directory = TestDirectory::create();
        let path = directory.0.join("window-state.json");
        let writer = WindowStateWriter::start_for_path(
            path.clone(),
            WindowStateRecord::default(),
            Duration::from_millis(250),
        )
        .expect("start window-state writer");

        writer.schedule(state(1280, 840, 140, 120, false));
        writer.schedule(state(1360, 900, 180, 150, false));

        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        while writer
            .shared
            .write_attempts
            .load(std::sync::atomic::Ordering::Relaxed)
            == 0
            && std::time::Instant::now() < deadline
        {
            thread::sleep(Duration::from_millis(10));
        }

        assert_eq!(
            writer
                .shared
                .write_attempts
                .load(std::sync::atomic::Ordering::Relaxed),
            1
        );
        assert_eq!(
            read_window_state(&path),
            Some(state(1360, 900, 180, 150, false))
        );

        writer.schedule(state(1440, 960, 220, 180, false));
        writer.shutdown_and_flush();

        assert_eq!(
            writer
                .shared
                .write_attempts
                .load(std::sync::atomic::Ordering::Relaxed),
            2
        );
        assert_eq!(
            read_window_state(&path),
            Some(state(1440, 960, 220, 180, false))
        );
    }

    #[test]
    fn writer_flush_acknowledges_a_successful_save() {
        let directory = TestDirectory::create();
        let path = directory.0.join("window-state.json");
        let writer = WindowStateWriter::start_for_path(
            path.clone(),
            WindowStateRecord::default(),
            Duration::from_secs(60),
        )
        .expect("start window-state writer");
        let expected = state(1280, 840, 140, 120, false);
        writer.schedule(expected);

        assert_eq!(writer.request_flush(None, true), Ok(()));
        assert_eq!(read_window_state(&path), Some(expected));
        assert_eq!(
            writer
                .shared
                .write_attempts
                .load(std::sync::atomic::Ordering::Relaxed),
            1
        );
    }

    #[test]
    fn writer_flush_reports_a_save_failure_without_retrying_in_a_loop() {
        let directory = TestDirectory::create();
        let blocker = directory.0.join("not-a-directory");
        fs::write(&blocker, b"blocker").expect("create non-directory parent");
        let path = blocker.join("window-state.json");
        let writer = WindowStateWriter::start_for_path(
            path,
            WindowStateRecord::default(),
            Duration::from_secs(60),
        )
        .expect("start window-state writer");
        writer.schedule(state(1280, 840, 140, 120, false));

        assert_eq!(
            writer.request_flush(None, true),
            Err(WindowStateWriteFailure::CreateDirectory)
        );
        assert_eq!(
            writer
                .shared
                .write_attempts
                .load(std::sync::atomic::Ordering::Relaxed),
            1
        );
        assert_eq!(
            WindowStateWriteFailure::CreateDirectory.diagnostic_code(),
            "create_directory"
        );
    }
}
