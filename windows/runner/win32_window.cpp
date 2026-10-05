#include "win32_window.h"

#include <dwmapi.h>
#include <flutter_windows.h>

#include <algorithm>
#include <optional>

#include "resource.h"

namespace {

/// Window attribute that enables dark mode window decorations.
///
/// Redefined in case the developer's machine has a Windows SDK older than
/// version 10.0.22000.0.
/// See: https://docs.microsoft.com/windows/win32/api/dwmapi/ne-dwmapi-dwmwindowattribute
#ifndef DWMWA_USE_IMMERSIVE_DARK_MODE
#define DWMWA_USE_IMMERSIVE_DARK_MODE 20
#endif

constexpr const wchar_t kWindowClassName[] = L"FLUTTER_RUNNER_WIN32_WINDOW";
constexpr LONG_PTR kFramelessWindowStyleMask = WS_CAPTION | WS_THICKFRAME;
constexpr LONG_PTR kFramelessWindowExStyleMask = WS_EX_WINDOWEDGE;

/// Registry key for app theme preference.
///
/// A value of 0 indicates apps should use dark mode. A non-zero or missing
/// value indicates apps should use light mode.
constexpr const wchar_t kGetPreferredBrightnessRegKey[] =
  L"Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize";
constexpr const wchar_t kGetPreferredBrightnessRegValue[] = L"AppsUseLightTheme";

// The number of Win32Window objects that currently exist.
static int g_active_window_count = 0;

using EnableNonClientDpiScaling = BOOL __stdcall(HWND hwnd);

// Scale helper to convert logical scaler values to physical using passed in
// scale factor
int Scale(int source, double scale_factor) {
  return static_cast<int>(source * scale_factor);
}

// Frameless windows must handle resize hit tests before the Flutter child view.
std::optional<LRESULT> GetResizeHitTest(HWND window, LPARAM lparam) {
  if (IsZoomed(window)) {
    return std::nullopt;
  }

  RECT bounds;
  if (!GetWindowRect(window, &bounds)) {
    return std::nullopt;
  }

  const int x = static_cast<int>(static_cast<short>(LOWORD(lparam)));
  const int y = static_cast<int>(static_cast<short>(HIWORD(lparam)));
  const int resize_border = GetSystemMetrics(SM_CXSIZEFRAME) +
                            GetSystemMetrics(SM_CXPADDEDBORDER);
  const bool left = x < bounds.left + resize_border;
  const bool right = x >= bounds.right - resize_border;
  const bool top = y < bounds.top + resize_border;
  const bool bottom = y >= bounds.bottom - resize_border;

  if (top && left) return HTTOPLEFT;
  if (top && right) return HTTOPRIGHT;
  if (bottom && left) return HTBOTTOMLEFT;
  if (bottom && right) return HTBOTTOMRIGHT;
  if (left) return HTLEFT;
  if (right) return HTRIGHT;
  if (top) return HTTOP;
  if (bottom) return HTBOTTOM;
  return std::nullopt;
}

// Dynamically loads the |EnableNonClientDpiScaling| from the User32 module.
// This API is only needed for PerMonitor V1 awareness mode.
void EnableFullDpiSupportIfAvailable(HWND hwnd) {
  HMODULE user32_module = LoadLibraryA("User32.dll");
  if (!user32_module) {
    return;
  }
  auto enable_non_client_dpi_scaling =
      reinterpret_cast<EnableNonClientDpiScaling*>(
          GetProcAddress(user32_module, "EnableNonClientDpiScaling"));
  if (enable_non_client_dpi_scaling != nullptr) {
    enable_non_client_dpi_scaling(hwnd);
  }
  FreeLibrary(user32_module);
}

}  // namespace

// Manages the Win32Window's window class registration.
class WindowClassRegistrar {
 public:
  ~WindowClassRegistrar() = default;

  // Returns the singleton registrar instance.
  static WindowClassRegistrar* GetInstance() {
    if (!instance_) {
      instance_ = new WindowClassRegistrar();
    }
    return instance_;
  }

  // Returns the name of the window class, registering the class if it hasn't
  // previously been registered.
  const wchar_t* GetWindowClass();

  // Unregisters the window class. Should only be called if there are no
  // instances of the window.
  void UnregisterWindowClass();

 private:
  WindowClassRegistrar() = default;

  static WindowClassRegistrar* instance_;

  bool class_registered_ = false;
};

WindowClassRegistrar* WindowClassRegistrar::instance_ = nullptr;

const wchar_t* WindowClassRegistrar::GetWindowClass() {
  if (!class_registered_) {
    WNDCLASS window_class{};
    window_class.hCursor = LoadCursor(nullptr, IDC_ARROW);
    window_class.lpszClassName = kWindowClassName;
    window_class.style = CS_HREDRAW | CS_VREDRAW;
    window_class.cbClsExtra = 0;
    window_class.cbWndExtra = 0;
    window_class.hInstance = GetModuleHandle(nullptr);
    window_class.hIcon =
        LoadIcon(window_class.hInstance, MAKEINTRESOURCE(IDI_APP_ICON));
    window_class.hbrBackground = 0;
    window_class.lpszMenuName = nullptr;
    window_class.lpfnWndProc = Win32Window::WndProc;
    RegisterClass(&window_class);
    class_registered_ = true;
  }
  return kWindowClassName;
}

