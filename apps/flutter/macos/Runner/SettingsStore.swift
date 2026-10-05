import Foundation
import Carbon

/// Persistence and legacy-directory migration, independent of the window lifecycle.
final class SettingsStore {
    let fileURL: URL
    var current: StoredSettings
    init(fileURL: URL = StoredSettings.fileURL) {
        self.fileURL = fileURL
        current = StoredSettings.load(from: fileURL)
    }
    func save() -> String? { current.save(to: fileURL) }
    static func supportDirectory() -> URL {
        let base = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
        let current = base.appendingPathComponent("Anywhere", isDirectory: true)
        if !FileManager.default.fileExists(atPath: current.path) {
            let previous = base.appendingPathComponent("ContextLauncher", isDirectory: true)
            if FileManager.default.fileExists(atPath: previous.path) {
                try? FileManager.default.moveItem(at: previous, to: current)
            }
        }
        try? FileManager.default.createDirectory(at: current, withIntermediateDirectories: true)
        return current
    }

}

struct StoredSettings: Codable {
    var currentAppKeyCode: UInt32
    var currentAppModifiers: UInt32
    var universalKeyCode: UInt32
    var universalModifiers: UInt32
    var launchAtLogin: Bool
    var excludedApps: [String]
    // Optional for compatibility with settings.json written before V0.4.
    var captureFinderSelection: Bool?
    var captureSelectedText: Bool?
    var captureBrowserCurrentPage: Bool?
    var captureClipboard: Bool?
    // Missing fields preserve settings written by 0.5 and earlier.
    var actionShortcutVersion: Int? = nil
    var actionShortcuts: [StoredActionShortcut]? = nil

    static var fileURL: URL {
        SettingsStore.supportDirectory().appendingPathComponent("settings.json")
    }

    static func load(from fileURL: URL = Self.fileURL) -> StoredSettings {
        let defaults = StoredSettings(
            currentAppKeyCode: UInt32(kVK_Space),
            currentAppModifiers: UInt32(cmdKey | optionKey),
            universalKeyCode: UInt32(kVK_Space),
            universalModifiers: UInt32(cmdKey | optionKey | shiftKey),
            launchAtLogin: false,
            excludedApps: [],
            captureFinderSelection: nil,
            captureSelectedText: nil,
            captureBrowserCurrentPage: nil,
            captureClipboard: nil
        )
        guard let data = try? Data(contentsOf: fileURL),
              let stored = try? JSONDecoder().decode(StoredSettings.self, from: data) else {
            return defaults
        }
        return stored
    }

    func save(to fileURL: URL = Self.fileURL) -> String? {
        do {
            let data = try JSONEncoder().encode(self)
            try data.write(to: fileURL, options: .atomic)
            return nil
        } catch {
            return "Settings could not be saved: \(error.localizedDescription)"
        }
    }

    func asMap() -> [String: Any] {
        [
            "currentAppKeyCode": Int(currentAppKeyCode),
            "currentAppModifiers": Int(currentAppModifiers),
            "universalKeyCode": Int(universalKeyCode),
            "universalModifiers": Int(universalModifiers),
            "launchAtLogin": launchAtLogin,
            "excludedApps": excludedApps,
            "captureFinderSelection": captureFinderSelection ?? true,
            "captureSelectedText": captureSelectedText ?? true,
            "captureBrowserCurrentPage": captureBrowserCurrentPage ?? true,
            "captureClipboard": captureClipboard ?? true,
            "actionShortcutVersion": actionShortcutVersion ?? 1,
            "actionShortcuts": (actionShortcuts ?? []).map { $0.asMap() }
        ]
    }

    func sameHotKeys(as other: StoredSettings) -> Bool {
        currentAppKeyCode == other.currentAppKeyCode &&
            currentAppModifiers == other.currentAppModifiers &&
            universalKeyCode == other.universalKeyCode &&
            universalModifiers == other.universalModifiers
    }

    mutating func restoreHotKeys(from other: StoredSettings) {
        currentAppKeyCode = other.currentAppKeyCode
        currentAppModifiers = other.currentAppModifiers
        universalKeyCode = other.universalKeyCode
        universalModifiers = other.universalModifiers
    }

