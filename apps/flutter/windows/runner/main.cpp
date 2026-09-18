#include <flutter/dart_project.h>
#include <flutter/flutter_view_controller.h>
#include <windows.h>
#include <shlobj.h>
#include <filesystem>
#include <io.h>
#include <cstdio>

#include "flutter_window.h"
#include "utils.h"

int APIENTRY wWinMain(_In_ HINSTANCE instance, _In_opt_ HINSTANCE prev,
                      _In_ wchar_t *command_line, _In_ int show_command) {
  // Attach to console when present (e.g., 'flutter run') or create a
  // new console when running with a debugger.
  if (!::AttachConsole(ATTACH_PARENT_PROCESS) && ::IsDebuggerPresent()) {
    CreateAndAttachConsole();
  }

  // Initialize COM, so that it is available for use in the library and/or
  // plugins.
  ::CoInitializeEx(nullptr, COINIT_APARTMENTTHREADED);

  PWSTR local = nullptr;
  if (SUCCEEDED(SHGetKnownFolderPath(FOLDERID_LocalAppData, 0, nullptr, &local))) {
    // Adopt the previous name once, so an existing usage history is kept.
    auto data = std::filesystem::path(local) / L"Anywhere";
    std::error_code error;
    if (!std::filesystem::exists(data)) {
      auto previous = std::filesystem::path(local) / L"ContextLauncher";
      if (std::filesystem::exists(previous)) {
        std::filesystem::rename(previous, data, error);
      }
    }
    CoTaskMemFree(local);
    std::filesystem::create_directories(data, error);
    if (!error) {
      FILE* log = nullptr;
      if (_wfreopen_s(&log, (data / L"launcher.log").c_str(), L"w", stderr) == 0) {
        SetStdHandle(STD_ERROR_HANDLE, reinterpret_cast<HANDLE>(_get_osfhandle(_fileno(stderr))));
      }
    }
  }
  wchar_t executable[32768] = {};
  GetModuleFileNameW(nullptr, executable, 32768);
  auto data = std::filesystem::path(executable).parent_path() / L"data";
  flutter::DartProject project(data.wstring());
  project.set_impeller_switch(flutter::ImpellerSwitch::Enabled);

  std::vector<std::string> command_line_arguments =
      GetCommandLineArguments();

  project.set_dart_entrypoint_arguments(std::move(command_line_arguments));

  FlutterWindow window(project);
  Win32Window::Point origin(10, 10);
  Win32Window::Size size(648, 64);
  if (!window.Create(L"Anywhere", origin, size)) {
    return EXIT_FAILURE;
  }
  window.SetQuitOnClose(true);

  ::MSG msg;
  while (::GetMessage(&msg, nullptr, 0, 0)) {
    ::TranslateMessage(&msg);
    ::DispatchMessage(&msg);
  }

  ::CoUninitialize();
  return EXIT_SUCCESS;
}
