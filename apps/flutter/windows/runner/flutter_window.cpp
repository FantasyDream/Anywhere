#include "flutter_window.h"
#include <flutter/standard_method_codec.h>
#include <dwmapi.h>
#include <shlobj.h>
#include <algorithm>
#include <cmath>
#include <cstdio>
#include <filesystem>
#include "flutter/generated_plugin_registrant.h"
#include "resource.h"
#include "utils.h"

using flutter::EncodableMap;
using flutter::EncodableValue;
namespace {
constexpr UINT kTrayMessage = WM_APP + 1;
const EncodableValue* Field(const EncodableValue* args, const char* key) {
  const auto* map = args ? std::get_if<EncodableMap>(args) : nullptr;
  if (!map) return nullptr;
  const auto found = map->find(EncodableValue(key));
  return found == map->end() ? nullptr : &found->second;
}
std::optional<int64_t> Generation(const EncodableValue* args) {
  const auto* value = Field(args, "generation");
  if (value) {
    if (const auto* small = std::get_if<int32_t>(value)) return *small;
    if (const auto* large = std::get_if<int64_t>(value)) return *large;
  }
  return std::nullopt;
}
double Height(const EncodableValue* args) {
  const auto* value = Field(args, "height");
  if (value) { if (const auto* height = std::get_if<double>(value)) return *height; }
  return 64;
}
}

FlutterWindow::FlutterWindow(const flutter::DartProject& project) : project_(project) {}
FlutterWindow::~FlutterWindow() { OnDestroy(); }

bool FlutterWindow::OnCreate() {
  if (!Win32Window::OnCreate()) return false;
  SetWindowLongPtr(GetHandle(), GWL_STYLE, WS_POPUP | WS_CLIPCHILDREN);
  SetWindowLongPtr(GetHandle(), GWL_EXSTYLE, WS_EX_TOOLWINDOW);
  BOOL dark = TRUE;
  DwmSetWindowAttribute(GetHandle(), static_cast<DWMWINDOWATTRIBUTE>(20), &dark, sizeof(dark));
  // Public Windows 11 attributes; unsupported systems keep the dark Flutter surface.
  DWORD corner = 2, backdrop = 3;
  DwmSetWindowAttribute(GetHandle(), static_cast<DWMWINDOWATTRIBUTE>(33), &corner, sizeof(corner));
  DwmSetWindowAttribute(GetHandle(), static_cast<DWMWINDOWATTRIBUTE>(38), &backdrop, sizeof(backdrop));
  MARGINS margins{-1, -1, -1, -1};
  DwmExtendFrameIntoClientArea(GetHandle(), &margins);
  Resize(64, true);
  RECT frame = GetClientArea();
  flutter_controller_ = std::make_unique<flutter::FlutterViewController>(
      frame.right - frame.left, frame.bottom - frame.top, project_);
  if (!flutter_controller_->engine() || !flutter_controller_->view()) return false;
  RegisterPlugins(flutter_controller_->engine());
  SetChildContent(flutter_controller_->view()->GetNativeWindow());

  std::string hotkey_error;
  if (!RegisterHotKey(GetHandle(), 1, MOD_CONTROL | MOD_ALT | MOD_NOREPEAT, VK_SPACE)) {
    hotkey_error = "Ctrl+Alt+Space is unavailable; use the tray icon or release the shortcut.";
  }
  tray_.cbSize = sizeof(tray_);
  tray_.hWnd = GetHandle();
  tray_.uID = 1;
  tray_.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
  tray_.uCallbackMessage = kTrayMessage;
  tray_.hIcon = LoadIcon(GetModuleHandle(nullptr), MAKEINTRESOURCE(IDI_APP_ICON));
  wcscpy_s(tray_.szTip, L"Context Launcher");
  if (!Shell_NotifyIconW(NIM_ADD, &tray_)) hotkey_error += " Tray icon unavailable.";

  channel_ = std::make_unique<flutter::MethodChannel<EncodableValue>>(
      flutter_controller_->engine()->messenger(), "context_launcher/window",
      &flutter::StandardMethodCodec::GetInstance());
  channel_->SetMethodCallHandler([this, hotkey_error](const auto& call, auto result) {
    const auto& method = call.method_name();
    auto generation = Generation(call.arguments());
    if (method == "paths") {
      PWSTR local = nullptr;
      if (FAILED(SHGetKnownFolderPath(FOLDERID_LocalAppData, 0, nullptr, &local))) {
        result->Error("paths", "Cannot locate LocalAppData"); return;
      }
      auto data = std::filesystem::path(local) / L"ContextLauncher";
      CoTaskMemFree(local);
      wchar_t executable[32768] = {};
      GetModuleFileNameW(nullptr, executable, 32768);
      auto plugins = std::filesystem::path(executable).parent_path() / L"plugins";
      result->Success(EncodableValue(EncodableMap{
          {EncodableValue("data"), EncodableValue(Utf8FromUtf16(data.c_str()))},
          {EncodableValue("plugins"), EncodableValue(Utf8FromUtf16(plugins.c_str()))},
          {EncodableValue("hotkeyError"), EncodableValue(hotkey_error)}}));
      return;
    }
    if (method == "ready") {
      result->Success();
      Activate();
      return;
    }
    if (method == "show" && generation && *generation >= latest_generation_) {
      latest_generation_ = *generation;
      Resize(Height(call.arguments()), true);
      ShowWindow(GetHandle(), SW_SHOW);
      SetForegroundWindow(GetHandle());
      SetFocus(flutter_controller_->view()->GetNativeWindow());
      presented_ = *generation;
      flutter_controller_->ForceRedraw();
      double elapsed = std::chrono::duration<double, std::milli>(
          std::chrono::steady_clock::now() - activation_start_).count();
      std::fprintf(stderr, "{\"event\":\"launcher_visible\",\"hotkey_to_visible_ms\":%.3f}\n", elapsed);
    } else if (method == "resize" && generation && presented_ == generation) {
      Resize(Height(call.arguments()), false);
    } else if (method == "hide" && generation && *generation >= latest_generation_) {
      latest_generation_ = *generation + 1;
      Dismiss(false);
    } else if (method != "show" && method != "resize" && method != "hide") {
      result->NotImplemented(); return;
    }
    result->Success();
  });
  // Dart requests show only after foreground context capture has completed.
  return true;
}

