//! Windows desktop observation and input for the model-only `computer_use` tool.

use std::{
    io::Write,
    mem::{size_of, zeroed},
    slice,
    sync::{
        Arc, Mutex, OnceLock, RwLock, Weak,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread::{self, JoinHandle},
    time::Instant,
};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use flate2::{Compression, write::ZlibEncoder};
use serde_json::{Value, json};
use uuid::Uuid;
use windows_sys::Win32::{
    Foundation::{HWND, RECT},
    Graphics::Gdi::{
        BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BitBlt, CAPTUREBLT, CreateCompatibleDC,
        CreateDIBSection, DIB_RGB_COLORS, DeleteDC, DeleteObject, EnumDisplayMonitors, GetDC,
        ReleaseDC, SRCCOPY, SelectObject,
    },
    System::Threading::GetCurrentThreadId,
    UI::{
        HiDpi::{DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetThreadDpiAwarenessContext},
        Input::KeyboardAndMouse::{
            INPUT, INPUT_0, INPUT_KEYBOARD, INPUT_MOUSE, KEYBDINPUT, KEYEVENTF_KEYUP,
            KEYEVENTF_UNICODE, MOUSEEVENTF_ABSOLUTE, MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP,
            MOUSEEVENTF_MIDDLEDOWN, MOUSEEVENTF_MIDDLEUP, MOUSEEVENTF_MOVE, MOUSEEVENTF_RIGHTDOWN,
            MOUSEEVENTF_RIGHTUP, MOUSEEVENTF_VIRTUALDESK, MOUSEEVENTF_WHEEL, MOUSEINPUT, SendInput,
            VK_BACK, VK_DELETE, VK_DOWN, VK_END, VK_ESCAPE, VK_HOME, VK_LEFT, VK_NEXT, VK_PRIOR,
            VK_RETURN, VK_RIGHT, VK_SPACE, VK_TAB, VK_UP,
        },
        WindowsAndMessaging::{
            CallNextHookEx, GetMessageW, GetSystemMetrics, HHOOK, KBDLLHOOKSTRUCT, LLKHF_INJECTED,
            MSG, PM_NOREMOVE, PeekMessageW, PostThreadMessageW, SM_CXVIRTUALSCREEN,
            SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN, SetWindowsHookExW,
            UnhookWindowsHookEx, WH_KEYBOARD_LL, WM_KEYDOWN, WM_KEYUP, WM_QUIT, WM_SYSKEYDOWN,
            WM_SYSKEYUP,
        },
    },
};

const MAX_CAPTURE_EDGE: i32 = 1600;
const MAX_SCREENSHOT_BYTES: usize = 8 * 1024 * 1024;
const MAX_TEXT_BYTES: usize = 2_000;

#[derive(Default)]
pub(crate) struct ComputerUseSession {
    frame: Mutex<Option<ScreenFrame>>,
    stopped: Arc<AtomicBool>,
}

static ACTIVE_SESSIONS: OnceLock<RwLock<Vec<Weak<AtomicBool>>>> = OnceLock::new();
static LAST_ESCAPE: Mutex<Option<Instant>> = Mutex::new(None);
static ESCAPE_KEY_DOWN: AtomicBool = AtomicBool::new(false);
static ESCAPE_MONITOR: OnceLock<Mutex<Option<EscapeMonitor>>> = OnceLock::new();

struct EscapeMonitor {
    thread_id: u32,
    thread: JoinHandle<bool>,
}

#[derive(Clone)]
struct ScreenFrame {
    id: String,
    image_width: i32,
    image_height: i32,
    virtual_bounds: RECT,
}

pub(crate) struct ComputerUseResult {
    pub(crate) output: Value,
    pub(crate) image: Option<String>,
}

impl ComputerUseSession {
    pub(crate) fn execute(&self, arguments: &Value) -> ComputerUseResult {
        if let Err((code, message)) = self.register_escape_stop() {
            return ComputerUseResult {
                output: json!({"error": {"code": code, "message": message}}),
                image: None,
            };
        }
        let result = self.execute_inner(arguments);
        match result {
            Ok(result) => result,
            Err((code, message)) => ComputerUseResult {
                output: json!({"error": {"code": code, "message": message}}),
                image: None,
            },
        }
    }

    fn register_escape_stop(&self) -> Result<(), (&'static str, &'static str)> {
        let monitors = ESCAPE_MONITOR.get_or_init(|| Mutex::new(None));
        let mut monitor = monitors.lock().map_err(|_| {
            (
                "computer_use_unavailable",
                "Computer Use state is unavailable.",
            )
        })?;
        if monitor.is_none() {
            *monitor = Some(
                start_escape_monitor().map_err(|message| ("computer_use_unavailable", message))?,
            );
        }
        let sessions = ACTIVE_SESSIONS.get_or_init(|| RwLock::new(Vec::new()));
        let mut sessions = sessions.write().map_err(|_| {
            (
                "computer_use_unavailable",
                "Computer Use state is unavailable.",
            )
        })?;
        if !sessions.iter().any(|session| {
            session
                .upgrade()
                .is_some_and(|session| Arc::ptr_eq(&session, &self.stopped))
        }) {
            sessions.push(Arc::downgrade(&self.stopped));
        }
        Ok(())
    }

    fn execute_inner(
        &self,
        arguments: &Value,
    ) -> Result<ComputerUseResult, (&'static str, &'static str)> {
        if self.stopped.load(Ordering::Acquire) {
            return Err((
                "computer_use_stopped",
                "Computer Use was stopped. Start a new chat request to use the desktop again.",
            ));
        }
        let action = arguments
            .get("action")
            .and_then(Value::as_str)
            .ok_or(("invalid_tool_input", "The Computer Use action is missing."))?;

        match action {
            "list_displays" => {
                require_only(arguments, &["action"])?;
                let displays = enumerate_displays()?;
                Ok(ComputerUseResult {
                    output: json!({"displays": displays.iter().enumerate().map(|(index, bounds)| json!({
                        "displayId": index.to_string(),
                        "bounds": {
                            "left": bounds.left,
                            "top": bounds.top,
                            "width": bounds.right - bounds.left,
                            "height": bounds.bottom - bounds.top
                        }
                    })).collect::<Vec<_>>()}),
                    image: None,
                })
            }
            "screenshot" | "desktop" => {
                require_only(arguments, &["action", "displayId"])?;
                let display_id = arguments.get("displayId").and_then(Value::as_str);
                let capture = capture_desktop(display_id)?;
                let frame_id = Uuid::new_v4().simple().to_string();
                let frame = ScreenFrame {
                    id: frame_id.clone(),
                    image_width: capture.image_width,
                    image_height: capture.image_height,
                    virtual_bounds: capture.virtual_bounds,
                };
                *self.frame.lock().map_err(|_| {
                    (
                        "computer_use_unavailable",
                        "Computer Use state is unavailable.",
                    )
                })? = Some(frame);
                Ok(ComputerUseResult {
                    output: json!({
                        "frameId": frame_id,
                        "width": capture.image_width,
                        "height": capture.image_height,
                        "displayId": display_id.unwrap_or("all"),
                        "displayBounds": {
                            "left": capture.virtual_bounds.left,
                            "top": capture.virtual_bounds.top,
                            "width": capture.virtual_bounds.right - capture.virtual_bounds.left,
                            "height": capture.virtual_bounds.bottom - capture.virtual_bounds.top
                        }
                    }),
                    image: Some(capture.data_url),
                })
            }
            "click" | "double_click" | "move" => {
                require_only(arguments, &["action", "x", "y", "frameId", "button"])?;
                let frame = self.frame_for(arguments)?;
                let point = map_point(
                    &frame,
                    read_coordinate(arguments, "x")?,
                    read_coordinate(arguments, "y")?,
                )?;
                let button = arguments
                    .get("button")
                    .and_then(Value::as_str)
                    .unwrap_or("left");
                match action {
                    "move" => send_mouse(point, MOUSEEVENTF_MOVE)?,
                    "click" => click(point, button)?,
                    _ => {
                        click(point, button)?;
                        click(point, button)?;
                    }
                }
                Ok(ComputerUseResult {
                    output: json!({"ok": true}),
                    image: None,
                })
            }
            "drag" => {
                require_only(arguments, &["action", "x", "y", "endX", "endY", "frameId"])?;
                let frame = self.frame_for(arguments)?;
                let start = map_point(
                    &frame,
                    read_coordinate(arguments, "x")?,
                    read_coordinate(arguments, "y")?,
                )?;
                let end = map_point(
                    &frame,
                    read_coordinate(arguments, "endX")?,
                    read_coordinate(arguments, "endY")?,
                )?;
                drag(start, end)?;
                Ok(ComputerUseResult {
                    output: json!({"ok": true}),
                    image: None,
                })
            }
            "scroll" => {
                require_only(arguments, &["action", "x", "y", "frameId", "scrollY"])?;
                let frame = self.frame_for(arguments)?;
                let point = map_point(
                    &frame,
                    read_coordinate(arguments, "x")?,
                    read_coordinate(arguments, "y")?,
                )?;
                let delta = arguments
                    .get("scrollY")
                    .and_then(Value::as_i64)
                    .unwrap_or(0);
                if !(-1000..=1000).contains(&delta) || delta == 0 {
                    return Err((
                        "invalid_tool_input",
                        "The scroll amount must be between -1000 and 1000 and cannot be zero.",
                    ));
                }
                scroll(point, delta as i32)?;
                Ok(ComputerUseResult {
                    output: json!({"ok": true}),
                    image: None,
                })
            }
            "type_text" => {
                require_only(arguments, &["action", "text"])?;
                let text = arguments
                    .get("text")
                    .and_then(Value::as_str)
                    .ok_or(("invalid_tool_input", "Text is required."))?;
                if text.is_empty()
                    || text.len() > MAX_TEXT_BYTES
                    || text.chars().any(char::is_control)
                {
                    return Err((
                        "invalid_tool_input",
                        "Text must contain 1 to 2000 printable UTF-8 bytes.",
                    ));
                }
                type_text(text)?;
                Ok(ComputerUseResult {
                    output: json!({"ok": true, "characters": text.chars().count()}),
                    image: None,
                })
            }
            "press_key" => {
                require_only(arguments, &["action", "key"])?;
                let key = arguments
                    .get("key")
                    .and_then(Value::as_str)
                    .ok_or(("invalid_tool_input", "A key is required."))?;
                press_key(key)?;
                Ok(ComputerUseResult {
                    output: json!({"ok": true}),
                    image: None,
                })
            }
            "hotkey" => {
                require_only(arguments, &["action", "keys"])?;
                let keys = arguments
                    .get("keys")
                    .and_then(Value::as_array)
                    .ok_or(("invalid_tool_input", "A key array is required."))?;
                if keys.is_empty() || keys.len() > 4 {
                    return Err((
                        "invalid_tool_input",
                        "A hotkey must contain between one and four keys.",
                    ));
                }
                let virtual_keys = keys
                    .iter()
                    .map(|key| {
                        key.as_str()
                            .ok_or(("invalid_tool_input", "Hotkey names must be text."))
                            .and_then(key_code)
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                press_hotkey(&virtual_keys)?;
                Ok(ComputerUseResult {
                    output: json!({"ok": true}),
                    image: None,
                })
            }
            _ => Err((
                "invalid_tool_input",
                "Unsupported Computer Use action. Use screenshot (or desktop) to view all displays, and use hotkey with keys [\"alt\", \"tab\"] for Alt+Tab.",
            )),
        }
    }

    fn frame_for(&self, arguments: &Value) -> Result<ScreenFrame, (&'static str, &'static str)> {
        let requested = arguments.get("frameId").and_then(Value::as_str).ok_or((
            "stale_screen_frame",
            "Take a screenshot before sending a coordinate action.",
        ))?;
        let frame = self.frame.lock().map_err(|_| {
            (
                "computer_use_unavailable",
                "Computer Use state is unavailable.",
            )
        })?;
        let Some(frame) = frame.as_ref().filter(|frame| frame.id == requested) else {
            return Err((
                "stale_screen_frame",
                "This coordinate action does not match the latest screenshot. Take a new screenshot.",
            ));
        };
        Ok(frame.clone())
    }
}

impl Drop for ComputerUseSession {
    fn drop(&mut self) {
        unregister_escape_stop(&self.stopped);
    }
}

fn unregister_escape_stop(stopped: &Arc<AtomicBool>) {
    let Some(monitors) = ESCAPE_MONITOR.get() else {
        return;
    };
    let Ok(mut monitor) = monitors.lock() else {
        return;
    };
    let Some(sessions) = ACTIVE_SESSIONS.get() else {
        return;
    };
    let Ok(mut sessions) = sessions.write() else {
        return;
    };
    sessions.retain(|session| {
        session
            .upgrade()
            .is_some_and(|session| !Arc::ptr_eq(&session, stopped))
    });
    let has_active_sessions = sessions.iter().any(|session| session.strong_count() > 0);
    drop(sessions);
    if !has_active_sessions && let Some(active_monitor) = monitor.as_ref() {
        let posted = unsafe { PostThreadMessageW(active_monitor.thread_id, WM_QUIT, 0, 0) };
        if posted != 0
            && let Some(active_monitor) = monitor.take()
        {
            match active_monitor.thread.join() {
                Ok(true) => {}
                Ok(false) | Err(_) => eprintln!("computer_use_escape_hook_cleanup_failed"),
            }
        } else if posted == 0 {
            eprintln!("computer_use_escape_hook_stop_signal_failed");
        }
    }
}

fn start_escape_monitor() -> Result<EscapeMonitor, &'static str> {
    let (ready_tx, ready_rx) = mpsc::sync_channel(1);
    let thread = thread::Builder::new()
        .name("openchat-computer-use-escape".to_owned())
        .spawn(move || unsafe {
            let thread_id = GetCurrentThreadId();
            let mut message: MSG = zeroed();
            PeekMessageW(&mut message, std::ptr::null_mut(), 0, 0, PM_NOREMOVE);
            let hook =
                SetWindowsHookExW(WH_KEYBOARD_LL, Some(escape_hook), std::ptr::null_mut(), 0);
            if hook.is_null() {
                let _ = ready_tx.send(None);
                return false;
            }
            let _ = ready_tx.send(Some(thread_id));
            while GetMessageW(&mut message, std::ptr::null_mut(), 0, 0) > 0 {}
            UnhookWindowsHookEx(hook) != 0
        })
        .map_err(|_| "The Escape stop listener could not be started.")?;
    match ready_rx.recv() {
        Ok(Some(thread_id)) => Ok(EscapeMonitor { thread_id, thread }),
        Ok(None) => Err("The Escape stop listener could not be installed."),
        Err(_) => Err("The Escape stop listener stopped unexpectedly."),
    }
}

unsafe extern "system" fn escape_hook(code: i32, wparam: usize, lparam: isize) -> isize {
    if code >= 0 {
        let key = unsafe { &*(lparam as *const KBDLLHOOKSTRUCT) };
        if key.vkCode == VK_ESCAPE as u32
            && key.flags & LLKHF_INJECTED == 0
            && (wparam == WM_KEYUP as usize || wparam == WM_SYSKEYUP as usize)
        {
            ESCAPE_KEY_DOWN.store(false, Ordering::Release);
        }
        if key.vkCode == VK_ESCAPE as u32
            && key.flags & LLKHF_INJECTED == 0
            && (wparam == WM_KEYDOWN as usize || wparam == WM_SYSKEYDOWN as usize)
            && ESCAPE_KEY_DOWN
                .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
        {
            let now = Instant::now();
            let is_second_press = LAST_ESCAPE
                .lock()
                .map(|mut previous| {
                    let second = previous
                        .is_some_and(|instant| now.duration_since(instant).as_millis() <= 1500);
                    *previous = if second { None } else { Some(now) };
                    second
                })
                .unwrap_or(false);
            if is_second_press
                && let Some(sessions) = ACTIVE_SESSIONS.get()
                && let Ok(mut sessions) = sessions.write()
            {
                sessions.retain(|session| {
                    if let Some(session) = session.upgrade() {
                        session.store(true, Ordering::Release);
                        true
                    } else {
                        false
                    }
                });
            }
        }
    }
    unsafe { CallNextHookEx(std::ptr::null_mut::<HHOOK>().cast(), code, wparam, lparam) }
}

struct ScreenCapture {
    image_width: i32,
    image_height: i32,
    virtual_bounds: RECT,
    data_url: String,
}

fn require_only(arguments: &Value, allowed: &[&str]) -> Result<(), (&'static str, &'static str)> {
    let Some(object) = arguments.as_object() else {
        return Err((
            "invalid_tool_input",
            "Computer Use arguments must be an object.",
        ));
    };
    if object.keys().all(|key| allowed.contains(&key.as_str())) {
        Ok(())
    } else {
        Err((
            "invalid_tool_input",
            "Computer Use arguments contain unsupported fields.",
        ))
    }
}

fn read_coordinate(arguments: &Value, name: &str) -> Result<i32, (&'static str, &'static str)> {
    arguments
        .get(name)
        .and_then(Value::as_i64)
        .and_then(|value| i32::try_from(value).ok())
        .filter(|value| *value >= 0)
        .ok_or((
            "invalid_tool_input",
            "Screen coordinates must be non-negative integers.",
        ))
}

fn map_point(
    frame: &ScreenFrame,
    x: i32,
    y: i32,
) -> Result<(i32, i32), (&'static str, &'static str)> {
    if x >= frame.image_width || y >= frame.image_height {
        return Err((
            "invalid_coordinates",
            "The target point is outside the latest screenshot.",
        ));
    }
    let display_width = frame.virtual_bounds.right - frame.virtual_bounds.left;
    let display_height = frame.virtual_bounds.bottom - frame.virtual_bounds.top;
    let physical_x = frame.virtual_bounds.left
        + ((i64::from(x) * i64::from(display_width)) / i64::from(frame.image_width)) as i32;
    let physical_y = frame.virtual_bounds.top
        + ((i64::from(y) * i64::from(display_height)) / i64::from(frame.image_height)) as i32;
    Ok((physical_x, physical_y))
}

fn enumerate_displays() -> Result<Vec<RECT>, (&'static str, &'static str)> {
    let mut displays = Vec::new();
    let success = unsafe {
        EnumDisplayMonitors(
            std::ptr::null_mut(),
            std::ptr::null(),
            Some(collect_display_bounds),
            (&mut displays as *mut Vec<RECT>) as isize,
        )
    };
    if success == 0 || displays.is_empty() {
        return Err(("screen_capture_unavailable", "No display is available."));
    }
    Ok(displays)
}

unsafe extern "system" fn collect_display_bounds(
    _monitor: windows_sys::Win32::Graphics::Gdi::HMONITOR,
    _dc: windows_sys::Win32::Graphics::Gdi::HDC,
    bounds: *mut RECT,
    data: isize,
) -> i32 {
    if bounds.is_null() || data == 0 {
        return 0;
    }
    let displays = unsafe { &mut *(data as *mut Vec<RECT>) };
    displays.push(unsafe { *bounds });
    1
}

fn capture_desktop(
    display_id: Option<&str>,
) -> Result<ScreenCapture, (&'static str, &'static str)> {
    unsafe {
        let previous_dpi_context =
            SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        let (left, top, width, height) = if let Some(display_id) = display_id {
            let Ok(index) = display_id.parse::<usize>() else {
                let _ = SetThreadDpiAwarenessContext(previous_dpi_context);
                return Err((
                    "invalid_tool_input",
                    "The displayId must come from list_displays.",
                ));
            };
            let displays = match enumerate_displays() {
                Ok(displays) => displays,
                Err(error) => {
                    let _ = SetThreadDpiAwarenessContext(previous_dpi_context);
                    return Err(error);
                }
            };
            let Some(bounds) = displays.get(index) else {
                let _ = SetThreadDpiAwarenessContext(previous_dpi_context);
                return Err((
                    "invalid_tool_input",
                    "The displayId must come from list_displays.",
                ));
            };
            (
                bounds.left,
                bounds.top,
                bounds.right - bounds.left,
                bounds.bottom - bounds.top,
            )
        } else {
            (
                GetSystemMetrics(SM_XVIRTUALSCREEN),
                GetSystemMetrics(SM_YVIRTUALSCREEN),
                GetSystemMetrics(SM_CXVIRTUALSCREEN),
                GetSystemMetrics(SM_CYVIRTUALSCREEN),
            )
        };
        if width <= 0 || height <= 0 || i64::from(width) * i64::from(height) > 20_000_000 {
            let _ = SetThreadDpiAwarenessContext(previous_dpi_context);
            return Err((
                "screen_capture_unavailable",
                "The desktop dimensions are unavailable or exceed the supported limit.",
            ));
        }
        let screen = GetDC(0 as HWND);
        if screen.is_null() {
            let _ = SetThreadDpiAwarenessContext(previous_dpi_context);
            return Err((
                "screen_capture_unavailable",
                "The desktop could not be captured.",
            ));
        }
        let memory = CreateCompatibleDC(screen);
        if memory.is_null() {
            ReleaseDC(0 as HWND, screen);
            let _ = SetThreadDpiAwarenessContext(previous_dpi_context);
            return Err((
                "screen_capture_unavailable",
                "The desktop could not be captured.",
            ));
        }
        let mut info: BITMAPINFO = zeroed();
        info.bmiHeader = BITMAPINFOHEADER {
            biSize: size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: width,
            biHeight: -height,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB,
            ..zeroed()
        };
        let mut bits = std::ptr::null_mut();
        let bitmap = CreateDIBSection(
            screen,
            &info,
            DIB_RGB_COLORS,
            &mut bits,
            std::ptr::null_mut(),
            0,
        );
        if bitmap.is_null() || bits.is_null() {
            DeleteDC(memory);
            ReleaseDC(0 as HWND, screen);
            let _ = SetThreadDpiAwarenessContext(previous_dpi_context);
            return Err((
                "screen_capture_unavailable",
                "The desktop could not be captured.",
            ));
        }
        let previous_object = SelectObject(memory, bitmap);
        if BitBlt(
            memory,
            0,
            0,
            width,
            height,
            screen,
            left,
            top,
            SRCCOPY | CAPTUREBLT,
        ) == 0
        {
            SelectObject(memory, previous_object);
            DeleteObject(bitmap);
            DeleteDC(memory);
            ReleaseDC(0 as HWND, screen);
            let _ = SetThreadDpiAwarenessContext(previous_dpi_context);
            return Err((
                "screen_capture_unavailable",
                "The desktop could not be captured.",
            ));
        }
        let source =
            slice::from_raw_parts(bits.cast::<u8>(), (width as usize) * (height as usize) * 4);
        let (pixels, image_width, image_height) = scale_bgra(source, width, height);
        SelectObject(memory, previous_object);
        DeleteObject(bitmap);
        DeleteDC(memory);
        ReleaseDC(0 as HWND, screen);
        let _ = SetThreadDpiAwarenessContext(previous_dpi_context);
        let png = encode_png(&pixels, image_width as u32, image_height as u32).map_err(|_| {
            (
                "screen_capture_unavailable",
                "The desktop image could not be encoded.",
            )
        })?;
        if png.len() > MAX_SCREENSHOT_BYTES {
            return Err((
                "screen_capture_too_large",
                "The desktop image exceeds the supported size.",
            ));
        }
        Ok(ScreenCapture {
            image_width,
            image_height,
            virtual_bounds: RECT {
                left,
                top,
                right: left + width,
                bottom: top + height,
            },
            data_url: format!("data:image/png;base64,{}", STANDARD.encode(png)),
        })
    }
}

fn scale_bgra(source: &[u8], width: i32, height: i32) -> (Vec<u8>, i32, i32) {
    let scale = (width.max(height) as f64 / f64::from(MAX_CAPTURE_EDGE)).max(1.0);
    let scaled_width = (f64::from(width) / scale).round().max(1.0) as i32;
    let scaled_height = (f64::from(height) / scale).round().max(1.0) as i32;
    let mut rgb = Vec::with_capacity((scaled_width as usize) * (scaled_height as usize) * 3);
    for y in 0..scaled_height {
        let source_y = (i64::from(y) * i64::from(height) / i64::from(scaled_height)) as usize;
        for x in 0..scaled_width {
            let source_x = (i64::from(x) * i64::from(width) / i64::from(scaled_width)) as usize;
            let offset = (source_y * width as usize + source_x) * 4;
            rgb.extend_from_slice(&[source[offset + 2], source[offset + 1], source[offset]]);
        }
    }
    (rgb, scaled_width, scaled_height)
}

fn encode_png(rgb: &[u8], width: u32, height: u32) -> Result<Vec<u8>, std::io::Error> {
    let row_bytes = width as usize * 3;
    let mut scanlines = Vec::with_capacity((row_bytes + 1) * height as usize);
    for row in rgb.chunks_exact(row_bytes) {
        scanlines.push(0);
        scanlines.extend_from_slice(row);
    }
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::fast());
    encoder.write_all(&scanlines)?;
    let compressed = encoder.finish()?;
    let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.extend_from_slice(&[8, 2, 0, 0, 0]);
    append_png_chunk(&mut png, b"IHDR", &ihdr);
    append_png_chunk(&mut png, b"IDAT", &compressed);
    append_png_chunk(&mut png, b"IEND", &[]);
    Ok(png)
}

fn append_png_chunk(output: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    output.extend_from_slice(&(data.len() as u32).to_be_bytes());
    output.extend_from_slice(kind);
    output.extend_from_slice(data);
    let mut crc = 0xffff_ffffu32;
    for byte in kind.iter().chain(data) {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            crc = if crc & 1 == 1 {
                (crc >> 1) ^ 0xedb8_8320
            } else {
                crc >> 1
            };
        }
    }
    output.extend_from_slice(&(!crc).to_be_bytes());
}

