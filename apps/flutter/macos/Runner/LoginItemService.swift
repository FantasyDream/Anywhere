import Foundation
import ServiceManagement

struct LoginItemUpdate {
  let enabled: Bool
  let error: String?
}

protocol LoginItemManaging {
  var isEnabled: Bool? { get }
  func apply(_ enabled: Bool) -> LoginItemUpdate
}

/// System login-item adaptation, independent of settings persistence and windows.
final class LoginItemService: LoginItemManaging {
  private let isolated: Bool
  private let log: ([String: Any]) -> Void

  init(
    isolated: Bool = ProcessInfo.processInfo.environment["ANYWHERE_DATA_DIRECTORY"] != nil,
    log: @escaping ([String: Any]) -> Void = { _ in }
  ) {
    self.isolated = isolated
    self.log = log
  }

  var isEnabled: Bool? {
    // No system reads or writes for an explicitly isolated acceptance run.
    guard !isolated else { return nil }
    if #available(macOS 13.0, *) { return SMAppService.mainApp.status == .enabled }
    return legacyIsEnabled()
  }

  func apply(_ want: Bool) -> LoginItemUpdate {
    guard !isolated else { return LoginItemUpdate(enabled: want, error: nil) }
    if #available(macOS 13.0, *) {
      let service = SMAppService.mainApp
      do {
        if want {
          if service.status != .enabled { try service.register() }
        } else if service.status == .enabled {
          try service.unregister()
        }
      } catch {
        log(["event": "login_item_failed", "error": "\(error)"])
        return LoginItemUpdate(
          enabled: service.status == .enabled,
          error: "Open at login failed: \(error)"
        )
      }
      let enabled = service.status == .enabled
      return LoginItemUpdate(
        enabled: enabled,
        error: want && !enabled ? "Open at login was not enabled." : nil
      )
    }
    guard let current = legacyIsEnabled() else {
      return LoginItemUpdate(enabled: want, error: "Open at login could not be read.")
    }
    if want == current { return LoginItemUpdate(enabled: want, error: nil) }
    let path = Bundle.main.bundlePath
      .replacingOccurrences(of: "\\", with: "\\\\")
      .replacingOccurrences(of: "\"", with: "\\\"")
    let script =
      want
      ? "tell application \"System Events\" to make login item at end with properties {path:\"\(path)\", hidden:false}"
      : "tell application \"System Events\" to delete (every login item whose path is \"\(path)\")"
    let result = runOsascript(script)
    guard let enabled = legacyIsEnabled() else {
      return LoginItemUpdate(enabled: want, error: "Open at login could not be confirmed.")
    }
    return LoginItemUpdate(
      enabled: enabled,
      error: result.status != 0 || enabled != want ? "Open at login failed." : nil
    )
  }

  private func legacyIsEnabled() -> Bool? {
    let result = runOsascript(
      """
      tell application "System Events"
          if (count of login items) is 0 then return ""
          set AppleScript's text item delimiters to linefeed
          return (path of every login item) as text
      end tell
      """
    )
    guard result.status == 0 else { return nil }
    return result.output.split(separator: "\n").contains(Substring(Bundle.main.bundlePath))
  }

  private func runOsascript(_ script: String) -> (status: Int32, output: String) {
    let process = Process()
    process.executableURL = URL(fileURLWithPath: "/usr/bin/osascript")
    process.arguments = ["-e", script]
    let pipe = Pipe()
    process.standardOutput = pipe
    process.standardError = Pipe()
    do {
      try process.run()
    } catch {
      return (1, "")
    }
    process.waitUntilExit()
    let data = pipe.fileHandleForReading.readDataToEndOfFile()
    return (
      process.terminationStatus,
      String(data: data, encoding: .utf8)?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
    )
  }
}
