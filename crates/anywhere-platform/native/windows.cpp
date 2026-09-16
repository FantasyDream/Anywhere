/// One plain-text clipboard snapshot.
///
/// Returns S_OK with `length` set to the copied length, S_FALSE when the clipboard holds
/// no text, and an error HRESULT otherwise. `capacity` bounds the copy so a huge clipboard
/// cannot be pulled into the launcher at activation.
/// Resolve one Start Menu shortcut: its display name, its launch target and its description.
///
/// The display name is the shortcut's own file name, which is what the shell shows in the
/// Start Menu and what users read and type. `GetDescription` is a free-text description, not
/// a name — `Google Chrome.lnk` can describe itself as "Access the Internet" — so it is
/// returned separately and used only as a searchable alias. The target is metadata as well:
/// the shortcut itself stays the launch path, because it carries the arguments and working
/// directory, and `GetPath` may return a string with unexpanded environment variables.
extern "C" int32_t launcher_windows_resolve_shortcut(const wchar_t* shortcut,
                                                     wchar_t* name, uint32_t name_capacity,
                                                     wchar_t* target, uint32_t target_capacity,
                                                     wchar_t* description,
                                                     uint32_t description_capacity) {
  if (!shortcut || !name || !target || !description) return E_INVALIDARG;
  name[0] = L'\0';
  target[0] = L'\0';
  description[0] = L'\0';
  ComScope com;
  if (FAILED(com.result)) return com.result;

  // The file name, without directory or extension.
  std::wstring stem(shortcut);
  const size_t slash = stem.find_last_of(L"\\/");
  if (slash != std::wstring::npos) stem = stem.substr(slash + 1);
  const size_t dot = stem.find_last_of(L'.');
  if (dot != std::wstring::npos) stem = stem.substr(0, dot);
  HRESULT hr = copy_path(stem.c_str(), name, name_capacity);
  if (FAILED(hr)) return hr;

  ComPtr<IShellLinkW> link;
  hr = CoCreateInstance(CLSID_ShellLink, nullptr, CLSCTX_INPROC_SERVER, IID_PPV_ARGS(&link));
  if (FAILED(hr)) return hr;
  ComPtr<IPersistFile> file;
  if (FAILED(hr = link.As(&file))) return hr;
  if (FAILED(hr = file->Load(shortcut, STGM_READ))) return hr;

  // `GetPath` with SLGP_RAWPATH returns the recorded target without environment expansion.
  wchar_t resolved[MAX_PATH] = {};
  WIN32_FIND_DATAW data{};
  if (SUCCEEDED(link->GetPath(resolved, MAX_PATH, &data, SLGP_RAWPATH)) && resolved[0]) {
    hr = copy_path(resolved, target, target_capacity);
    if (FAILED(hr)) return hr;
  } else {
    hr = copy_path(shortcut, target, target_capacity);
    if (FAILED(hr)) return hr;
  }

  wchar_t described[INFOTIPSIZE] = {};
  if (SUCCEEDED(link->GetDescription(described, INFOTIPSIZE)) && described[0]) {
    hr = copy_path(described, description, description_capacity);
    if (FAILED(hr)) return hr;
  }
  return S_OK;
}

#define NOMINMAX
#include <windows.h>
#include <exdisp.h>
#include <shlobj.h>
#include <shobjidl.h>
#include <wrl/client.h>
#include <cstdint>
#include <cstring>
#include <string>

using Microsoft::WRL::ComPtr;
namespace {
struct ComScope {
  HRESULT result = CoInitializeEx(nullptr, COINIT_APARTMENTTHREADED);
  ~ComScope() { if (SUCCEEDED(result)) CoUninitialize(); }
};
HRESULT copy_path(const wchar_t* source, wchar_t* output, uint32_t capacity) {
  if (wcslen(source) >= capacity) return HRESULT_FROM_WIN32(ERROR_INSUFFICIENT_BUFFER);
  wcscpy_s(output, capacity, source);
  return S_OK;
}
HRESULT shell_open(const wchar_t* target, const wchar_t* parameters = nullptr,
                   const wchar_t* directory = nullptr) {
  auto status = reinterpret_cast<INT_PTR>(ShellExecuteW(
      nullptr, L"open", target, parameters, directory, SW_SHOWNORMAL));
  return status > 32 ? S_OK : HRESULT_FROM_WIN32(static_cast<DWORD>(status ? status : ERROR_GEN_FAILURE));
}
}

