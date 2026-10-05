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
            "captureClipboard": captureClipboard ?? true
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

    mutating func apply(_ args: [String: Any]) {
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
