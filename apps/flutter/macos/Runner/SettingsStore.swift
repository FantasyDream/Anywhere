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
    var fileSearchEnabled: Bool? = nil
    var fileSearchEngine: String? = nil
    var fileSearchIncludeHidden: Bool? = nil
    var fileSearchRoots: [String]? = nil
    var fileSearchExcludedNames: [String]? = nil
    // Missing fields preserve settings written by 0.5 and earlier.
    var actionShortcutVersion: Int? = 4
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
            captureClipboard: nil,
            fileSearchIncludeHidden: true
        )
        guard let data = try? Data(contentsOf: fileURL) else { return defaults }
        guard var stored = try? JSONDecoder().decode(StoredSettings.self, from: data) else {
            if let raw = (try? JSONSerialization.jsonObject(with: data)) as? [String: Any],
               let version = raw["actionShortcutVersion"] as? Int, ![1, 2, 3, 4].contains(version) {
                var protected = defaults
                protected.actionShortcutVersion = version
                return protected // Never allow an older writer to replace an unknown schema.
            }
            return defaults
        }
        if let version = stored.actionShortcutVersion, ![1, 2, 3, 4].contains(version) {
            var protected = defaults; protected.actionShortcutVersion = version
            return protected // The key encoding of an unknown schema must never be guessed.
        }
        if stored.actionShortcutVersion == 4 {
            guard let currentCode = NativeHotKeyMap.nativeCode(stored.currentAppKeyCode),
                  let currentMask = NativeHotKeyMap.nativeMask(stored.currentAppModifiers),
                  let universalCode = NativeHotKeyMap.nativeCode(stored.universalKeyCode),
                  let universalMask = NativeHotKeyMap.nativeMask(stored.universalModifiers),
                  let bindings = StoredActionShortcut.nativeBindings(stored.actionShortcuts ?? []) else {
                var protected = defaults
                protected.actionShortcutVersion = Int.max // Invalid portable data must not be overwritten.
                return protected
            }
            stored.currentAppKeyCode = currentCode; stored.currentAppModifiers = currentMask
            stored.universalKeyCode = universalCode; stored.universalModifiers = universalMask
            if stored.actionShortcuts != nil { stored.actionShortcuts = bindings }
        }
        if stored.actionShortcutVersion == nil || stored.actionShortcutVersion == 1 {
            stored.actionShortcutVersion = 4
            stored.actionShortcuts = stored.actionShortcuts?.map { binding in
                var local = binding; local.global = false; local.routes = nil; return local
            }
        }
        if stored.actionShortcutVersion == 2 {
            stored.actionShortcutVersion = 4
            stored.actionShortcuts = stored.actionShortcuts?.map { binding in var fixed = binding; fixed.routes = nil; return fixed }
        }
        if stored.actionShortcutVersion == 3 { stored.actionShortcutVersion = 4 }
        return stored
    }

    func save(to fileURL: URL = Self.fileURL) -> String? {
        guard actionShortcutVersion == nil || actionShortcutVersion == 4 else {
            return "Unsupported action shortcut settings version."
        }
        if let error = fileSearchError() { return error }
        do {
            let data = try JSONSerialization.data(withJSONObject: asMap(), options: [.sortedKeys])
            try data.write(to: fileURL, options: .atomic)
            return nil
        } catch {
            return "Settings could not be saved: \(error.localizedDescription)"
        }
    }

    func asMap() -> [String: Any] {
        [
            "currentAppKeyCode": Int(NativeHotKeyMap.portableKeys[currentAppKeyCode] ?? UInt32.max),
            "currentAppModifiers": Int(NativeHotKeyMap.portableMask(currentAppModifiers)),
            "universalKeyCode": Int(NativeHotKeyMap.portableKeys[universalKeyCode] ?? UInt32.max),
            "universalModifiers": Int(NativeHotKeyMap.portableMask(universalModifiers)),
            "launchAtLogin": launchAtLogin,
            "excludedApps": excludedApps,
            "captureFinderSelection": captureFinderSelection ?? true,
            "captureSelectedText": captureSelectedText ?? true,
            "captureBrowserCurrentPage": captureBrowserCurrentPage ?? true,
            "captureClipboard": captureClipboard ?? true,
            "fileSearchEnabled": fileSearchEnabled ?? true,
            "fileSearchEngine": fileSearchEngine ?? "systemIndex",
            "fileSearchIncludeHidden": fileSearchIncludeHidden ?? false,
            "fileSearchRoots": fileSearchRoots ?? [],
            "fileSearchExcludedNames": fileSearchExcludedNames ?? [],
            "actionShortcutVersion": actionShortcutVersion ?? 4,
            "actionShortcuts": (actionShortcuts ?? []).map { $0.asMap() }
        ]
    }

    /// Legacy files can omit the entire list; transport normalizes it to empty.
    func sameActionShortcutConfiguration(as other: StoredSettings) -> Bool {
        (actionShortcutVersion ?? 4) == (other.actionShortcutVersion ?? 4) &&
            (actionShortcuts ?? []) == (other.actionShortcuts ?? [])
    }
    func sameHotKeys(as other: StoredSettings) -> Bool {
        currentAppKeyCode == other.currentAppKeyCode &&
            currentAppModifiers == other.currentAppModifiers &&
            universalKeyCode == other.universalKeyCode &&
            universalModifiers == other.universalModifiers &&
            globalActionShortcuts == other.globalActionShortcuts
    }

    mutating func restoreHotKeys(from other: StoredSettings) {
        currentAppKeyCode = other.currentAppKeyCode
        currentAppModifiers = other.currentAppModifiers
        universalKeyCode = other.universalKeyCode
        universalModifiers = other.universalModifiers
        // Global registration is transactional with its persisted bindings.
        actionShortcutVersion = other.actionShortcutVersion
        actionShortcuts = other.actionShortcuts
    }

    var globalActionShortcuts: [StoredActionShortcut] {
        (actionShortcuts ?? []).filter { $0.global == true }
    }

    func hotKeysConflict() -> Bool {
        currentAppKeyCode == universalKeyCode && currentAppModifiers == universalModifiers
    }

    func shortcutLabel(code: UInt32, modifiers: UInt32) -> String {
        NativeHotKeyMap.label(code: code, modifiers: modifiers)
    }

    mutating func apply(_ args: [String: Any]) -> String? {
        guard actionShortcutVersion == nil || actionShortcutVersion == 4 else {
            return "Unsupported action shortcut settings version."
        }
        if let raw = args["actionShortcuts"] {
            guard args["actionShortcutVersion"] as? Int == 4,
                  let data = try? JSONSerialization.data(withJSONObject: raw),
                  let portable = try? JSONDecoder().decode([StoredActionShortcut].self, from: data),
                  let bindings = StoredActionShortcut.nativeBindings(portable)
            else { return "Unsupported or invalid action shortcut settings." }
            actionShortcutVersion = 4
            actionShortcuts = bindings
        }
        for key in ["currentAppKeyCode", "universalKeyCode"] where args[key] != nil {
            guard let value = Self.uint32(args[key]), let code = NativeHotKeyMap.nativeCode(value) else { return "Invalid portable shortcut key." }
            if key == "currentAppKeyCode" { currentAppKeyCode = code } else { universalKeyCode = code }
        }
        for key in ["currentAppModifiers", "universalModifiers"] where args[key] != nil {
            guard let value = Self.uint32(args[key]), let mask = NativeHotKeyMap.nativeMask(value) else { return "Invalid portable shortcut modifiers." }
            if key == "currentAppModifiers" { currentAppModifiers = mask } else { universalModifiers = mask }
        }
        if let value = args["launchAtLogin"] as? Bool { launchAtLogin = value }
        if let value = args["excludedApps"] as? [String] {
            excludedApps = value
        }
        if let value = args["captureFinderSelection"] as? Bool { captureFinderSelection = value }
        if let value = args["captureSelectedText"] as? Bool { captureSelectedText = value }
        if let value = args["captureBrowserCurrentPage"] as? Bool { captureBrowserCurrentPage = value }
        if let value = args["captureClipboard"] as? Bool { captureClipboard = value }
        if let value = args["fileSearchEnabled"] as? Bool { fileSearchEnabled = value }
        if let value = args["fileSearchEngine"] as? String { fileSearchEngine = value }
        if let value = args["fileSearchIncludeHidden"] as? Bool { fileSearchIncludeHidden = value }
        if let value = args["fileSearchRoots"] as? [String] { fileSearchRoots = value }
        if let value = args["fileSearchExcludedNames"] as? [String] { fileSearchExcludedNames = value }
        return fileSearchError() ?? actionShortcutError()
    }

    func fileSearchError() -> String? {
        if !["systemIndex", "ripgrep", "combined"].contains(fileSearchEngine ?? "systemIndex") {
            return "Unsupported file search engine."
        }
        let roots = fileSearchRoots ?? [], names = fileSearchExcludedNames ?? []
        if roots.count > RuntimePolicy.maxSearchRoots || names.count > RuntimePolicy.maxSearchExclusions { return "Use at most 8 directories and 32 exclusions." }
        func controls(_ text: String) -> Bool { text.unicodeScalars.contains { $0.value <= 31 || (127...159).contains($0.value) } }
        if roots.contains(where: { !$0.hasPrefix("/") || $0.utf8.count > RuntimePolicy.maxSearchPathBytes || controls($0) }) { return "Search directories must be absolute paths." }
        if names.contains(where: { $0.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty || $0.utf8.count > RuntimePolicy.maxSearchExclusionBytes || $0.contains("/") || $0.contains("\\") || controls($0) || $0 == "." || $0 == ".." }) { return "Exclusions must be directory names." }
        return nil
    }

    func actionShortcutError() -> String? {
        guard actionShortcutVersion == nil || actionShortcutVersion == 4 else {
            return "Unsupported action shortcut settings version."
        }
        let bindings = actionShortcuts ?? []
        if bindings.count > RuntimePolicy.maxActionShortcuts { return "At most 128 action shortcuts are supported." }
        var actions = Set<String>(), chords = Set<String>()
        let allowed = UInt32(cmdKey | optionKey | controlKey | shiftKey)
        for binding in bindings {
            guard !binding.actionId.isEmpty, binding.actionId.utf8.count <= RuntimePolicy.maxActionIdBytes,
                  binding.title.utf8.count <= RuntimePolicy.maxActionTitleBytes,
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
            if let error = binding.routeError() { return error }
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

struct StoredActionShortcut: Codable, Equatable {
    let actionId: String
    let title: String
    let keyCode: UInt32
    let modifiers: UInt32
    var global: Bool? = nil
    var routes: [StoredShortcutRoute]? = nil
    static func == (lhs: Self, rhs: Self) -> Bool {
        lhs.actionId == rhs.actionId && lhs.title == rhs.title && lhs.keyCode == rhs.keyCode && lhs.modifiers == rhs.modifiers &&
            (lhs.global == true) == (rhs.global == true) && (lhs.routes ?? []) == (rhs.routes ?? [])
    }
    /// Native representation exists only inside the macOS adapter.
    static func nativeBindings(_ portable: [StoredActionShortcut]) -> [StoredActionShortcut]? {
        var result: [StoredActionShortcut] = []
        for binding in portable {
            guard let code = NativeHotKeyMap.nativeCode(binding.keyCode),
                  let mask = NativeHotKeyMap.nativeMask(binding.modifiers) else { return nil }
            result.append(StoredActionShortcut(actionId: binding.actionId, title: binding.title,
                keyCode: code, modifiers: mask, global: binding.global, routes: binding.routes))
        }
        return result
    }
    func routeError() -> String? {
        let rules = routes ?? []
        if (!rules.isEmpty) != actionId.hasPrefix("intent:") { return "Invalid shortcut intent identity." }
        if rules.count > 16 || (!rules.isEmpty && title.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty) { return "An intent needs a name and at most 16 rules." }
        var conditions = Set<String>()
        for rule in rules {
            if !rule.valid { return "Invalid shortcut rule." }
            if !conditions.insert("\(rule.appBundleId ?? "")/\(rule.contextType ?? "")").inserted { return "Shortcut rules must use distinct app and context conditions." }
        }
        return nil
    }
    func asMap() -> [String: Any] {
        ["actionId": actionId, "title": title, "keyCode": Int(NativeHotKeyMap.portableKeys[keyCode] ?? UInt32.max), "modifiers": Int(NativeHotKeyMap.portableMask(modifiers)), "global": global == true, "routes": (routes ?? []).map { $0.asMap() }]
    }
}

struct StoredShortcutRoute: Codable, Equatable {
    let actionId: String
    let title: String
    var appBundleId: String? = nil
    var appName: String? = nil
    var contextType: String? = nil
    var valid: Bool {
        func condition(_ value: String?, max: Int) -> Bool {
            guard let value else { return true }
            return !value.isEmpty && value.count <= max && value.range(of: "^[A-Za-z0-9._-]+$", options: .regularExpression) != nil
        }
        return !actionId.isEmpty && actionId.utf8.count <= RuntimePolicy.maxActionIdBytes && title.utf8.count <= RuntimePolicy.maxActionTitleBytes && (appName?.utf8.count ?? 0) <= RuntimePolicy.maxActionTitleBytes && condition(appBundleId, max: 256) && condition(contextType, max: 128)
    }
    func asMap() -> [String: Any] {
        var map: [String: Any] = ["actionId": actionId, "title": title]
        if let appBundleId { map["appBundleId"] = appBundleId }
        if let appName { map["appName"] = appName }
        if let contextType { map["contextType"] = contextType }
        return map
    }
}