void FlutterWindow::Activate() {
  if (!channel_) return;
  activation_start_ = std::chrono::steady_clock::now();
  std::fprintf(stderr, "{\"event\":\"activation_requested\"}\n");
  HWND foreground = GetForegroundWindow();
  if (foreground && foreground != GetHandle()) previous_foreground_ = foreground;
  channel_->InvokeMethod("toggle", nullptr);
}

void FlutterWindow::Dismiss(bool notify) {
  auto generation = presented_;
  presented_.reset();
  if (generation) latest_generation_ = *generation + 1;
  bool restore = GetForegroundWindow() == GetHandle();
  ShowWindow(GetHandle(), SW_HIDE);
  if (restore && IsWindow(previous_foreground_)) SetForegroundWindow(previous_foreground_);
  if (notify && generation && channel_) {
    channel_->InvokeMethod("dismiss", std::make_unique<EncodableValue>(
        EncodableMap{{EncodableValue("generation"), EncodableValue(*generation)}}));
  }
}

void FlutterWindow::Resize(double requested, bool position) {
  if (!std::isfinite(requested)) return;
  POINT mouse{};
  GetCursorPos(&mouse);
  HMONITOR monitor = position ? MonitorFromPoint(mouse, MONITOR_DEFAULTTONEAREST)
                              : MonitorFromWindow(GetHandle(), MONITOR_DEFAULTTONEAREST);
  MONITORINFO info{};
  info.cbSize = sizeof(info);
  GetMonitorInfoW(monitor, &info);
  double scale = GetDpiForWindow(GetHandle()) / 96.0;
  int width = static_cast<int>(648 * scale);
  int height = std::min(static_cast<int>(std::clamp(requested, 64.0, 520.0) * scale),
                        static_cast<int>(info.rcWork.bottom - info.rcWork.top));
  RECT frame{};
  GetWindowRect(GetHandle(), &frame);
  int x = position ? info.rcWork.left + (info.rcWork.right - info.rcWork.left - width) / 2 : frame.left;
  int y = position ? info.rcWork.top + (info.rcWork.bottom - info.rcWork.top) / 5 : frame.top;
  y = std::min(y, static_cast<int>(info.rcWork.bottom) - height);
  if (position || frame.right - frame.left != width || frame.bottom - frame.top != height) {
    SetWindowPos(GetHandle(), HWND_TOPMOST, x, y, width, height, SWP_NOACTIVATE | SWP_FRAMECHANGED);
  }
}

void FlutterWindow::OnDestroy() {
  if (tray_.hWnd) {
    Shell_NotifyIconW(NIM_DELETE, &tray_);
    UnregisterHotKey(tray_.hWnd, 1);
    tray_.hWnd = nullptr;
  }
  channel_.reset();
  flutter_controller_.reset();
  Win32Window::OnDestroy();
}

LRESULT FlutterWindow::MessageHandler(HWND hwnd, UINT message, WPARAM wparam, LPARAM lparam) noexcept {
  if (message == WM_HOTKEY && wparam == 1) { Activate(); return 0; }
  if (message == WM_ACTIVATE && LOWORD(wparam) == WA_INACTIVE) {
    if (presented_) Dismiss(true);
    return 0;
  }
  if (message == WM_CLOSE) { Dismiss(true); return 0; }
  if (message == kTrayMessage) {
    if (lparam == WM_LBUTTONUP) Activate();
    if (lparam == WM_RBUTTONUP) {
      HWND foreground = GetForegroundWindow();
      if (foreground != hwnd) previous_foreground_ = foreground;
      POINT mouse{};
      GetCursorPos(&mouse);
      HMENU menu = CreatePopupMenu();
      AppendMenuW(menu, MF_STRING, 1, L"Open Context Launcher  Ctrl+Alt+Space");
      AppendMenuW(menu, MF_STRING, 2, L"Quit Context Launcher");
      SetForegroundWindow(hwnd);
      UINT command = TrackPopupMenu(menu, TPM_RETURNCMD | TPM_RIGHTBUTTON, mouse.x, mouse.y, 0, hwnd, nullptr);
      DestroyMenu(menu);
      if (IsWindow(previous_foreground_)) SetForegroundWindow(previous_foreground_);
      if (command == 1) Activate();
      if (command == 2) DestroyWindow(hwnd);
    }
    return 0;
  }
  if (flutter_controller_) {
    auto result = flutter_controller_->HandleTopLevelWindowProc(hwnd, message, wparam, lparam);
    if (result) return *result;
    if (message == WM_FONTCHANGE) flutter_controller_->engine()->ReloadSystemFonts();
  }
  return Win32Window::MessageHandler(hwnd, message, wparam, lparam);
}
