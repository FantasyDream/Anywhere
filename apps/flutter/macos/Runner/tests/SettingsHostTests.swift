import Carbon
import Darwin
import FlutterMacOS
import Foundation

private func verify(_ condition: @autoclosure () -> Bool, line: UInt = #line) {
  if !condition() {
    fputs("Settings Host failed at line \(line)\n", stderr)
    exit(EXIT_FAILURE)
  }
}

private final class SettingsTestHotKeys: HotKeyBackend {
  var active: [UInt32: UInt32] = [:]
  var rejected = Set<UInt32>()
  var registrations = 0
  func install(onActivate: @escaping (UInt32) -> Void) -> OSStatus { noErr }
  func clear() { active.removeAll() }
  func unregister(id: UInt32) { active[id] = nil }
  func register(code: UInt32, modifiers: UInt32, id: UInt32, exclusive: Bool) -> OSStatus {
    registrations += 1
    if rejected.contains(code) { return -1 }
    active[id] = code
    return noErr
  }
  func shutdown() { clear() }
}

private final class SettingsTestLoginItem: LoginItemManaging {
  var isEnabled: Bool? = false
  var error: String?
  var requested: [Bool] = []
  func apply(_ enabled: Bool) -> LoginItemUpdate {
    requested.append(enabled)
    if error == nil { isEnabled = enabled }
    return LoginItemUpdate(enabled: isEnabled ?? enabled, error: error)
  }
}

private final class SettingsHostHarness {
  let store: SettingsStore
  let backend = SettingsTestHotKeys()
  let login = SettingsTestLoginItem()
  let messenger = HostChannelTestMessenger()
  let channel: FlutterMethodChannel
  let codec = FlutterStandardMethodCodec.sharedInstance()
  let fileAccess: FileSearchAccess
  var locales: [String?] = []
  var privacyPanes: [PrivacySettingsPane] = []
  var privacyAvailable = true
  private(set) var host: SettingsHost!
  private(set) var hotKeys: HotKeyService!

  init(_ directory: URL, fileAccess: FileSearchAccess? = nil) {
    store = SettingsStore(fileURL: directory.appendingPathComponent("settings.json"))
    self.fileAccess =
      fileAccess
      ?? FileSearchAccess(
        storage: directory.appendingPathComponent("grants.plist"),
        request: { _, reply in reply([]) },
        work: { $0() }
      )
    channel = FlutterMethodChannel(name: "anywhere/window", binaryMessenger: messenger)
    hotKeys = HotKeyService(backend: backend, supportedActionIds: [], onActivate: { _ in })
    host = SettingsHost(
      store: store,
      hotKeys: hotKeys,
      fileAccess: self.fileAccess,
      loginItem: login,
      updateLocale: { [weak self] in self?.locales.append($0) },
      openPrivacy: { [weak self] pane in
        self?.privacyPanes.append(pane)
        return self?.privacyAvailable == true
      }
    )
    channel.setMethodCallHandler { [weak self] call, reply in
      guard let self else { return }
      if !self.host.handle(call, reply: reply) { reply(FlutterMethodNotImplemented) }
    }
  }

  func request(_ method: String, _ arguments: Any? = nil) -> HostChannelTestMessenger.Reply {
    messenger.request("anywhere/window", method, arguments)
  }
  func map(_ method: String) -> [String: Any] {
    let response = request(method)
    verify(response.responses.count == 1 && response.responses[0] != nil)
    return codec.decodeEnvelope(response.responses[0]!) as! [String: Any]
  }
  func success(_ method: String, _ arguments: Any? = nil) {
    verify(request(method, arguments).responses == [codec.encodeSuccessEnvelope(nil)])
  }
  func failure(_ method: String, _ arguments: Any?, code: String, message: String) {
    verify(
      request(method, arguments).responses == [
        codec.encodeErrorEnvelope(FlutterError(code: code, message: message, details: nil))
      ]
    )
  }
  deinit {
    channel.setMethodCallHandler(nil)
    hotKeys.shutdown()
  }
}

@main
struct SettingsHostTests {
  @MainActor static func main() throws {
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
    defer { try? FileManager.default.removeItem(at: root) }
    try savesAndFailures(root)
    try catalogAndResume(root)
    try fileAccessAndPrivacy(root)
    let isolated = LoginItemService(isolated: true)
    verify(isolated.isEnabled == nil)
    verify(isolated.apply(true).enabled && isolated.apply(true).error == nil)
    verify(!isolated.apply(false).enabled && isolated.apply(false).error == nil)
    print(
      "Settings Host: encoded saves, partial failures, catalog guards, resume, authorization and privacy passed"
    )
  }