fn absolute_mouse_input(
    point: (i32, i32),
    flags: u32,
) -> Result<INPUT, (&'static str, &'static str)> {
    let left = unsafe { GetSystemMetrics(SM_XVIRTUALSCREEN) };
    let top = unsafe { GetSystemMetrics(SM_YVIRTUALSCREEN) };
    let width = unsafe { GetSystemMetrics(SM_CXVIRTUALSCREEN) };
    let height = unsafe { GetSystemMetrics(SM_CYVIRTUALSCREEN) };
    if width <= 1 || height <= 1 {
        return Err((
            "desktop_unavailable",
            "The desktop dimensions are unavailable.",
        ));
    }
    let normalized_x =
        ((i64::from(point.0 - left) * 65535) / i64::from(width - 1)).clamp(0, 65535) as i32;
    let normalized_y =
        ((i64::from(point.1 - top) * 65535) / i64::from(height - 1)).clamp(0, 65535) as i32;
    Ok(INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 {
            mi: MOUSEINPUT {
                dx: normalized_x,
                dy: normalized_y,
                mouseData: 0,
                dwFlags: flags | MOUSEEVENTF_ABSOLUTE | MOUSEEVENTF_VIRTUALDESK,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    })
}

fn send_inputs(inputs: &[INPUT]) -> Result<(), (&'static str, &'static str)> {
    let inserted = unsafe {
        SendInput(
            inputs.len() as u32,
            inputs.as_ptr(),
            size_of::<INPUT>() as i32,
        )
    };
    if inserted as usize == inputs.len() {
        Ok(())
    } else {
        Err((
            "desktop_input_blocked",
            "Windows blocked the requested desktop input. The target may require a higher integrity level.",
        ))
    }
}

fn send_mouse(point: (i32, i32), flags: u32) -> Result<(), (&'static str, &'static str)> {
    send_inputs(&[absolute_mouse_input(point, flags)?])
}

fn click(point: (i32, i32), button: &str) -> Result<(), (&'static str, &'static str)> {
    let (down, up) = match button {
        "left" => (MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP),
        "right" => (MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP),
        "middle" => (MOUSEEVENTF_MIDDLEDOWN, MOUSEEVENTF_MIDDLEUP),
        _ => return Err(("invalid_tool_input", "The mouse button is not supported.")),
    };
    send_inputs(&[
        absolute_mouse_input(point, MOUSEEVENTF_MOVE)?,
        absolute_mouse_input(point, down)?,
        absolute_mouse_input(point, up)?,
    ])
}

fn drag(start: (i32, i32), end: (i32, i32)) -> Result<(), (&'static str, &'static str)> {
    send_inputs(&[
        absolute_mouse_input(start, MOUSEEVENTF_MOVE | MOUSEEVENTF_LEFTDOWN)?,
        absolute_mouse_input(end, MOUSEEVENTF_MOVE)?,
        absolute_mouse_input(end, MOUSEEVENTF_LEFTUP)?,
    ])
}

fn scroll(point: (i32, i32), delta: i32) -> Result<(), (&'static str, &'static str)> {
    let mut input = absolute_mouse_input(point, MOUSEEVENTF_MOVE | MOUSEEVENTF_WHEEL)?;
    input.Anonymous.mi.mouseData = (delta * 120) as u32;
    send_inputs(&[input])
}

fn type_text(text: &str) -> Result<(), (&'static str, &'static str)> {
    let mut inputs = Vec::with_capacity(text.encode_utf16().count() * 2);
    for unit in text.encode_utf16() {
        inputs.push(keyboard_input(unit, KEYEVENTF_UNICODE));
        inputs.push(keyboard_input(unit, KEYEVENTF_UNICODE | KEYEVENTF_KEYUP));
    }
    send_inputs(&inputs)
}

fn press_key(key: &str) -> Result<(), (&'static str, &'static str)> {
    let code = key_code(key)?;
    press_hotkey(&[code])
}

fn press_hotkey(keys: &[u16]) -> Result<(), (&'static str, &'static str)> {
    let mut inputs = Vec::with_capacity(keys.len() * 2);
    for key in keys {
        inputs.push(keyboard_input(*key, 0));
    }
    for key in keys.iter().rev() {
        inputs.push(keyboard_input(*key, KEYEVENTF_KEYUP));
    }
    send_inputs(&inputs)
}

fn keyboard_input(vk: u16, flags: u32) -> INPUT {
    let is_unicode = flags & KEYEVENTF_UNICODE != 0;
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: if is_unicode { 0 } else { vk },
                wScan: if is_unicode { vk } else { 0 },
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

fn key_code(key: &str) -> Result<u16, (&'static str, &'static str)> {
    let key = key.to_ascii_lowercase();
    let key = match key.as_str() {
        "enter" | "return" => VK_RETURN as u16,
        "escape" | "esc" => VK_ESCAPE as u16,
        "tab" => VK_TAB as u16,
        "space" => VK_SPACE as u16,
        "backspace" => VK_BACK as u16,
        "delete" => VK_DELETE as u16,
        "home" => VK_HOME as u16,
        "end" => VK_END as u16,
        "pageup" => VK_PRIOR as u16,
        "pagedown" => VK_NEXT as u16,
        "up" => VK_UP as u16,
        "down" => VK_DOWN as u16,
        "left" => VK_LEFT as u16,
        "right" => VK_RIGHT as u16,
        "ctrl" | "control" => 0x11,
        "shift" => 0x10,
        "alt" => 0x12,
        "win" | "meta" => 0x5b,
        _ if key.len() == 1 && key.as_bytes()[0].is_ascii_alphanumeric() => {
            key.as_bytes()[0].to_ascii_uppercase() as u16
        }
        _ => return Err(("invalid_tool_input", "The key name is not supported.")),
    };
    Ok(key)
}

#[cfg(test)]
mod tests {
    use super::{ScreenFrame, key_code, map_point, require_only};
    use serde_json::json;
    use windows_sys::Win32::Foundation::RECT;

    #[test]
    fn coordinate_mapping_accounts_for_scaled_multi_monitor_bounds() {
        let frame = ScreenFrame {
            id: "frame".to_owned(),
            image_width: 1600,
            image_height: 900,
            virtual_bounds: RECT {
                left: -1920,
                top: 0,
                right: 1920,
                bottom: 2160,
            },
        };

        assert_eq!(map_point(&frame, 400, 225), Ok((-960, 540)));
    }

    #[test]
    fn tool_arguments_reject_fields_outside_the_action_contract() {
        assert!(require_only(&json!({"action": "screenshot"}), &["action"]).is_ok());
        assert!(
            require_only(
                &json!({"action": "screenshot", "unexpected": true}),
                &["action"]
            )
            .is_err()
        );
    }

    #[test]
    fn key_names_are_normalized_and_unknown_keys_are_rejected() {
        assert_eq!(key_code("ENTER"), Ok(0x0d));
        assert_eq!(key_code("a"), Ok(b'A' as u16));
        assert!(key_code("launch-control-panel").is_err());
    }
}
