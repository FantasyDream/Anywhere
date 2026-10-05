import Foundation
import Carbon

final class FakeHotKeys: HotKeyBackend {
    var active: [UInt32: UInt32] = [:]
    var rejected: Set<UInt32> = []
    var installStatus: OSStatus = noErr
    var cleared = 0
    func install(onActivate: @escaping (String) -> Void) -> OSStatus { installStatus }
    func clear() { cleared += 1; active.removeAll() }
    func register(code: UInt32, modifiers: UInt32, id: UInt32) -> OSStatus {
        if rejected.contains(code) { return -1 }
        active[id] = code
        return noErr
    }
    func shutdown() { clear() }
}

@main
struct HostServicesTests {
    static func main() throws {
        let temp = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: temp, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: temp) }
        let store = SettingsStore(fileURL: temp.appendingPathComponent("settings.json"))
        let previous = store.current
        let backend = FakeHotKeys()
        let service = HotKeyService(backend: backend, onActivate: { _ in })
        assert(service.apply(previous) == nil)
        var proposed = previous
        proposed.currentAppKeyCode = 0
        proposed.launchAtLogin = true
        backend.rejected = [0]
        let failed = service.update(proposed, previous: previous)
        assert(failed.error != nil && failed.settings.sameHotKeys(as: previous))
        assert(failed.settings.launchAtLogin) // unrelated accepted preferences survive
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

        store.current.excludedApps = ["test.hidden.app"]
        assert(store.save() == nil)
        let restored = SettingsStore(fileURL: store.fileURL)
        assert(restored.current.excludedApps == ["test.hidden.app"])
        store.current.actionShortcutVersion = 1
        store.current.actionShortcuts = [StoredActionShortcut(actionId: "core.directory.terminal", title: "Open Terminal Here", keyCode: 17, modifiers: UInt32(optionKey))]
        assert(store.current.actionShortcutError() == nil)
        assert(store.save() == nil)
        let shortcutRestored = SettingsStore(fileURL: store.fileURL)
        assert(shortcutRestored.current.actionShortcuts?.first?.actionId == "core.directory.terminal")
        assert(shortcutRestored.current.actionShortcuts?.first?.keyCode == 17)
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
        conflicting.actionShortcuts = [StoredActionShortcut(actionId: "copy", title: "Copy", keyCode: 8, modifiers: UInt32(cmdKey))]
        assert(conflicting.actionShortcutError()?.contains("reserved") == true)
        conflicting = store.current
        assert(conflicting.apply(["actionShortcutVersion": 2, "actionShortcuts": []]) != nil)
        var legacy = store.current.asMap()
        for key in ["captureFinderSelection", "captureSelectedText", "captureBrowserCurrentPage", "captureClipboard"] { legacy.removeValue(forKey: key) }
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
        print("Host services: rollback, conflicts, failed restoration, settings compatibility and key labels passed")
    }
}