  private static func savesAndFailures(_ root: URL) throws {
    let directory = root.appendingPathComponent("saves")
    try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
    let h = SettingsHostHarness(directory)
    let initial = h.host.current
    h.host.start()
    verify(h.backend.active.count == 2 && h.login.requested == [false])
    h.login.isEnabled = true
    verify(h.map("settings")["launchAtLogin"] as? Bool == true)
    verify(
      h.host.current.launchAtLogin && h.map("settings")["fileSearchAccessReady"] as? Bool == false
    )
    // An unreadable system status retains the last authoritative value.
    h.login.isEnabled = nil
    verify(h.map("settings")["launchAtLogin"] as? Bool == true)
    h.success("saveSettings", ["uiLanguage": "zh", "excludedApps": ["test.hidden"]])
    verify(h.locales == ["zh"])
    verify(StoredSettings.load(from: h.store.fileURL).excludedApps == ["test.hidden"])
    let registrations = h.backend.registrations
    let loginRequests = h.login.requested
    h.failure(
      "saveSettings",
      ["uiLanguage": "invalid", "launchAtLogin": false],
      code: "settings",
      message: "Unsupported interface language."
    )
    h.failure(
      "saveSettings",
      ["supportedActionIds": [""]],
      code: "settings",
      message: "Invalid shortcut catalog."
    )
    h.failure(
      "saveSettings",
      ["fileSearchRoots": ["relative"]],
      code: "settings",
      message: "Search directories must be absolute paths."
    )
    h.success("saveSettings", "malformed")  // Preserve the existing no-op contract.
    verify(h.locales == ["zh"] && h.login.requested == loginRequests)
    verify(h.backend.registrations == registrations)
    verify(h.host.current.uiLanguage == "zh" && h.host.current.launchAtLogin)

    // Reject a wake chord, retain unrelated changes and persist the accepted settings.
    h.backend.rejected = [0]
    h.failure(
      "saveSettings",
      ["currentAppKeyCode": 4, "excludedApps": ["accepted"]],
      code: "settings",
      message: "Current App shortcut is unavailable (-1)"
    )
    verify(h.host.current.sameHotKeys(as: initial))
    verify(h.backend.active[1] == initial.currentAppKeyCode)
    verify(StoredSettings.load(from: h.store.fileURL).excludedApps == ["accepted"])
    verify(h.map("settingsStatus")["hotkeyError"] as? String == "")

    h.backend.rejected = []
    h.login.error = "Login rejected"
    h.login.isEnabled = true
    h.failure(
      "saveSettings",
      ["launchAtLogin": false, "uiLanguage": "en"],
      code: "settings",
      message: "Login rejected"
    )
    verify(h.host.current.launchAtLogin && h.host.current.uiLanguage == "en")
    verify(StoredSettings.load(from: h.store.fileURL).launchAtLogin)
    verify(h.map("settingsStatus")["loginError"] as? String == "Login rejected")
    h.login.isEnabled = nil
    h.failure("saveSettings", ["launchAtLogin": false], code: "settings", message: "Login rejected")
    verify(h.host.current.launchAtLogin)  // Failed readback falls back to the previous value.
    h.login.error = nil
    h.success("saveSettings", ["launchAtLogin": false])
    verify(!h.host.current.launchAtLogin && h.map("settingsStatus")["loginError"] as? String == "")

    // A disk failure must not distribute an unsaved language to the engines.
    let locales = h.locales
    try FileManager.default.removeItem(at: directory)
    let failed = h.request("saveSettings", ["uiLanguage": "zh", "excludedApps": ["memory-only"]])
    verify(failed.responses.count == 1 && failed.responses[0] != nil)
    let error = h.codec.decodeEnvelope(failed.responses[0]!) as? FlutterError
    verify(
      error?.code == "settings" && error?.message?.hasPrefix("Settings could not be saved:") == true
    )
    verify(h.locales == locales && h.host.current.uiLanguage == "en")
    verify(h.host.current.excludedApps == ["memory-only"])
    try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
    h.success("saveSettings", ["uiLanguage": "zh"])
    verify(StoredSettings.load(from: h.store.fileURL).uiLanguage == "zh")

    // Startup publishes registration and login failures through the same status owner.
    h.backend.rejected = [initial.currentAppKeyCode]
    h.login.error = "Startup login rejected"
    h.host.start()
    verify(h.host.hotKeyError?.contains("use the menu bar icon") == true)
    verify(h.map("settingsStatus")["loginError"] as? String == "Startup login rejected")
  }

