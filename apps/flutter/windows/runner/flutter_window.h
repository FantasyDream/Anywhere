#ifndef RUNNER_FLUTTER_WINDOW_H_
#define RUNNER_FLUTTER_WINDOW_H_
#include <flutter/dart_project.h>
#include <flutter/flutter_view_controller.h>
#include <flutter/method_channel.h>
#include <flutter/encodable_value.h>
#include <shellapi.h>
#include <memory>
#include <optional>
#include <chrono>
#include "win32_window.h"

class FlutterWindow : public Win32Window {
 public:
  explicit FlutterWindow(const flutter::DartProject& project);
  ~FlutterWindow() override;
 protected:
  bool OnCreate() override;
  void OnDestroy() override;
  LRESULT MessageHandler(HWND window, UINT message, WPARAM wparam, LPARAM lparam) noexcept override;
 private:
  void Activate();
  void Dismiss(bool notify);
  void Resize(double height, bool position);
  flutter::DartProject project_;
  std::unique_ptr<flutter::FlutterViewController> flutter_controller_;
  std::unique_ptr<flutter::MethodChannel<flutter::EncodableValue>> channel_;
  std::optional<int64_t> presented_;
  int64_t latest_generation_ = 0;
  std::chrono::steady_clock::time_point activation_start_;
  HWND previous_foreground_ = nullptr;
  NOTIFYICONDATAW tray_{};
};
#endif