void WindowClassRegistrar::UnregisterWindowClass() {
  UnregisterClass(kWindowClassName, nullptr);
  class_registered_ = false;
}

Win32Window::Win32Window() {
  ++g_active_window_count;
}

Win32Window::~Win32Window() {
  --g_active_window_count;
  Destroy();
}

bool Win32Window::Create(const std::wstring& title,
                         const Point& origin,
                         const Size& size) {
  Destroy();

  const wchar_t* window_class =
      WindowClassRegistrar::GetInstance()->GetWindowClass();

  const POINT target_point = {static_cast<LONG>(origin.x),
                              static_cast<LONG>(origin.y)};
  HMONITOR monitor = MonitorFromPoint(target_point, MONITOR_DEFAULTTONEAREST);
  UINT dpi = FlutterDesktopGetDpiForMonitor(monitor);
  double scale_factor = dpi / 96.0;

  const DWORD window_style =
      WS_OVERLAPPEDWINDOW & ~static_cast<DWORD>(kFramelessWindowStyleMask);
  HWND window = CreateWindow(
      window_class, title.c_str(), window_style,
      Scale(origin.x, scale_factor), Scale(origin.y, scale_factor),
      Scale(size.width, scale_factor), Scale(size.height, scale_factor),
      nullptr, nullptr, GetModuleHandle(nullptr), this);

  if (!window) {
    return false;
  }

  UpdateTheme(window);

  if (!OnCreate()) {
    return false;
  }

  // Recalculate the non-client area after Flutter initialization so the
  // window keeps using the frameless client area from its first visible frame.
  const LONG_PTR initialized_style = GetWindowLongPtr(window, GWL_STYLE);
  const LONG_PTR frameless_style =
      initialized_style & ~kFramelessWindowStyleMask;
  if (initialized_style != frameless_style) {
    SetLastError(ERROR_SUCCESS);
    const LONG_PTR previous_style = SetWindowLongPtr(
        window, GWL_STYLE, frameless_style);
    if (previous_style == 0 && GetLastError() != ERROR_SUCCESS) {
      return false;
    }
  }

  const LONG_PTR initialized_ex_style = GetWindowLongPtr(window, GWL_EXSTYLE);
  const LONG_PTR frameless_ex_style =
      initialized_ex_style & ~kFramelessWindowExStyleMask;
  if (initialized_ex_style != frameless_ex_style) {
    SetLastError(ERROR_SUCCESS);
    const LONG_PTR previous_ex_style = SetWindowLongPtr(
        window, GWL_EXSTYLE, frameless_ex_style);
    if (previous_ex_style == 0 && GetLastError() != ERROR_SUCCESS) {
      return false;
    }
  }

  if (!SetWindowPos(window, nullptr, 0, 0, 0, 0,
                    SWP_FRAMECHANGED | SWP_NOACTIVATE | SWP_NOMOVE |
                        SWP_NOSIZE | SWP_NOZORDER)) {
    return false;
  }

  return true;
}

bool Win32Window::Show() {
  return ShowWindow(window_handle_, SW_SHOWMAXIMIZED);
}

// static
LRESULT CALLBACK Win32Window::WndProc(HWND const window,
                                      UINT const message,
                                      WPARAM const wparam,
                                      LPARAM const lparam) noexcept {
  if (message == WM_NCCREATE) {
    auto window_struct = reinterpret_cast<CREATESTRUCT*>(lparam);
    SetWindowLongPtr(window, GWLP_USERDATA,
                     reinterpret_cast<LONG_PTR>(window_struct->lpCreateParams));

    auto that = static_cast<Win32Window*>(window_struct->lpCreateParams);
    EnableFullDpiSupportIfAvailable(window);
    that->window_handle_ = window;
  } else {
    // Keep frame messages under the native window procedure's control before
    // Flutter can replace their result.
    if (message == WM_NCCALCSIZE) {
      return 0;
    }

    if (message == WM_GETMINMAXINFO && lparam != 0) {
      const HMONITOR monitor =
          MonitorFromWindow(window, MONITOR_DEFAULTTONEAREST);
      MONITORINFO monitor_info{};
      monitor_info.cbSize = sizeof(monitor_info);
      if (GetMonitorInfo(monitor, &monitor_info)) {
        auto min_max_info = reinterpret_cast<MINMAXINFO*>(lparam);
        min_max_info->ptMaxPosition.x =
            monitor_info.rcWork.left - monitor_info.rcMonitor.left;
        min_max_info->ptMaxPosition.y =
            monitor_info.rcWork.top - monitor_info.rcMonitor.top;
        min_max_info->ptMaxSize.x =
            monitor_info.rcWork.right - monitor_info.rcWork.left;
        min_max_info->ptMaxSize.y =
            monitor_info.rcWork.bottom - monitor_info.rcWork.top;
        const UINT dpi = FlutterDesktopGetDpiForMonitor(monitor);
        const double scale_factor = dpi / 96.0;
        // Keep the navigation rail and conversation header usable after restore.
        min_max_info->ptMinTrackSize.x = std::min(
            static_cast<LONG>(Scale(900, scale_factor)),
            min_max_info->ptMaxSize.x);
        min_max_info->ptMinTrackSize.y = std::min(
            static_cast<LONG>(Scale(560, scale_factor)),
            min_max_info->ptMaxSize.y);
        return 0;
      }
    }

    if (message == WM_NCHITTEST) {
      if (const auto hit_test = GetResizeHitTest(window, lparam)) {
        return *hit_test;
      }
    }

    if (message == WM_STYLECHANGING && lparam != 0 &&
        (wparam == GWL_STYLE || wparam == GWL_EXSTYLE)) {
      auto style = reinterpret_cast<STYLESTRUCT*>(lparam);
      if (wparam == GWL_STYLE) {
        style->styleNew &= ~static_cast<DWORD>(kFramelessWindowStyleMask);
      } else {
        style->styleNew &= ~static_cast<DWORD>(kFramelessWindowExStyleMask);
      }
      return 0;
    }

    if (message == WM_STYLECHANGED &&
        (wparam == GWL_STYLE || wparam == GWL_EXSTYLE)) {
      SetWindowPos(window, nullptr, 0, 0, 0, 0,
                   SWP_FRAMECHANGED | SWP_NOACTIVATE | SWP_NOMOVE |
                       SWP_NOSIZE | SWP_NOZORDER);
    }

    if (Win32Window* that = GetThisFromHandle(window)) {
      return that->MessageHandler(window, message, wparam, lparam);
    }
  }

  return DefWindowProc(window, message, wparam, lparam);
}

