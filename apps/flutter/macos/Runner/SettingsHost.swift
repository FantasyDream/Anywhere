import AppKit
import FlutterMacOS

/// Owns authoritative native settings, save side effects and permission requests.
/// The shared window channel delegates these methods without changing its wire contract.
final class SettingsHost {
  private let store: SettingsStore
  private let hotKeys: HotKeyService
  private let fileAccess: FileSearchAccess
  private let loginItem: LoginItemManaging
  private let updateLocale: (String?) -> Void
  private let openPrivacy: (PrivacySettingsPane) -> Bool
  private(set) var loginError: String?
  var current: StoredSettings { store.current }
  var hotKeyError: String? { hotKeys.lastError }
  var status: [String: String] {
    [
      "hotkeyError": hotKeyError ?? "",
      "loginError": loginError ?? "",
      "fileAccessError": fileAccess.status,
    ]
  }

  init(
    store: SettingsStore = SettingsStore(),
    hotKeys: HotKeyService,
    fileAccess: FileSearchAccess = FileSearchAccess(),
    loginItem: LoginItemManaging = LoginItemService(),
    updateLocale: @escaping (String?) -> Void = LocaleHost.update,
    openPrivacy: @escaping (PrivacySettingsPane) -> Bool = { $0.open() }
  ) {
    self.store = store
    self.hotKeys = hotKeys
    self.fileAccess = fileAccess
    self.loginItem = loginItem
    self.updateLocale = updateLocale
    self.openPrivacy = openPrivacy
  }

  func start() {
    if let error = hotKeys.apply(current) {
      hotKeys.lastError = error + "; use the menu bar icon or release the shortcut in another app."
    }
    let update = loginItem.apply(current.launchAtLogin)
    store.current.launchAtLogin = update.enabled
    loginError = update.error
  }

  /// Returns false without replying when the method belongs to another Host.
  func handle(_ call: FlutterMethodCall, reply: @escaping FlutterResult) -> Bool {
    switch call.method {
    case "settings":
      if let enabled = loginItem.isEnabled { store.current.launchAtLogin = enabled }
      var snapshot = current.asMap()
      snapshot["fileSearchAccessReady"] = fileAccess.ready
      reply(snapshot)
    case "saveSettings":
      save(call.arguments, reply: reply)
    case "syncActionShortcutCatalog":
      syncShortcutCatalog(call.arguments, reply: reply)
    case "prepareFileSearchAccess":
      prepareFileAccess(call.arguments, reply: reply)
    case "settingsStatus":
      reply(status)
    case "resumeActionShortcuts":
      if let error = hotKeys.resumeForSettingsClose() {
        reply(FlutterError(code: "shortcuts", message: error, details: nil))
      } else {
        reply(nil)
      }
    case "openPrivacySettings":
      guard let args = call.arguments as? [String: Any],
        let name = args["pane"] as? String, let pane = PrivacySettingsPane(rawValue: name)
      else {
        reply(FlutterError(code: "privacy", message: "Unknown privacy setting.", details: nil))
        return true
      }
      if openPrivacy(pane) {
        reply(nil)
      } else {
        reply(
          FlutterError(
            code: "privacy",
            message: "System Settings could not be opened.",
            details: nil
          )
        )
      }
    default: return false
    }
    return true
  }

  private func save(_ arguments: Any?, reply: FlutterResult) {
    guard let args = arguments as? [String: Any] else {
      reply(nil)
      return
    }
    let previous = current
    var next = previous
    if let error = next.apply(args) {
      reply(FlutterError(code: "settings", message: error, details: nil))
      return
    }
    var support: Set<String>?
    if let raw = args["supportedActionIds"] {
      guard let ids = Self.shortcutIds(raw) else {
        reply(FlutterError(code: "settings", message: "Invalid shortcut catalog.", details: nil))
        return
      }
      support = ids
    }
    var errors: [String] = []
    let update = hotKeys.update(next, previous: previous, supportedActionIds: support)
    next = update.settings
    if let error = update.error { errors.append(error) }
    if next.launchAtLogin != previous.launchAtLogin {
      let loginUpdate = loginItem.apply(next.launchAtLogin)
      loginError = loginUpdate.error
      if let error = loginUpdate.error {
        errors.append(error)
        next.launchAtLogin = loginItem.isEnabled ?? previous.launchAtLogin
      }
    }
    store.current = next
    if let error = store.save() {
      store.current.uiLanguage = previous.uiLanguage
      errors.append(error)
    } else {
      updateLocale(next.uiLanguage)
    }
    if errors.isEmpty {
      reply(nil)
    } else {
      reply(FlutterError(code: "settings", message: errors.joined(separator: "\n"), details: nil))
    }
  }

  private func syncShortcutCatalog(_ arguments: Any?, reply: FlutterResult) {
    guard let args = arguments as? [String: Any],
      let ids = Self.shortcutIds(args["supportedActionIds"]),
      let snapshot = args["settings"] as? [String: Any]
    else {
      reply(FlutterError(code: "shortcuts", message: "Invalid shortcut catalog.", details: nil))
      return
    }
    var expected = current
    guard expected.apply(snapshot) == nil, expected.sameActionShortcutConfiguration(as: current)
    else {
      reply(
        FlutterError(
          code: "shortcuts",
          message: "Shortcut configuration changed. Refresh settings and try again.",
          details: nil
        )
      )
      return
    }
    if let error = hotKeys.syncActionCatalog(ids) {
      reply(FlutterError(code: "shortcuts", message: error, details: nil))
    } else {
      reply(nil)
    }
  }

  private static func shortcutIds(_ raw: Any?) -> Set<String>? {
    guard let ids = raw as? [String], ids.count <= RuntimePolicy.maxShortcutCatalogRequestIds,
      ids.allSatisfy({ !$0.isEmpty && $0.utf8.count <= RuntimePolicy.maxActionIdBytes })
    else { return nil }
    return Set(ids)
  }

  private func prepareFileAccess(_ arguments: Any?, reply: @escaping FlutterResult) {
    guard let args = arguments as? [String: Any],
      let enabled = args["enabled"] as? Bool,
      let roots = args["roots"] as? [String], roots.count <= RuntimePolicy.maxSearchRoots,
      roots.allSatisfy({ $0.hasPrefix("/") && $0.utf8.count <= RuntimePolicy.maxSearchPathBytes })
    else {
      reply(FlutterError(code: "file_access", message: "Invalid search scope.", details: nil))
      return
    }
    fileAccess.prepare(enabled: enabled, roots: roots, force: args["force"] as? Bool == true) {
      reply($0)
    }
  }
}

enum PrivacySettingsPane: String {
  case accessibility, automation, clipboard, fullDiskAccess

  func open() -> Bool {
    let app = NSWorkspace.shared.urlForApplication(
      withBundleIdentifier: "com.apple.systempreferences"
    )
    let anchor =
      self == .accessibility
      ? "Privacy_Accessibility"
      : self == .fullDiskAccess ? "Privacy_AllFiles" : "Privacy_Automation"
    // Clipboard preferences vary by macOS version; use System Settings itself.
    let link =
      self == .clipboard
      ? app : URL(string: "x-apple.systempreferences:com.apple.preference.security?\(anchor)")
    if let link, NSWorkspace.shared.open(link) { return true }
    if let app, NSWorkspace.shared.open(app) { return true }
    return false
  }
}