    func hotKeysConflict() -> Bool {
        currentAppKeyCode == universalKeyCode && currentAppModifiers == universalModifiers
    }

    func shortcutLabel(code: UInt32, modifiers: UInt32) -> String {
        NativeHotKeyMap.label(code: code, modifiers: modifiers)
    }

    mutating func apply(_ args: [String: Any]) -> String? {
        if let raw = args["actionShortcuts"] {
            guard args["actionShortcutVersion"] as? Int == 1,
                  let data = try? JSONSerialization.data(withJSONObject: raw),
                  let bindings = try? JSONDecoder().decode([StoredActionShortcut].self, from: data)
            else { return "Unsupported or invalid action shortcut settings." }
            actionShortcutVersion = 1
            actionShortcuts = bindings
        }
        if let value = Self.uint32(args["currentAppKeyCode"]) { currentAppKeyCode = value }
        if let value = Self.uint32(args["currentAppModifiers"]) { currentAppModifiers = value }
        if let value = Self.uint32(args["universalKeyCode"]) { universalKeyCode = value }
        if let value = Self.uint32(args["universalModifiers"]) { universalModifiers = value }
        if let value = args["launchAtLogin"] as? Bool { launchAtLogin = value }
        if let value = args["excludedApps"] as? [String] {
            excludedApps = value
        }
        if let value = args["captureFinderSelection"] as? Bool { captureFinderSelection = value }
        if let value = args["captureSelectedText"] as? Bool { captureSelectedText = value }
        if let value = args["captureBrowserCurrentPage"] as? Bool { captureBrowserCurrentPage = value }
        if let value = args["captureClipboard"] as? Bool { captureClipboard = value }
        return actionShortcutError()
    }

    func actionShortcutError() -> String? {
        guard actionShortcutVersion == nil || actionShortcutVersion == 1 else {
            return "Unsupported action shortcut settings version."
        }
        let bindings = actionShortcuts ?? []
        if bindings.count > 128 { return "At most 128 action shortcuts are supported." }
        var actions = Set<String>(), chords = Set<String>()
        let allowed = UInt32(cmdKey | optionKey | controlKey | shiftKey)
        for binding in bindings {
            guard !binding.actionId.isEmpty, binding.actionId.count <= 2048,
                  binding.title.count <= 512,
                  let name = NativeHotKeyMap.keys[binding.keyCode], name.count == 1,
                  name.range(of: "^[A-Z]$", options: .regularExpression) != nil,
                  binding.modifiers & ~allowed == 0,
                  binding.modifiers & UInt32(cmdKey | optionKey | controlKey) != 0 else {
                return "Use a letter key with Command, Option or Control."
            }
            if binding.modifiers & UInt32(optionKey | controlKey) == 0 &&
                NativeHotKeyMap.reservedActionKeyCodes.contains(binding.keyCode) {
                return "This action shortcut is reserved for editing or window commands."
            }
            if (binding.keyCode == currentAppKeyCode && binding.modifiers == currentAppModifiers) ||
                (binding.keyCode == universalKeyCode && binding.modifiers == universalModifiers) {
                return "An action shortcut conflicts with the Anywhere wake shortcut."
            }
            if !actions.insert(binding.actionId).inserted ||
                !chords.insert("\(binding.keyCode):\(binding.modifiers)").inserted {
                return "Action shortcuts must use unique actions and key combinations."
            }
        }
        return nil
    }

    static func uint32(_ value: Any?) -> UInt32? {
        if let value = value as? Int { return UInt32(exactly: value) }
        if let value = value as? NSNumber {
            let number = value.doubleValue
            guard number.isFinite, number.rounded() == number, number >= 0, number <= Double(UInt32.max) else { return nil }
            return UInt32(number)
        }
        return nil
    }
}

struct StoredActionShortcut: Codable {
    let actionId: String
    let title: String
    let keyCode: UInt32
    let modifiers: UInt32
    func asMap() -> [String: Any] {
        ["actionId": actionId, "title": title, "keyCode": Int(keyCode), "modifiers": Int(modifiers)]
    }
}
