import Carbon

protocol HotKeyBackend {
    func install(onActivate: @escaping (String) -> Void) -> OSStatus
    func clear()
    func register(code: UInt32, modifiers: UInt32, id: UInt32) -> OSStatus
    func shutdown()
}

final class CarbonHotKeyBackend: HotKeyBackend {
    private var keys: [EventHotKeyRef] = []
    private var handler: EventHandlerRef?
    private var onActivate: ((String) -> Void)?

    func install(onActivate: @escaping (String) -> Void) -> OSStatus {
        if handler != nil { return noErr }
        self.onActivate = onActivate
        var event = EventTypeSpec(eventClass: OSType(kEventClassKeyboard), eventKind: UInt32(kEventHotKeyPressed))
        let pointer = Unmanaged.passUnretained(self).toOpaque()
        return InstallEventHandler(GetApplicationEventTarget(), { _, event, pointer in
            guard let pointer, let event else { return OSStatus(eventNotHandledErr) }
            var id = EventHotKeyID()
            let status = GetEventParameter(event, EventParamName(kEventParamDirectObject),
                EventParamType(typeEventHotKeyID), nil, MemoryLayout<EventHotKeyID>.size, nil, &id)
            guard status == noErr else { return OSStatus(eventNotHandledErr) }
            let backend = Unmanaged<CarbonHotKeyBackend>.fromOpaque(pointer).takeUnretainedValue()
            switch id.id {
            case 1: backend.onActivate?("currentApp")
            case 2: backend.onActivate?("universal")
            default: return OSStatus(eventNotHandledErr)
            }
            return noErr
        }, 1, &event, pointer, &handler)
    }
    func clear() { keys.forEach { UnregisterEventHotKey($0) }; keys.removeAll() }
    func register(code: UInt32, modifiers: UInt32, id: UInt32) -> OSStatus {
        var key: EventHotKeyRef?
        let status = RegisterEventHotKey(code, modifiers, EventHotKeyID(signature: 0x4354584C, id: id),
            GetApplicationEventTarget(), 0, &key)
        if let key { keys.append(key) }
        return status
    }
    func shutdown() {
        clear()
        if let handler { RemoveEventHandler(handler) }
        handler = nil
        onActivate = nil
    }
    deinit { shutdown() }
}

struct HotKeyUpdate {
    let settings: StoredSettings
    let error: String?
}

/// Native registration and rollback. Failure does not persist a rejected chord.
final class HotKeyService {
    private let backend: HotKeyBackend
    private let onActivate: (String) -> Void
    private let onRegistered: (StoredSettings) -> Void
    private let log: ([String: Any]) -> Void
    var lastError: String?

    init(backend: HotKeyBackend = CarbonHotKeyBackend(), onActivate: @escaping (String) -> Void,
         onRegistered: @escaping (StoredSettings) -> Void = { _ in },
         log: @escaping ([String: Any]) -> Void = { _ in }) {
        self.backend = backend; self.onActivate = onActivate; self.onRegistered = onRegistered; self.log = log
    }
    func apply(_ settings: StoredSettings) -> String? {
        if settings.hotKeysConflict() { return "Current App and Universal Search use the same shortcut." }
        let installed = backend.install(onActivate: onActivate)
        guard installed == noErr else { return "Global shortcut handler failed (\(installed))" }
        backend.clear()
        let current = backend.register(code: settings.currentAppKeyCode, modifiers: settings.currentAppModifiers, id: 1)
        let universal = backend.register(code: settings.universalKeyCode, modifiers: settings.universalModifiers, id: 2)
        var errors: [String] = []
        for (status, mode, label) in [(current, "currentApp", "Current App"), (universal, "universal", "Universal")] where status != noErr {
            errors.append("\(label) shortcut is unavailable (\(status))")
            log(["event": "hotkey_registration_failed", "status": status, "mode": mode])
        }
        if !errors.isEmpty { return errors.joined(separator: "; ") }
        onRegistered(settings)
        return nil
    }
    func update(_ proposed: StoredSettings, previous: StoredSettings) -> HotKeyUpdate {
        if proposed.sameHotKeys(as: previous) { return HotKeyUpdate(settings: proposed, error: nil) }
        var next = proposed
        if proposed.hotKeysConflict() {
            next.restoreHotKeys(from: previous)
            return HotKeyUpdate(settings: next, error: "Current App and Universal Search use the same shortcut.")
        }
        if let error = apply(proposed) {
            var errors = [error]
            if let restoreError = apply(previous) {
                lastError = "Previous shortcuts could not be restored: \(restoreError)"
                errors.append(lastError!)
            } else { lastError = nil }
            next.restoreHotKeys(from: previous)
            return HotKeyUpdate(settings: next, error: errors.joined(separator: "\n"))
        }
        lastError = nil
        return HotKeyUpdate(settings: next, error: nil)
    }
    func shutdown() { backend.shutdown() }
}