extern "C" int32_t launcher_windows_foreground(wchar_t* executable, uint32_t capacity,
                                                intptr_t* captured_window) {
  HWND window = GetForegroundWindow();
  if (!window) return S_FALSE;
  DWORD pid = 0;
  GetWindowThreadProcessId(window, &pid);
  HANDLE process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, FALSE, pid);
  if (!process) return HRESULT_FROM_WIN32(GetLastError());
  DWORD length = capacity;
  BOOL ok = QueryFullProcessImageNameW(process, 0, executable, &length);
  DWORD error = ok ? ERROR_SUCCESS : GetLastError();
  CloseHandle(process);
  if (!ok) return HRESULT_FROM_WIN32(error);
  *captured_window = reinterpret_cast<intptr_t>(window);
  return S_OK;
}

extern "C" int32_t launcher_windows_directory(intptr_t captured_window,
                                               wchar_t* output, uint32_t capacity) {
  HWND window = reinterpret_cast<HWND>(captured_window);
  if (!IsWindow(window)) return S_FALSE;
  ComScope com;
  if (FAILED(com.result)) return com.result;
  wchar_t class_name[64] = {};
  GetClassNameW(window, class_name, 64);
  if (wcscmp(class_name, L"Progman") == 0 || wcscmp(class_name, L"WorkerW") == 0) {
    PWSTR desktop = nullptr;
    HRESULT hr = SHGetKnownFolderPath(FOLDERID_Desktop, 0, nullptr, &desktop);
    if (FAILED(hr)) return hr;
    hr = copy_path(desktop, output, capacity);
    CoTaskMemFree(desktop);
    return hr;
  }
  ComPtr<IShellWindows> windows;
  HRESULT hr = CoCreateInstance(CLSID_ShellWindows, nullptr, CLSCTX_ALL, IID_PPV_ARGS(&windows));
  if (FAILED(hr)) return hr;
  long count = 0;
  if (FAILED(hr = windows->get_Count(&count))) return hr;
  for (long index = 0; index < count; ++index) {
    VARIANT item{};
    item.vt = VT_I4;
    item.lVal = index;
    ComPtr<IDispatch> dispatch;
    if (FAILED(windows->Item(item, &dispatch)) || !dispatch) continue;
    ComPtr<IWebBrowserApp> browser;
    if (FAILED(dispatch.As(&browser))) continue;
    SHANDLE_PTR handle = 0;
    if (FAILED(browser->get_HWND(&handle)) || reinterpret_cast<HWND>(handle) != window) continue;
    ComPtr<IServiceProvider> services;
    ComPtr<IShellBrowser> shell;
    ComPtr<IShellView> view;
    if (FAILED(browser.As(&services)) ||
        FAILED(services->QueryService(SID_STopLevelBrowser, IID_PPV_ARGS(&shell))) ||
        FAILED(shell->QueryActiveShellView(&view))) continue;
    // Windows 11 can enumerate inactive tabs for the same top-level HWND.
    HWND view_window = nullptr;
    if (FAILED(view->GetWindow(&view_window)) || !IsWindowVisible(view_window)) continue;
    ComPtr<IFolderView> folder_view;
    ComPtr<IPersistFolder2> folder;
    if (FAILED(view.As(&folder_view)) ||
        FAILED(folder_view->GetFolder(IID_PPV_ARGS(&folder)))) continue;
    PIDLIST_ABSOLUTE id = nullptr;
    if (FAILED(hr = folder->GetCurFolder(&id))) return hr;
    BOOL is_path = SHGetPathFromIDListEx(id, output, capacity, GPFIDL_DEFAULT);
    CoTaskMemFree(id);
    // Home, This PC, search results and other virtual views have no filesystem path.
    if (!is_path) return S_FALSE;
    DWORD attributes = GetFileAttributesW(output);
    return attributes != INVALID_FILE_ATTRIBUTES && (attributes & FILE_ATTRIBUTE_DIRECTORY)
               ? S_OK : S_FALSE;
  }
  return S_FALSE;
}

extern "C" int32_t launcher_windows_open(const wchar_t* value, int32_t kind) {
  ComScope com;
  if (FAILED(com.result)) return com.result;
  if (kind == 1) {
    PIDLIST_ABSOLUTE id = nullptr;
    HRESULT hr = SHParseDisplayName(value, nullptr, &id, 0, nullptr);
    if (FAILED(hr)) return hr;
    hr = SHOpenFolderAndSelectItems(id, 0, nullptr, 0);
    CoTaskMemFree(id);
    return hr;
  }
  if (kind == 2) {
    wchar_t system[MAX_PATH] = {};
    if (!GetSystemDirectoryW(system, MAX_PATH)) return HRESULT_FROM_WIN32(GetLastError());
    std::wstring executable = std::wstring(system) + L"\\WindowsPowerShell\\v1.0\\powershell.exe";
    std::wstring parameters = L"-NoLogo -NoExit -Command Set-Location -LiteralPath '";
    for (const wchar_t* c = value; *c; ++c) {
      parameters += *c;
      if (*c == L'\'') parameters += L'\'';
    }
    parameters += L"'";
    return shell_open(executable.c_str(), parameters.c_str(), value);
  }
  return shell_open(value);
}

