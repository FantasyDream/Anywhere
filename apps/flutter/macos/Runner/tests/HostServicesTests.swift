import AppKit
import Carbon
import Foundation

final class FakeHotKeys: HotKeyBackend {
  var active: [UInt32: UInt32] = [:]
  var rejected: Set<UInt32> = []
  var exclusivity: [UInt32: Bool] = [:]
  var installStatus: OSStatus = noErr
  var cleared = 0
  var onActivate: ((UInt32) -> Void)?
  func install(onActivate: @escaping (UInt32) -> Void) -> OSStatus {
    self.onActivate = onActivate
    return installStatus
  }
  func unregister(id: UInt32) { active.removeValue(forKey: id) }
  func clear() {
    cleared += 1
    active.removeAll()
  }
  func register(code: UInt32, modifiers: UInt32, id: UInt32, exclusive: Bool) -> OSStatus {
    if rejected.contains(code) { return -1 }
    active[id] = code
    exclusivity[id] = exclusive
    return noErr
  }
  func shutdown() { clear() }
}

@main
struct HostServicesTests {
  static func main() throws {
    let recorder = ShortcutRecorder()
    var chords: [[String: Int]?] = []
    recorder.start(id: 1) { chords.append($0) }
    assert(recorder.handle(code: 17, flags: [.control, .option, .shift], repeatKey: true))
    assert(chords.isEmpty && recorder.isRecording)
    assert(
      recorder.handle(code: 17, flags: [.control, .option, .shift, .capsLock], repeatKey: false)
    )
    assert(chords.count == 1 && chords[0]?["keyCode"] == 23)
    assert(chords[0]?["modifiers"] == 7)
    assert(!recorder.isRecording)  // General flags work without undocumented side flags.
    assert(!recorder.handle(code: 17, flags: .command, repeatKey: false))
    recorder.start(id: 2) { chords.append($0) }
    recorder.cancel(id: 1)  // A late cancel cannot cancel a newer request.
    assert(recorder.isRecording)
    assert(recorder.handle(code: 99, flags: [], repeatKey: false))
    assert(recorder.isRecording && chords.count == 1)
    assert(recorder.handle(code: 53, flags: [], repeatKey: false))
    assert(chords.count == 2 && chords[1] == nil && !recorder.isRecording)
    recorder.start(id: 3) { chords.append($0) }
    recorder.start(id: 4) { chords.append($0) }
    assert(chords.count == 3 && chords[2] == nil)
    recorder.cancel()  // Window focus loss/dispose.
    assert(chords.count == 4 && chords[3] == nil && !recorder.isRecording)
    recorder.start(id: 5) { chords.append($0) }
    assert(recorder.accept(code: 49, modifiers: UInt32(cmdKey | optionKey)))
    assert(chords[4]?["keyCode"] == 44 && chords[4]?["modifiers"] == 10)
    assert(!recorder.accept(code: 49, modifiers: UInt32(cmdKey)))
    let temp = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    try FileManager.default.createDirectory(at: temp, withIntermediateDirectories: true)
    defer { try? FileManager.default.removeItem(at: temp) }
    let store = SettingsStore(fileURL: temp.appendingPathComponent("settings.json"))
    let previous = store.current
    assert(previous.asMap()["uiLanguage"] as? String == "system")
    var localized = previous
    assert(localized.apply(["uiLanguage": "zh"]) == nil)
    let languageFile = temp.appendingPathComponent("language.json")
    assert(localized.save(to: languageFile) == nil)
    let restoredLanguage = StoredSettings.load(from: languageFile)
    assert(restoredLanguage.uiLanguage == "zh")
    assert(restoredLanguage.sameHotKeys(as: previous))
    assert(localized.apply(["uiLanguage": "unsupported"]) != nil)
    assert(localized.uiLanguage == "zh")
    assert(previous.asMap()["fileSearchEngine"] as? String == "systemIndex")
    assert(previous.asMap()["fileSearchIncludeHidden"] as? Bool == true)
    var searchSettings = previous
    assert(
      searchSettings.apply(["fileSearchEngine": "combined", "fileSearchIncludeHidden": true]) == nil
    )
    assert(searchSettings.save(to: temp.appendingPathComponent("search.json")) == nil)
    let loadedSearch = StoredSettings.load(from: temp.appendingPathComponent("search.json"))
    assert(
      loadedSearch.fileSearchEngine == "combined" && loadedSearch.fileSearchIncludeHidden == true
    )
    assert(searchSettings.apply(["fileSearchEngine": "arbitrary"]) != nil)
    var normalizedLegacy = previous
    assert(normalizedLegacy.apply(previous.asMap()) == nil)
    assert(normalizedLegacy.actionShortcuts == [])
    // Absent list equals transported empty list.
    assert(normalizedLegacy.sameActionShortcutConfiguration(as: previous))
    var altered = normalizedLegacy
    altered.actionShortcuts = [
      StoredActionShortcut(
        actionId: "core.directory.copy",
        title: "Copy",
        keyCode: 8,
        modifiers: UInt32(optionKey)
      )
    ]
    assert(!altered.sameActionShortcutConfiguration(as: previous))
    altered.actionShortcutVersion = 5
    assert(!altered.sameActionShortcutConfiguration(as: normalizedLegacy))
    var byteLimits = previous
    let byteId = String(repeating: "😀", count: 512)
    let byteTitle = String(repeating: "😀", count: 128)
    byteLimits.actionShortcuts = [
      StoredActionShortcut(
        actionId: byteId,
        title: byteTitle,
        keyCode: 17,
        modifiers: UInt32(optionKey)
      )
    ]
    assert(byteLimits.actionShortcutError() == nil)
    byteLimits.actionShortcuts = [
      StoredActionShortcut(
        actionId: byteId + "a",
        title: byteTitle,
        keyCode: 17,
        modifiers: UInt32(optionKey)
      )
    ]
    assert(byteLimits.actionShortcutError() != nil)
    byteLimits.actionShortcuts = [
      StoredActionShortcut(
        actionId: byteId,
        title: byteTitle + "a",
        keyCode: 17,
        modifiers: UInt32(optionKey)
      )
    ]
    assert(byteLimits.actionShortcutError() != nil)
    let backend = FakeHotKeys()
    let service = HotKeyService(backend: backend, onActivate: { _ in })
    assert(service.apply(previous) == nil)
    var proposed = previous
    proposed.currentAppKeyCode = 0
    proposed.launchAtLogin = true
    backend.rejected = [0]
    let failed = service.update(proposed, previous: previous)
    assert(failed.error != nil && failed.settings.sameHotKeys(as: previous))
    assert(failed.settings.launchAtLogin)  // unrelated accepted preferences survive
    assert(backend.active == [1: previous.currentAppKeyCode, 2: previous.universalKeyCode])
    assert(service.lastError == nil)

    let clears = backend.cleared
    proposed = previous
    proposed.universalModifiers = proposed.currentAppModifiers
    let conflict = service.update(proposed, previous: previous)
    assert(conflict.error != nil && backend.cleared == clears)
    assert(conflict.settings.sameHotKeys(as: previous))

    proposed = previous
    proposed.currentAppKeyCode = 0
    backend.rejected = [0, previous.currentAppKeyCode]
    let unrestored = service.update(proposed, previous: previous)
    assert(unrestored.error?.contains("could not be restored") == true)
    assert(service.lastError?.contains("could not be restored") == true)
    assert(unrestored.settings.sameHotKeys(as: previous))
    service.shutdown()
    assert(backend.active.isEmpty)

    let actionsBackend = FakeHotKeys()
    var activations: [String] = []
    let actions = HotKeyService(backend: actionsBackend, onActivate: { activations.append($0) })
    var global = previous
    global.actionShortcuts = [
      StoredActionShortcut(
        actionId: "core.directory.terminal",
        title: "Terminal",
        keyCode: 17,
        modifiers: UInt32(optionKey),
        global: true
      )
    ]
    assert(actions.apply(global) == nil)
    let initialId = actionsBackend.active.first { $0.value == 17 }!.key
    assert(actionsBackend.exclusivity[1] == false && actionsBackend.exclusivity[2] == false)
    // New action keys use exclusive Carbon registration; wake behavior is preserved.
    assert(actionsBackend.exclusivity[initialId] == true)
    actionsBackend.onActivate?(initialId)
    assert(activations == ["action:core.directory.terminal"])
    let wakeClears = actionsBackend.cleared
    actions.setActionShortcutsSuspended(true)
    assert(actionsBackend.active.count == 2)  // Recorder receives action keys, wake keys remain.
    actionsBackend.onActivate?(initialId)
    assert(activations.count == 1)
    actions.setActionShortcutsSuspended(false)
    assert(actionsBackend.cleared == wakeClears)
    let resumedId = actionsBackend.active.first { $0.value == 17 }!.key
    assert(resumedId != initialId)
    actionsBackend.onActivate?(initialId)  // Queued event cannot resolve to new registration.
    assert(activations.count == 1)
    actionsBackend.onActivate?(resumedId)
    assert(activations.count == 2)
    var changed = global
    changed.actionShortcuts = [
      StoredActionShortcut(
        actionId: "core.directory.copy",
        title: "Copy",
        keyCode: 0,
        modifiers: UInt32(optionKey),
        global: true
      )
    ]
    actionsBackend.rejected = [0]
    let rejected = actions.update(changed, previous: global)
    assert(rejected.error != nil && rejected.settings.actionShortcuts == global.actionShortcuts)
    assert(actionsBackend.active.values.contains(17) && !actionsBackend.active.values.contains(0))
    actionsBackend.onActivate?(resumedId)
    assert(activations.count == 2)
    let restoredId = actionsBackend.active.first { $0.value == 17 }!.key
    actionsBackend.onActivate?(restoredId)
    assert(activations.last == "action:core.directory.terminal")
    changed = global
    changed.actionShortcuts = []
    assert(actions.update(changed, previous: global).error == nil)
    assert(actionsBackend.active.count == 2)
    actionsBackend.onActivate?(restoredId)
    assert(activations.count == 3)
    actions.shutdown()
    assert(actionsBackend.active.isEmpty)
    actionsBackend.onActivate?(restoredId)
    assert(activations.count == 3)
    // Production starts with no action registrations until Core supplies eligibility.
    let catalogBackend = FakeHotKeys()
    var catalogActivations: [String] = []
    let catalog = HotKeyService(
      backend: catalogBackend,
      supportedActionIds: [],
      onActivate: { catalogActivations.append($0) }
    )
    var catalogSettings = global
    catalogSettings.actionShortcuts?.append(
      StoredActionShortcut(
        actionId: "plugin.copy",
        title: "Plugin",
        keyCode: 8,
        modifiers: UInt32(optionKey),
        global: true
      )
    )
    assert(catalog.apply(catalogSettings) == nil && catalogBackend.active.count == 2)
    let wakeCount = catalogBackend.cleared
    assert(catalog.syncActionCatalog(["core.directory.terminal", "plugin.copy"]) == nil)
    let pluginEvent = catalogBackend.active.first { $0.value == 8 }!.key
    assert(catalogBackend.active.count == 4)
    assert(catalog.syncActionCatalog(["core.directory.terminal"]) == nil)
    assert(catalogBackend.active.count == 3 && !catalogBackend.active.values.contains(8))
    catalogBackend.onActivate?(pluginEvent)
    assert(catalogActivations.isEmpty)  // Revoked queued event is inert.
    assert(catalogSettings.actionShortcuts?.count == 2)  // Configuration retained.
    catalog.setActionShortcutsSuspended(true)
    assert(catalog.syncActionCatalog(["plugin.copy"]) == nil && catalogBackend.active.count == 2)
    catalog.setActionShortcutsSuspended(false)
    let restoredPluginEvent = catalogBackend.active.first { $0.value == 8 }!.key
    assert(restoredPluginEvent != pluginEvent && !catalogBackend.active.values.contains(17))
    assert(catalogBackend.cleared == wakeCount)  // Wake registrations never rebuilt.
    catalogBackend.onActivate?(restoredPluginEvent)
    assert(catalogActivations == ["action:plugin.copy"])
    var addition = catalogSettings
    addition.actionShortcuts?.append(
      StoredActionShortcut(
        actionId: "plugin.new",
        title: "New",
        keyCode: 0,
        modifiers: UInt32(optionKey),
        global: true
      )
    )
    catalogBackend.rejected = [0]
    let rejectedAddition = catalog.update(
      addition,
      previous: catalogSettings,
      supportedActionIds: ["plugin.copy", "plugin.new"]
    )
    assert(
      rejectedAddition.error != nil
        && rejectedAddition.settings.actionShortcuts == catalogSettings.actionShortcuts
    )
    assert(catalogBackend.active.values.contains(8) && !catalogBackend.active.values.contains(0))
    catalog.setActionShortcutsSuspended(true)
    catalog.setActionShortcutsSuspended(false)
    // Previous eligibility restored too.
    assert(catalogBackend.active.values.contains(8) && !catalogBackend.active.values.contains(0))
    catalogBackend.rejected = [8]
    catalog.setActionShortcutsSuspended(true)
    catalog.setActionShortcutsSuspended(false)
    assert(catalog.lastError != nil)
    catalogBackend.rejected = []
    // Successful catalog retry clears resume failure.
    assert(catalog.syncActionCatalog(["plugin.copy"]) == nil && catalog.lastError == nil)
    assert(catalog.syncActionCatalog(["plugin.copy", "core.directory.terminal"]) == nil)
    catalog.setActionShortcutsSuspended(true)
    catalogBackend.rejected = [8]
    assert(catalog.resumeForSettingsClose() != nil)
    assert(catalogBackend.active.count == 2)  // Partial success is released; wake keys remain.
    catalogBackend.rejected = []
    assert(catalog.resumeForSettingsClose() == nil && catalog.lastError == nil)
    assert(catalogBackend.active.count == 4)
    assert(catalog.syncActionCatalog(["plugin.copy"]) == nil)
    catalogBackend.rejected = [17]
    assert(catalog.syncActionCatalog(["core.directory.terminal", "plugin.copy"]) != nil)
    assert(catalog.lastError != nil && catalogBackend.active.count == 3)
    catalogBackend.rejected = []
    assert(
      catalog.syncActionCatalog(["core.directory.terminal", "plugin.copy"]) == nil
        && catalog.lastError == nil
    )
    catalog.shutdown()

    var presses = HotKeyPressState()
    assert(presses.accept(100, pressed: true))
    assert(!presses.accept(100, pressed: true))
    assert(!presses.accept(100, pressed: false))
    assert(presses.accept(100, pressed: true))

    store.current.fileSearchEnabled = false
    store.current.fileSearchRoots = ["/Users/test/Documents"]
    store.current.fileSearchExcludedNames = ["logs"]
    store.current.excludedApps = ["test.hidden.app"]
    assert(store.save() == nil)
    let restored = SettingsStore(fileURL: store.fileURL)
    assert(restored.current.excludedApps == ["test.hidden.app"])
    assert(restored.current.fileSearchEnabled == false)
    assert(restored.current.fileSearchRoots == ["/Users/test/Documents"])
    assert(restored.current.fileSearchExcludedNames == ["logs"])
    var invalidSearch = restored.current
    invalidSearch.fileSearchRoots = ["relative"]
    assert(invalidSearch.fileSearchError() != nil)
    assert(invalidSearch.save(to: store.fileURL) != nil)
    assert(
      SettingsStore(fileURL: store.fileURL).current.fileSearchRoots == ["/Users/test/Documents"]
    )
    store.current.actionShortcutVersion = 4
    store.current.actionShortcuts = [
      StoredActionShortcut(
        actionId: "core.directory.terminal",
        title: "Open Terminal Here",
        keyCode: 17,
        modifiers: UInt32(optionKey)
      )
    ]
    assert(store.current.actionShortcutError() == nil)
    assert(store.save() == nil)
    let shortcutRestored = SettingsStore(fileURL: store.fileURL)
    assert(shortcutRestored.current.actionShortcuts?.first?.actionId == "core.directory.terminal")
    assert(shortcutRestored.current.actionShortcuts?.first?.keyCode == 17)
    // Legacy local binding.
    assert(shortcutRestored.current.actionShortcuts?.first?.global == false)
    store.current = global
    assert(store.save() == nil)
    assert(SettingsStore(fileURL: store.fileURL).current.actionShortcuts?.first?.global == true)
    var equivalentLegacy = global
    equivalentLegacy.actionShortcuts?[0].routes = []
    // Absent routes and empty rules carry the same meaning.
    assert(equivalentLegacy.actionShortcuts == global.actionShortcuts)
    var conflicting = store.current
    conflicting.currentAppKeyCode = 17
    conflicting.currentAppModifiers = UInt32(optionKey)
    assert(conflicting.actionShortcutError()?.contains("conflicts") == true)
    assert(service.update(conflicting, previous: store.current).error != nil)
    conflicting = store.current
    let duplicate = conflicting.actionShortcuts!.first!
    conflicting.actionShortcuts?.append(duplicate)
    assert(conflicting.actionShortcutError()?.contains("unique") == true)
    conflicting = store.current
    conflicting.actionShortcuts = [
      StoredActionShortcut(actionId: "copy", title: "Copy", keyCode: 8, modifiers: UInt32(cmdKey))
    ]
    assert(conflicting.actionShortcutError()?.contains("reserved") == true)
    conflicting = store.current
    assert(conflicting.apply(["actionShortcutVersion": 5, "actionShortcuts": []]) != nil)
    let portableDisk =
      try JSONSerialization.jsonObject(with: Data(contentsOf: store.fileURL)) as! [String: Any]
    assert(portableDisk["actionShortcutVersion"] as? Int == 4)
    let portableBindings = portableDisk["actionShortcuts"] as! [[String: Any]]
    assert(portableBindings[0]["keyCode"] as? Int == 23)
    assert(portableBindings[0]["modifiers"] as? Int == 2)
    assert(shortcutRestored.current.currentAppKeyCode == store.current.currentAppKeyCode)
    var oldLocal =
      try JSONSerialization.jsonObject(with: JSONEncoder().encode(store.current)) as! [String: Any]
    oldLocal["actionShortcutVersion"] = 1
    // A pre-global file remains local even if an unknown global field was supplied.
    try JSONSerialization.data(withJSONObject: oldLocal).write(to: store.fileURL)
    let migrated = SettingsStore(fileURL: store.fileURL).current
    assert(migrated.actionShortcutVersion == 4 && migrated.actionShortcuts?.first?.global == false)
    assert(migrated.actionShortcuts?.first?.actionId == global.actionShortcuts?.first?.actionId)
    var previousGlobal =
      try JSONSerialization.jsonObject(with: JSONEncoder().encode(global)) as! [String: Any]
    previousGlobal["actionShortcutVersion"] = 2
    try JSONSerialization.data(withJSONObject: previousGlobal).write(to: store.fileURL)
    let upgradedGlobal = SettingsStore(fileURL: store.fileURL).current
    assert(
      upgradedGlobal.actionShortcutVersion == 4
        && upgradedGlobal.actionShortcuts?.first?.global == true
    )
    var intentSettings = previous
    let rule = StoredShortcutRoute(
      actionId: "core.directory.terminal",
      title: "Terminal",
      appBundleId: "com.apple.finder",
      appName: "Finder",
      contextType: "core.directory"
    )
    intentSettings.actionShortcuts = [
      StoredActionShortcut(
        actionId: "intent:test",
        title: "Terminal by context",
        keyCode: 17,
        modifiers: UInt32(optionKey),
        global: true,
        routes: [rule]
      )
    ]
    assert(intentSettings.actionShortcutError() == nil)
    assert(intentSettings.save(to: store.fileURL) == nil)
    assert(
      SettingsStore(fileURL: store.fileURL).current.actionShortcuts?.first?.routes?.first == rule
    )
    var legacyIntent = intentSettings
    legacyIntent.actionShortcutVersion = 3
    try JSONEncoder().encode(legacyIntent).write(to: store.fileURL)
    let migratedIntent = SettingsStore(fileURL: store.fileURL).current
    assert(migratedIntent.actionShortcutVersion == 4)
    assert(migratedIntent.actionShortcuts == intentSettings.actionShortcuts)
    assert(migratedIntent.currentAppKeyCode == intentSettings.currentAppKeyCode)
    var invalidIntent = intentSettings
    invalidIntent.actionShortcuts?[0].routes?.append(rule)
    assert(invalidIntent.actionShortcutError()?.contains("distinct") == true)
    var noRules = intentSettings
    noRules.actionShortcuts?[0].routes = []
    assert(noRules.actionShortcutError()?.contains("identity") == true)
    let intentBackend = FakeHotKeys()
    var received: [String] = []
    let intentService = HotKeyService(backend: intentBackend, onActivate: { received.append($0) })
    assert(intentService.apply(intentSettings) == nil)
    let intentId = intentBackend.active.first { $0.value == 17 }!.key
    intentBackend.onActivate?(intentId)
    assert(received == ["action:intent:test"])
    let changedRule = StoredShortcutRoute(
      actionId: "core.directory.copy",
      title: "Copy",
      appBundleId: "com.apple.finder",
      appName: "Finder",
      contextType: "core.directory"
    )
    var modifiedIntent = intentSettings
    modifiedIntent.actionShortcuts?[0].routes = [changedRule]
    assert(!modifiedIntent.sameHotKeys(as: intentSettings))
    assert(intentService.update(modifiedIntent, previous: intentSettings).error == nil)
    intentBackend.onActivate?(intentId)
    assert(received.count == 1)  // A rule edit invalidates queued system events.
    intentService.shutdown()
    let routeBackend = FakeHotKeys()
    let routeCatalog = HotKeyService(
      backend: routeBackend,
      supportedActionIds: ["core.directory.terminal"],
      onActivate: { _ in }
    )
    var mixedIntent = intentSettings
    mixedIntent.actionShortcuts?[0].routes?.append(
      StoredShortcutRoute(
        actionId: "plugin.copy",
        title: "Plugin",
        appBundleId: "test.app",
        appName: "Test"
      )
    )
    assert(routeCatalog.apply(mixedIntent) == nil && routeBackend.active.count == 3)
    assert(routeCatalog.syncActionCatalog(["plugin.copy"]) == nil && routeBackend.active.count == 3)
    assert(routeCatalog.syncActionCatalog([]) == nil && routeBackend.active.count == 2)
    assert(
      routeCatalog.syncActionCatalog(["core.directory.terminal"]) == nil
        && routeBackend.active.count == 3
    )
    routeCatalog.shutdown()
    var futureSettings = previous
    futureSettings.actionShortcutVersion = 5
    futureSettings.actionShortcuts = intentSettings.actionShortcuts
    let futureBackend = FakeHotKeys()
    let futureService = HotKeyService(backend: futureBackend, onActivate: { _ in })
    assert(futureService.apply(futureSettings)?.contains("Unsupported") == true)
    assert(futureBackend.active.count == 2)  // Unsupported action settings do not break wake keys.
    futureService.shutdown()
    var futureMap = previous.asMap()
    futureMap["actionShortcutVersion"] = 5
    futureMap["actionShortcuts"] = [["futureFormat": true]]
    try JSONSerialization.data(withJSONObject: futureMap).write(to: store.fileURL)
    let protected = SettingsStore(fileURL: store.fileURL).current
    assert(protected.actionShortcutVersion == 5 && protected.actionShortcutError() != nil)
    var futureKnownShape = previous.asMap()
    futureKnownShape["actionShortcutVersion"] = 5
    try JSONSerialization.data(withJSONObject: futureKnownShape).write(to: store.fileURL)
    let protectedKeys = SettingsStore(fileURL: store.fileURL).current
    assert(protectedKeys.actionShortcutVersion == 5)
    assert(protectedKeys.currentAppKeyCode == 49)
    assert(protectedKeys.save(to: store.fileURL) != nil)
    assert(NativeHotKeyMap.nativeCode(999) == nil && NativeHotKeyMap.nativeMask(16) == nil)
    for (native, usage) in NativeHotKeyMap.portableKeys {
      assert(NativeHotKeyMap.nativeCode(usage) == native)
    }
    var legacy =
      try JSONSerialization.jsonObject(with: JSONEncoder().encode(store.current)) as! [String: Any]
    for key in [
      "captureFinderSelection", "captureSelectedText", "captureBrowserCurrentPage",
      "captureClipboard",
    ] { legacy.removeValue(forKey: key) }
    legacy.removeValue(forKey: "actionShortcutVersion")
    legacy.removeValue(forKey: "actionShortcuts")
    try JSONSerialization.data(withJSONObject: legacy).write(to: store.fileURL)
    assert(SettingsStore(fileURL: store.fileURL).current.captureClipboard == nil)
    assert(SettingsStore(fileURL: store.fileURL).current.actionShortcuts == nil)
    assert(store.current.save(to: temp) != nil)
    assert(StoredSettings.uint32(-1) == nil)
    assert(StoredSettings.uint32(Int64(UInt32.max) + 1) == nil)
    assert(StoredSettings.uint32(NSNumber(value: 1.5)) == nil)
    assert(StoredSettings.uint32(NSNumber(value: UInt32.max)) == UInt32.max)
    assert(NativeHotKeyMap.label(code: 0, modifiers: UInt32(cmdKey | optionKey)) == "⌥⌘A")
    print(
      "Host services: rollback, conflicts, failed restoration, settings compatibility and key labels passed"
    )
  }
}
