import Carbon

protocol HotKeyBackend {
    func install(onActivate: @escaping (UInt32) -> Void) -> OSStatus
    func clear()
    func unregister(id: UInt32)
    func register(code: UInt32, modifiers: UInt32, id: UInt32, exclusive: Bool) -> OSStatus
    func shutdown()
}

final class CarbonHotKeyBackend: HotKeyBackend {
    private var keys: [UInt32: EventHotKeyRef] = [:]
    private var handler: EventHandlerRef?
    private var onActivate: ((UInt32) -> Void)?

    func install(onActivate: @escaping (UInt32) -> Void) -> OSStatus {
        if handler != nil { return noErr }
        self.onActivate = onActivate
        var events = [
            EventTypeSpec(eventClass: OSType(kEventClassKeyboard), eventKind: UInt32(kEventHotKeyPressed)),
            EventTypeSpec(eventClass: OSType(kEventClassKeyboard), eventKind: UInt32(kEventHotKeyReleased))
        ]
        let pointer = Unmanaged.passUnretained(self).toOpaque()
        return InstallEventHandler(GetApplicationEventTarget(), { _, event, pointer in
            guard let pointer, let event else { return OSStatus(eventNotHandledErr) }
            var id = EventHotKeyID()
            let status = GetEventParameter(event, EventParamName(kEventParamDirectObject),
                EventParamType(typeEventHotKeyID), nil, MemoryLayout<EventHotKeyID>.size, nil, &id)
            guard status == noErr, id.signature == 0x4354584C else { return OSStatus(eventNotHandledErr) }
            let backend = Unmanaged<CarbonHotKeyBackend>.fromOpaque(pointer).takeUnretainedValue()
            if backend.presses.accept(id.id, pressed: GetEventKind(event) == UInt32(kEventHotKeyPressed)) {
                backend.onActivate?(id.id)
            }
            return noErr
        }, events.count, &events, pointer, &handler)
    }
    private var presses = HotKeyPressState()
    func unregister(id: UInt32) {
        if let key = keys.removeValue(forKey: id) { UnregisterEventHotKey(key) }
        presses.release(id)
    }
    func clear() { keys.values.forEach { UnregisterEventHotKey($0) }; keys.removeAll(); presses = HotKeyPressState() }