extern "C" int32_t launcher_windows_copy(const wchar_t* text, uint32_t length) {
  const SIZE_T bytes = (static_cast<SIZE_T>(length) + 1) * sizeof(wchar_t);
  HGLOBAL memory = GlobalAlloc(GMEM_MOVEABLE, bytes);
  if (!memory) return E_OUTOFMEMORY;
  void* buffer = GlobalLock(memory);
  if (!buffer) { GlobalFree(memory); return E_OUTOFMEMORY; }
  memcpy(buffer, text, bytes);
  GlobalUnlock(memory);
  // EmptyClipboard requires a non-null owner for subsequent SetClipboardData.
  HWND owner = CreateWindowExW(0, L"STATIC", L"", 0, 0, 0, 0, 0, HWND_MESSAGE,
                               nullptr, GetModuleHandleW(nullptr), nullptr);
  if (!owner) { GlobalFree(memory); return HRESULT_FROM_WIN32(GetLastError()); }
  bool opened = false;
  for (int attempt = 0; attempt < 10 && !opened; ++attempt) {
    opened = OpenClipboard(owner) != FALSE;
    if (!opened) Sleep(10);
  }
  HRESULT hr = HRESULT_FROM_WIN32(ERROR_BUSY);
  if (opened) {
    if (EmptyClipboard() && SetClipboardData(CF_UNICODETEXT, memory)) {
      memory = nullptr;  // Ownership transfers to Windows only on success.
      hr = S_OK;
    } else { hr = HRESULT_FROM_WIN32(GetLastError()); }
    CloseClipboard();
  }
  if (memory) GlobalFree(memory);
  DestroyWindow(owner);
  return hr;
}

extern "C" int32_t launcher_windows_clipboard_text(wchar_t* output, uint32_t capacity,
                                                   uint32_t* length) {
  if (!output || capacity == 0 || !length) return E_INVALIDARG;
  *length = 0;
  HWND owner = CreateWindowExW(0, L"STATIC", L"", 0, 0, 0, 0, 0, HWND_MESSAGE,
                               nullptr, GetModuleHandleW(nullptr), nullptr);
  if (!owner) return HRESULT_FROM_WIN32(GetLastError());
  bool opened = false;
  for (int attempt = 0; attempt < 5 && !opened; ++attempt) {
    opened = OpenClipboard(owner) != FALSE;
    if (!opened) Sleep(10);
  }
  HRESULT hr = S_FALSE;
  if (opened) {
    if (IsClipboardFormatAvailable(CF_UNICODETEXT)) {
      HANDLE handle = GetClipboardData(CF_UNICODETEXT);
      if (handle) {
        // The clipboard belongs to another process and can be malformed, so the source is
        // never scanned past what the handle actually spans *or* past what this snapshot
        // can hold: `GlobalSize` bounds the allocation, the caller's capacity bounds the
        // useful text, and the search stops at whichever is smaller. Reading to the end of
        // a huge clipboard only to copy a fraction of it would make activation cost grow
        // with whatever the user happened to copy. A missing terminator inside that bound is
        // treated as no text rather than read past.
        const SIZE_T bytes = GlobalSize(handle);
        const wchar_t* text = static_cast<const wchar_t*>(GlobalLock(handle));
        if (text) {
          const SIZE_T limit = static_cast<SIZE_T>(capacity) - 1;
          const SIZE_T spanned = bytes / sizeof(wchar_t);
          const SIZE_T scanned = spanned < limit ? spanned : limit;
          SIZE_T available = 0;
          while (available < scanned && text[available] != L'\0') ++available;
          if (available < scanned) {
            memcpy(output, text, available * sizeof(wchar_t));
            output[available] = L'\0';
            *length = static_cast<uint32_t>(available);
            hr = S_OK;
          }
          GlobalUnlock(handle);
        } else {
          hr = HRESULT_FROM_WIN32(GetLastError());
        }
      }
    }
    CloseClipboard();
  } else {
    hr = HRESULT_FROM_WIN32(ERROR_BUSY);
  }
  DestroyWindow(owner);
  return hr;
}