  private static func catalogAndResume(_ root: URL) throws {
    let directory = root.appendingPathComponent("catalog")
    try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
    let h = SettingsHostHarness(directory)
    h.host.start()
    let oldSnapshot = h.host.current.asMap()
    let binding: [String: Any] = [
      "actionId": "test.action", "title": "Test", "keyCode": 23, "modifiers": 2, "global": true,
    ]
    h.success(
      "saveSettings",
      [
        "actionShortcutVersion": 4, "actionShortcuts": [binding],
        "supportedActionIds": ["test.action"],
      ]
    )
    verify(h.backend.active.values.contains(17))
    let registrations = h.backend.registrations
    h.failure(
      "syncActionShortcutCatalog",
      ["settings": oldSnapshot, "supportedActionIds": []],
      code: "shortcuts",
      message: "Shortcut configuration changed. Refresh settings and try again."
    )
    verify(h.backend.registrations == registrations && h.backend.active.values.contains(17))
    for ids in [
      [""], [String(repeating: "😀", count: 513)], [17],
      Array(repeating: "test.action", count: RuntimePolicy.maxShortcutCatalogRequestIds + 1),
    ] as [Any] {
      h.failure(
        "syncActionShortcutCatalog",
        ["settings": h.host.current.asMap(), "supportedActionIds": ids],
        code: "shortcuts",
        message: "Invalid shortcut catalog."
      )
    }
    h.success(
      "syncActionShortcutCatalog",
      ["settings": h.host.current.asMap(), "supportedActionIds": []]
    )
    verify(h.backend.active.count == 2 && h.host.current.actionShortcuts?.count == 1)
    h.success(
      "syncActionShortcutCatalog",
      ["settings": h.host.current.asMap(), "supportedActionIds": ["test.action"]]
    )
    h.hotKeys.setActionShortcutsSuspended(true)
    h.backend.rejected = [17]
    h.failure(
      "resumeActionShortcuts",
      nil,
      code: "shortcuts",
      message: "Test shortcut is unavailable (-1)"
    )
    verify(h.backend.active.count == 2)
    verify(h.map("settingsStatus")["hotkeyError"] as? String == "Test shortcut is unavailable (-1)")
    h.backend.rejected = []
    h.success("resumeActionShortcuts")
    verify(h.backend.active.count == 3 && h.map("settingsStatus")["hotkeyError"] as? String == "")
    // Unowned methods must leave the dispatcher free to route elsewhere.
    var replied = false
    verify(
      !h.host.handle(FlutterMethodCall(methodName: "show", arguments: nil)) { _ in replied = true }
    )
    verify(!replied && h.request("unknown").responses == [nil])
  }

  private static func fileAccessAndPrivacy(_ root: URL) throws {
    let directory = root.appendingPathComponent("access")
    try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
    let grantFile = directory.appendingPathComponent("grants.plist")
    var approval: (([URL]) -> Void)?
    var prompts = 0
    let access = FileSearchAccess(
      storage: grantFile,
      request: { _, reply in
        prompts += 1
        approval = reply
      },
      work: { $0() }
    )
    let h = SettingsHostHarness(directory, fileAccess: access)
    for arguments: Any in [
      ["enabled": true, "roots": ["relative"]],
      ["enabled": true, "roots": Array(repeating: directory.path, count: 9)],
      ["enabled": true], "malformed",
    ] {
      h.failure(
        "prepareFileSearchAccess",
        arguments,
        code: "file_access",
        message: "Invalid search scope."
      )
    }
    verify(prompts == 0)
    let first = h.request("prepareFileSearchAccess", ["enabled": true, "roots": [directory.path]])
    verify(prompts == 1 && first.responses.isEmpty)
    let disabled = h.request(
      "prepareFileSearchAccess",
      ["enabled": false, "roots": [directory.path]]
    )
    verify(disabled.responses.count == 1)
    approval?([directory])
    verify(first.responses.count == 1 && first.responses[0] != nil)
    let superseded = h.codec.decodeEnvelope(first.responses[0]!) as? [String: Any]
    verify(superseded?["superseded"] as? Bool == true)
    verify(!access.ready && !FileManager.default.fileExists(atPath: grantFile.path))
    let approved = h.request(
      "prepareFileSearchAccess",
      ["enabled": true, "roots": [directory.path]]
    )
    approval?([directory])
    verify(approved.responses.count == 1 && access.ready)
    verify(h.map("settings")["fileSearchAccessReady"] as? Bool == true)
    let forced = h.request(
      "prepareFileSearchAccess",
      ["enabled": true, "roots": [directory.path], "force": true]
    )
    verify(prompts == 3 && forced.responses.isEmpty)
    approval?([])
    verify(forced.responses.count == 1)
    h.failure(
      "openPrivacySettings",
      ["pane": "unknown"],
      code: "privacy",
      message: "Unknown privacy setting."
    )
    verify(h.privacyPanes.isEmpty)
    for name in ["accessibility", "automation", "clipboard", "fullDiskAccess"] {
      h.success("openPrivacySettings", ["pane": name])
    }
    verify(h.privacyPanes == [.accessibility, .automation, .clipboard, .fullDiskAccess])
    h.privacyAvailable = false
    h.failure(
      "openPrivacySettings",
      ["pane": "clipboard"],
      code: "privacy",
      message: "System Settings could not be opened."
    )
  }
}