    func register(code: UInt32, modifiers: UInt32, id: UInt32, exclusive: Bool = true) -> OSStatus {
        var key: EventHotKeyRef?
        let status = RegisterEventHotKey(code, modifiers, EventHotKeyID(signature: 0x4354584C, id: id),
            GetApplicationEventTarget(), exclusive ? OptionBits(kEventHotKeyExclusive) : 0, &key)
        if let key { keys[id] = key }
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

/// Carbon can repeat a press while held. One activation requires one release.
struct HotKeyPressState {
    private var held = Set<UInt32>()
    mutating func accept(_ id: UInt32, pressed: Bool) -> Bool {
        if !pressed { held.remove(id); return false }
        return held.insert(id).inserted
    }
    mutating func release(_ id: UInt32) { held.remove(id) }
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
    private var registeredActions: [UInt32: String] = [:]
    private var nextActionId: UInt32 = 100
    private var configured: StoredSettings?
    private var actionsSuspended = false
    private var resumeError: String?
    private var supportedActionIds: Set<String>?
    private var catalogError: String?

    init(backend: HotKeyBackend = CarbonHotKeyBackend(), supportedActionIds: Set<String>? = nil, onActivate: @escaping (String) -> Void,
         onRegistered: @escaping (StoredSettings) -> Void = { _ in },
         log: @escaping ([String: Any]) -> Void = { _ in }) {
        self.supportedActionIds = supportedActionIds
        self.backend = backend; self.onActivate = onActivate; self.onRegistered = onRegistered; self.log = log
    }
    func apply(_ settings: StoredSettings) -> String? {
        let bindingError = settings.actionShortcutError()
        if settings.hotKeysConflict() { return "Current App and Universal Search use the same shortcut." }
        let installed = backend.install { [weak self] id in
            guard let self else { return }
            switch id {
            case 1: self.onActivate("currentApp")
            case 2: self.onActivate("universal")
            default:
                if let action = self.registeredActions[id], !self.actionsSuspended {
                    self.onActivate("action:" + action)
                }
            }
        }
        guard installed == noErr else { return "Global shortcut handler failed (\(installed))" }
        backend.clear()
        registeredActions.removeAll()
        let current = backend.register(code: settings.currentAppKeyCode, modifiers: settings.currentAppModifiers, id: 1, exclusive: false)
        let universal = backend.register(code: settings.universalKeyCode, modifiers: settings.universalModifiers, id: 2, exclusive: false)
        var errors: [String] = []
        for (status, mode, label) in [(current, "currentApp", "Current App"), (universal, "universal", "Universal")] where status != noErr {
            errors.append("\(label) shortcut is unavailable (\(status))")
            log(["event": "hotkey_registration_failed", "status": status, "mode": mode])
        }
        if let bindingError { errors.append(bindingError) }
        else { errors.append(contentsOf: registerActions(settings)) }
        if actionsSuspended { unregisterActions() }
        configured = settings
        if !errors.isEmpty { return errors.joined(separator: "; ") }
        onRegistered(settings)
        return nil
    }
    private func registerActions(_ settings: StoredSettings) -> [String] {
        var errors: [String] = []
        for binding in settings.globalActionShortcuts where catalogSupports(binding) {
            guard nextActionId < UInt32.max else { errors.append("Action shortcut identity exhausted; restart Anywhere."); break }
            let id = nextActionId
            nextActionId += 1 // Never reuse an ID: queued Carbon events cannot target a replacement binding.
            let status = backend.register(code: binding.keyCode, modifiers: binding.modifiers, id: id, exclusive: true)
            if status == noErr { registeredActions[id] = binding.actionId }
            else {
                errors.append("\(binding.title) shortcut is unavailable (\(status))")
                log(["event": "action_hotkey_registration_failed", "status": status, "action": binding.actionId])
            }
        }
        return errors
    }
    private func catalogSupports(_ binding: StoredActionShortcut) -> Bool {
        guard let supportedActionIds else { return true }
        if let routes = binding.routes, !routes.isEmpty {
            return routes.contains { supportedActionIds.contains($0.actionId) }
        }
        return supportedActionIds.contains(binding.actionId)
    }
    /// Core catalog revocation releases keys without erasing user configuration.
    /// Wake keys and their held state remain installed. Resume uses the same filter.
    func syncActionCatalog(_ ids: Set<String>) -> String? {
        supportedActionIds = ids
        unregisterActions()
        if lastError == catalogError { lastError = nil }
        catalogError = nil
        guard let configured, !actionsSuspended else { return nil }
        if let error = configured.actionShortcutError() { return error }
        let errors = registerActions(configured)
        if !errors.isEmpty {
            catalogError = errors.joined(separator: "; ")
            lastError = catalogError
        } else {
            if lastError == resumeError { lastError = nil }
            resumeError = nil
        }
        return catalogError
    }
    func update(_ proposed: StoredSettings, previous: StoredSettings, supportedActionIds: Set<String>? = nil) -> HotKeyUpdate {
        if let error = proposed.actionShortcutError() {
            return HotKeyUpdate(settings: previous, error: error)
        }
        let previousSupport = self.supportedActionIds
        if let supportedActionIds { self.supportedActionIds = supportedActionIds }
        if proposed.sameHotKeys(as: previous), previousSupport == self.supportedActionIds, lastError == nil {
            configured = proposed
            return HotKeyUpdate(settings: proposed, error: nil)
        }
        var next = proposed
        if proposed.hotKeysConflict() {
            self.supportedActionIds = previousSupport
            next.restoreHotKeys(from: previous)
            return HotKeyUpdate(settings: next, error: "Current App and Universal Search use the same shortcut.")
        }
        if let error = apply(proposed) {
            self.supportedActionIds = previousSupport
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
    /// Let the settings recorder receive keys; wake shortcuts remain registered.
    func setActionShortcutsSuspended(_ suspended: Bool) {
        guard suspended != actionsSuspended else { return }
        actionsSuspended = suspended
        if suspended { unregisterActions() }
        else if let configured {
            if let error = configured.actionShortcutError() { lastError = error; return }
            // Wake keys stay installed: resuming the recorder must not reset a held wake key.
            let errors = registerActions(configured)
            if errors.isEmpty {
                if lastError == resumeError { lastError = nil }
                resumeError = nil
            } else {
                resumeError = errors.joined(separator: "; ")
                lastError = resumeError
                log(["event": "action_hotkey_resume_failed", "detail": resumeError!])
            }
        }
    }
    private func unregisterActions() {
        for id in registeredActions.keys { backend.unregister(id: id) }
        registeredActions.removeAll()
    }
    /// Check restoration before hiding Settings. A failed group remains suspended,
    /// so a partial registration cannot look like a successful settings close.
    func resumeForSettingsClose() -> String? {
        setActionShortcutsSuspended(false)
        guard let error = resumeError else { return nil }
        setActionShortcutsSuspended(true)
        return error
    }
    func shutdown() { registeredActions.removeAll(); configured = nil; backend.shutdown() }
}