LRESULT
Win32Window::MessageHandler(HWND hwnd,
                            UINT const message,
                            WPARAM const wparam,
                            LPARAM const lparam) noexcept {
  switch (message) {
    case WM_DESTROY:
      window_handle_ = nullptr;
      Destroy();
      if (quit_on_close_) {
        PostQuitMessage(0);
      }
      return 0;

    case WM_DPICHANGED: {
      auto newRectSize = reinterpret_cast<RECT*>(lparam);
      LONG newWidth = newRectSize->right - newRectSize->left;
      LONG newHeight = newRectSize->bottom - newRectSize->top;

      SetWindowPos(hwnd, nullptr, newRectSize->left, newRectSize->top, newWidth,
                   newHeight, SWP_NOZORDER | SWP_NOACTIVATE);

      return 0;
    }
    case WM_SIZE: {
      RECT rect = GetClientArea();
      if (child_content_ != nullptr) {
        // Size and position the child window.
        MoveWindow(child_content_, rect.left, rect.top, rect.right - rect.left,
                   rect.bottom - rect.top, TRUE);
      }
      return 0;
    }

    case WM_ACTIVATE:
      if (child_content_ != nullptr) {
        SetFocus(child_content_);
      }
      return 0;

    case WM_DWMCOLORIZATIONCOLORCHANGED:
      UpdateTheme(hwnd);
      return 0;
  }

  return DefWindowProc(window_handle_, message, wparam, lparam);
}

void Win32Window::Destroy() {
  OnDestroy();

  if (window_handle_) {
    DestroyWindow(window_handle_);
    window_handle_ = nullptr;
  }
  if (g_active_window_count == 0) {
    WindowClassRegistrar::GetInstance()->UnregisterWindowClass();
  }
}

Win32Window* Win32Window::GetThisFromHandle(HWND const window) noexcept {
  return reinterpret_cast<Win32Window*>(
      GetWindowLongPtr(window, GWLP_USERDATA));
}

void Win32Window::SetChildContent(HWND content) {
  child_content_ = content;
  SetParent(content, window_handle_);
  RECT frame = GetClientArea();

  MoveWindow(content, frame.left, frame.top, frame.right - frame.left,
             frame.bottom - frame.top, true);

  SetFocus(child_content_);
}

RECT Win32Window::GetClientArea() {
  RECT frame;
  GetClientRect(window_handle_, &frame);
  return frame;
}

HWND Win32Window::GetHandle() {
  return window_handle_;
}

void Win32Window::SetQuitOnClose(bool quit_on_close) {
  quit_on_close_ = quit_on_close;
}

bool Win32Window::OnCreate() {
  // No-op; provided for subclasses.
  return true;
}

void Win32Window::OnDestroy() {
  // No-op; provided for subclasses.
}

void Win32Window::UpdateTheme(HWND const window) {
  DWORD light_mode;
  DWORD light_mode_size = sizeof(light_mode);
  LSTATUS result = RegGetValue(HKEY_CURRENT_USER, kGetPreferredBrightnessRegKey,
                               kGetPreferredBrightnessRegValue,
                               RRF_RT_REG_DWORD, nullptr, &light_mode,
                               &light_mode_size);

  if (result == ERROR_SUCCESS) {
    BOOL enable_dark_mode = light_mode == 0;
    DwmSetWindowAttribute(window, DWMWA_USE_IMMERSIVE_DARK_MODE,
                          &enable_dark_mode, sizeof(enable_dark_mode));
  }
}
