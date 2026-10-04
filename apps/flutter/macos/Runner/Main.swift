import AppKit
import ApplicationServices
import Carbon
import FlutterMacOS
import ServiceManagement

// macOS window lifecycle only. Context capture and every action stay in Rust.
final class LauncherWindow: NSWindow {
    override var canBecomeKey: Bool { true }
    override var canBecomeMain: Bool { true }
}

final class AppDelegate: NSObject, NSApplicationDelegate, NSWindowDelegate, NSMenuItemValidation {
    var window: LauncherWindow!
    var controller: FlutterViewController!
    var glass: NSVisualEffectView!
    var channel: FlutterMethodChannel!
    var statusItem: NSStatusItem!
    var currentAppHotKey: EventHotKeyRef?
    var universalHotKey: EventHotKeyRef?
    var eventHandler: EventHandlerRef?
    var activationStart: TimeInterval = 0
    var hotKeyError: String?
    var loginError: String?
    var currentAppItem: NSMenuItem?
    var universalItem: NSMenuItem?
    var presentedGeneration: Int?
    var latestGeneration = 0
    var settingsOpen = false
    var settingsWindow: NSWindow?
    var launcherGlassMask: NSImage?
    var tasksWindow: NSWindow?
    var tasksController: FlutterViewController?
    var tasksChannel: FlutterMethodChannel?
    var tasksReady = false
    var taskStopKeyMonitor: Any?
    var pendingTaskId: String?
    var terminationPending = false
    var stored = StoredSettings.load()

    /// Application support directory for this app.
    ///
    /// On first launch under the new name the previous directory is adopted, so the
    /// usage history and plugins a user already has are kept.
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

    func applicationDidFinishLaunching(_ notification: Notification) {
        let logs = Self.supportDirectory()
        freopen(logs.appendingPathComponent("launcher.log").path, "w", stderr)
        NSApp.setActivationPolicy(.accessory)
        controller = FlutterViewController(project: FlutterDartProject())
        controller.backgroundColor = .clear
        window = LauncherWindow(contentRect: NSRect(x: 0, y: 0, width: 648, height: 64),
                                styleMask: [.borderless], backing: .buffered, defer: false)
        window.title = "Anywhere"
        window.isExcludedFromWindowsMenu = true
        let windowFrame = window.frame
        window.contentViewController = controller
        // Flutter's initial view is 0×0; preserve the requested window size when attaching it.
        window.setFrame(windowFrame, display: false)
        window.delegate = self
        window.level = .floating
        window.backgroundColor = .clear
        window.isOpaque = false
        window.hasShadow = true
        window.isMovableByWindowBackground = true
        window.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary]
        window.contentView?.wantsLayer = true
        window.contentView?.layer?.cornerRadius = 18
        window.contentView?.layer?.masksToBounds = true
        // Blur the desktop behind the transparent Flutter surface. Keep Flutter's
        // controller and responder chain intact so keyboard/IME handling stays native.
        glass = NSVisualEffectView(frame: controller.view.bounds)
        glass.autoresizingMask = [.width, .height]
        glass.material = .hudWindow
        glass.blendingMode = .behindWindow
        glass.state = .active
        glass.appearance = NSAppearance(named: .darkAqua)
        // Mask the effect itself as well as the content; a layer corner alone can
        // leave the window-server backdrop rectangular. Stretch only the center.
        let mask = NSImage(size: NSSize(width: 37, height: 37), flipped: false) { rect in
            NSColor.white.setFill()
            NSBezierPath(roundedRect: rect, xRadius: 18, yRadius: 18).fill()
            return true
        }
        mask.capInsets = NSEdgeInsets(top: 18, left: 18, bottom: 18, right: 18)
        mask.resizingMode = .stretch
        launcherGlassMask = mask
        glass.maskImage = mask
        glass.isHidden = true
        controller.view.addSubview(glass, positioned: .below, relativeTo: nil)
        channel = FlutterMethodChannel(name: "anywhere/window", binaryMessenger: controller.engine.binaryMessenger)
        channel.setMethodCallHandler { [weak self] call, reply in
            guard let self else { return }
            switch call.method {
            case "paths":
                let data = Self.supportDirectory()
                reply(["data": data.path,
                       "plugins": Bundle.main.resourceURL!.appendingPathComponent("plugins", isDirectory: true).path,
                       "hotkeyError": self.hotKeyError ?? "",
                       "loginError": self.loginError ?? "",
                       "excludedApps": self.stored.excludedApps.joined(separator: "\n"),
                       "captureFinderSelection": self.stored.captureFinderSelection == false ? "0" : "1",
                       "captureSelectedText": self.stored.captureSelectedText == false ? "0" : "1",
                       "captureBrowserCurrentPage": self.stored.captureBrowserCurrentPage == false ? "0" : "1",
                       "captureClipboard": self.stored.captureClipboard == false ? "0" : "1"])
            case "settings":
                if let enabled = self.loginItemIsEnabled() {
                    self.stored.launchAtLogin = enabled
                }
                reply(self.stored.asMap())
            case "settingsStatus":
                reply(["hotkeyError": self.hotKeyError ?? "",
                       "loginError": self.loginError ?? ""])
            case "openPrivacySettings":
                guard let args = call.arguments as? [String: Any],
                      let pane = args["pane"] as? String,
                      ["accessibility", "automation", "clipboard"].contains(pane) else {
                    reply(FlutterError(code: "privacy", message: "Unknown privacy setting.", details: nil))
                    return
                }
                let app = NSWorkspace.shared.urlForApplication(withBundleIdentifier: "com.apple.systempreferences")
                let anchor = pane == "accessibility" ? "Privacy_Accessibility" : "Privacy_Automation"
                // Clipboard preferences vary by macOS version. Open System Settings
                // itself rather than guessing an unsupported pane identifier.
                let link = pane == "clipboard" ? app : URL(string: "x-apple.systempreferences:com.apple.preference.security?\(anchor)")
                if let link, NSWorkspace.shared.open(link) {
                    reply(nil)
                } else if let app, NSWorkspace.shared.open(app) {
                    reply(nil)
                } else {
                    reply(FlutterError(code: "privacy", message: "System Settings could not be opened.", details: nil))
                }
            case "showExecutionSessions":
                let args = call.arguments as? [String: Any]
                if self.showTasks(id: args?["id"] as? String) {
                    reply(nil)
                } else {
                    reply(FlutterError(code: "tasks", message: "Running Tasks could not start.", details: nil))
                }
            case "saveSettings":
                guard let args = call.arguments as? [String: Any] else { reply(nil); return }
                let previous = self.stored
                var next = previous
                next.apply(args)
                var errors: [String] = []
                if !next.sameHotKeys(as: previous) {
                    if next.hotKeysConflict() {
                        errors.append("Current App and Universal Search use the same shortcut.")
                        next.restoreHotKeys(from: previous)
                    } else if let error = self.applyHotKeys(next) {
                        errors.append(error)
                        if let restoreError = self.applyHotKeys(previous) {
                            let restoreMessage = "Previous shortcuts could not be restored: \(restoreError)"
                            self.hotKeyError = restoreMessage
                            errors.append(restoreMessage)
                        } else {
                            self.hotKeyError = nil
                        }
                        next.restoreHotKeys(from: previous)
                    } else {
                        self.hotKeyError = nil
                    }
                }
                self.stored = next
                if next.launchAtLogin != previous.launchAtLogin {
                    let error = self.applyLoginItem()
                    self.loginError = error
                    if let error {
                        errors.append(error)
                        next.launchAtLogin = self.loginItemIsEnabled() ?? previous.launchAtLogin
                    }
                }
                self.stored = next
                if let error = self.stored.save() {
                    errors.append(error)
                }
                if errors.isEmpty {
                    reply(nil)
                } else {
                    reply(FlutterError(code: "settings", message: errors.joined(separator: "\n"), details: nil))
                }
            case "ready":
                reply(nil)
                self.activate(mode: "currentApp")
            case "show":
                guard let args = call.arguments as? [String: Any],
                      let generation = args["generation"] as? Int,
                      generation >= self.latestGeneration else { reply(nil); return }
                self.latestGeneration = generation
                self.settingsOpen = args["settings"] as? Bool == true
                self.glass.isHidden = false
                NSApp.unhide(nil)
                NSApp.activate(ignoringOtherApps: true)
                if self.settingsOpen {
                    self.showSettingsWindow(height: args["height"] as? Double ?? 508)
                } else {
                    self.restoreLauncherView()
                    self.resizeWindow(args["height"] as? Double ?? 64)
                    self.positionWindow()
                    self.window.makeKeyAndOrderFront(nil)
                    self.window.makeFirstResponder(self.controller.view)
                }
                self.presentedGeneration = generation
                let elapsed = (ProcessInfo.processInfo.systemUptime - self.activationStart) * 1000
                let visibleFrame = (self.settingsOpen ? self.settingsWindow?.frame : self.window.frame) ?? self.window.frame
                var visibility: [String: Any] = [
                    "event": self.settingsOpen ? "settings_visible" : "launcher_visible",
                    "window_width": visibleFrame.width,
                    "window_height": visibleFrame.height,
                    "ax_trusted": AXIsProcessTrusted()
                ]
                if !self.settingsOpen { visibility["hotkey_to_visible_ms"] = elapsed }
                self.log(visibility)
                reply(nil)
            case "resize":
                if let args = call.arguments as? [String: Any],
                   let generation = args["generation"] as? Int,
                   generation == self.presentedGeneration,
                   let height = args["height"] as? Double {
                    self.resizeWindow(height)
                }
                reply(nil)
            case "hide":
                if let args = call.arguments as? [String: Any],
                   let generation = args["generation"] as? Int,
                   generation >= self.latestGeneration {
                    self.latestGeneration = generation + 1
                    self.presentedGeneration = nil
                    self.settingsOpen = false
                    self.hideLauncherWindow()
                }
                reply(nil)
            default: reply(FlutterMethodNotImplemented)
            }
        }
        installMenu()
        // AppKit can route ⌘. as cancelOperation before key-equivalent menus.
        // Its characters may already be Escape, so match the period key code.
        // Consume only this task-window shortcut; Escape remains Flutter-owned.
        taskStopKeyMonitor = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { [weak self] event in
            guard let self, self.tasksReady,
                  let tasks = self.tasksWindow, NSApp.keyWindow === tasks,
                  event.keyCode == UInt16(kVK_ANSI_Period),
                  event.modifierFlags.intersection([.command, .option, .control, .shift]) == .command
            else { return event }
            self.stopTask()
            return nil
        }
        if let error = applyHotKeys(stored) {
            hotKeyError = error + "; use the menu bar icon or release the shortcut in another app."
        }
        loginError = applyLoginItem()
        // A hidden view never receives viewWillAppear, which normally starts the engine.
        if !controller.engine.run(withEntrypoint: nil) {
            log(["event": "flutter_engine_start_failed"])
            NSApp.terminate(nil)
        }
    }

    func positionWindow() {
        let pointer = NSEvent.mouseLocation
        let screen = NSScreen.screens.first { NSMouseInRect(pointer, $0.frame, false) } ?? NSScreen.main!
        let frame = screen.visibleFrame
        window.setFrameOrigin(NSPoint(x: frame.midX - window.frame.width / 2,
                                      y: max(frame.minY, frame.maxY - frame.height * 0.20 - window.frame.height)))
    }

    // Settings and Launcher share one engine, but have distinct native window lifecycles.
    // Moving the view avoids keeping an extra Impeller engine just for preferences.
    func showSettingsWindow(height: Double) {
        window.orderOut(nil)
        if settingsWindow == nil {
            let panel = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 648, height: 508),
                                 styleMask: [.titled, .closable, .miniaturizable, .fullSizeContentView],
                                 backing: .buffered, defer: false)
            panel.title = "Anywhere · Settings"
            panel.titleVisibility = .hidden
            panel.titlebarAppearsTransparent = true
            panel.appearance = NSAppearance(named: .darkAqua)
            panel.backgroundColor = .clear
            panel.isOpaque = false
            panel.isReleasedWhenClosed = false
            panel.delegate = self
            panel.standardWindowButton(.zoomButton)?.isEnabled = false
            panel.center()
            settingsWindow = panel
        }
        if settingsWindow?.contentViewController !== controller {
            window.contentViewController = nil
            settingsWindow?.contentViewController = controller
            glass.maskImage = nil
            controller.view.layer?.cornerRadius = 0
            controller.view.layer?.masksToBounds = false
        }
        resizeWindow(height)
        if settingsWindow?.isMiniaturized == true { settingsWindow?.deminiaturize(nil) }
        settingsWindow?.makeKeyAndOrderFront(nil)
        settingsWindow?.makeFirstResponder(controller.view)
    }

    func restoreLauncherView() {
        guard settingsWindow?.contentViewController === controller else { return }
        settingsWindow?.orderOut(nil)
        settingsWindow?.contentViewController = nil
        let frame = window.frame
        window.contentViewController = controller
        window.setFrame(frame, display: false)
        glass.maskImage = launcherGlassMask
        controller.view.layer?.cornerRadius = 18
        controller.view.layer?.masksToBounds = true
    }

    func resizeWindow(_ requestedHeight: Double) {
        guard requestedHeight.isFinite else { return }
        if settingsOpen, let panel = settingsWindow {
            let screen = panel.screen ?? NSScreen.main!
            let height = min(max(requestedHeight, 300), screen.visibleFrame.height)
            panel.setContentSize(NSSize(width: 648, height: height))
            return
        }
        let screen = window.screen ?? NSScreen.main!
        let height = min(max(requestedHeight, 64), min(520, screen.visibleFrame.height))
        guard window.frame.size != NSSize(width: 648, height: height) else { return }
        var frame = window.frame
        frame.origin.y = max(screen.visibleFrame.minY, frame.maxY - height)
        frame.size = NSSize(width: 648, height: height)
        window.setFrame(frame, display: window.isVisible)
        window.invalidateShadow()
    }

    func hideLauncherWindow() {
        restoreLauncherView()
        window.orderOut(nil)
        glass.isHidden = true
        // The hidden window need not retain backing surfaces for seven result rows.
        // `show` restores the requested height before ordering it front again.
        resizeWindow(64)
        if NSApp.isActive && tasksWindow?.isVisible != true { NSApp.hide(nil) }
    }

    func dismissForFocusLoss() {
        if settingsOpen || terminationPending { return }
        guard let generation = presentedGeneration else { return }
        presentedGeneration = nil
        latestGeneration = generation + 1
        hideLauncherWindow()
        log(["event": "launcher_dismissed", "reason": "focus_lost", "generation": generation])
        channel.invokeMethod("dismiss", arguments: ["generation": generation])
    }

    func windowDidResignKey(_ notification: Notification) {
        if notification.object as? NSWindow === window { dismissForFocusLoss() }
    }
    func applicationDidResignActive(_ notification: Notification) { dismissForFocusLoss() }

    @objc func activateCurrentApp() { activate(mode: "currentApp") }
    @objc func activateUniversal() { activate(mode: "universal") }
    @objc func openSettings() {
        channel.invokeMethod("openSettings", arguments: nil)
    }

    func windowShouldClose(_ sender: NSWindow) -> Bool {
        if sender === settingsWindow {
            // Flutter waits for saving and asks about unapplied drafts before hiding.
            channel.invokeMethod("closeSettings", arguments: nil)
            return false
        }
        return true
    }

    @objc func closeActiveWindow() { NSApp.keyWindow?.performClose(nil) }
    @objc func minimizeActiveWindow() { NSApp.keyWindow?.performMiniaturize(nil) }
    @objc func zoomActiveWindow() { NSApp.keyWindow?.performZoom(nil) }

    @objc func openTasks() { _ = showTasks(id: nil) }

    @objc func findTask() { tasksChannel?.invokeMethod("find", arguments: nil) }
    @objc func stopTask() { tasksChannel?.invokeMethod("stop", arguments: nil) }
    @objc func refreshTasks() { tasksChannel?.invokeMethod("refresh", arguments: nil) }
    @objc func taskLauncher() { activate(mode: "universal") }
    @objc func hideTasks() { tasksWindow?.performClose(nil) }

    func validateMenuItem(_ item: NSMenuItem) -> Bool {
        if item.action == #selector(closeActiveWindow) || item.action == #selector(minimizeActiveWindow) {
            guard let key = NSApp.keyWindow else { return false }
            return key === settingsWindow || key === tasksWindow
        }
        if item.action == #selector(zoomActiveWindow) {
            guard let key = NSApp.keyWindow else { return false }
            return key === tasksWindow
        }
        let taskActions: [Selector] = [#selector(findTask), #selector(stopTask),
                                      #selector(refreshTasks), #selector(taskLauncher), #selector(hideTasks)]
        if let action = item.action, taskActions.contains(action) {
            return tasksWindow?.isKeyWindow == true && tasksReady
        }
        return true
    }

    /// Lazy second Flutter engine: releasing its window never destroys Rust process sessions.
    func showTasks(id: String?) -> Bool {
        pendingTaskId = id
        let created = tasksWindow == nil
        if created {
            let view = FlutterViewController(project: FlutterDartProject())
            let channel = FlutterMethodChannel(name: "anywhere/executions", binaryMessenger: view.engine.binaryMessenger)
            channel.setMethodCallHandler { [weak self] call, reply in
                guard let self else { reply(nil); return }
                if call.method == "ready" {
                    self.tasksReady = true
                    reply(self.pendingTaskId)
                } else if call.method == "showLauncher" {
                    reply(nil)
                    self.activate(mode: "universal")
                } else if call.method == "hide" {
                    reply(nil)
                    DispatchQueue.main.async { [weak self] in self?.tasksWindow?.performClose(nil) }
                } else {
                    reply(FlutterMethodNotImplemented)
                }
            }
            guard view.engine.run(withEntrypoint: "executionSessionsMain") else {
                channel.setMethodCallHandler(nil)
                return false
            }
            let panel = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 920, height: 600),
                                 styleMask: [.titled, .closable, .miniaturizable, .resizable, .fullSizeContentView], backing: .buffered, defer: false)
            panel.title = "Anywhere · Running Tasks"
            panel.titleVisibility = .hidden
            panel.titlebarAppearsTransparent = true
            panel.appearance = NSAppearance(named: .darkAqua)
            panel.backgroundColor = .clear
            panel.isOpaque = false
            panel.minSize = NSSize(width: 760, height: 420)
            panel.isReleasedWhenClosed = false
            panel.delegate = self
            panel.contentViewController = view
            view.backgroundColor = .clear
            let material = NSVisualEffectView(frame: view.view.bounds)
            material.autoresizingMask = [.width, .height]
            material.material = .hudWindow
            material.blendingMode = .behindWindow
            material.state = .active
            view.view.addSubview(material, positioned: .below, relativeTo: nil)
            panel.setContentSize(NSSize(width: 920, height: 600))
            panel.center()
            tasksWindow = panel
            tasksController = view
            tasksChannel = channel
        }
        NSApp.unhide(nil)
        NSApp.activate(ignoringOtherApps: true)
        if tasksWindow?.isMiniaturized == true { tasksWindow?.deminiaturize(nil) }
        tasksWindow?.makeKeyAndOrderFront(nil)
        if created, let view = tasksController?.view {
            tasksWindow?.makeFirstResponder(view)
        }
        if tasksReady {
            tasksChannel?.invokeMethod("visibility", arguments: true)
            if let id { tasksChannel?.invokeMethod("select", arguments: id) }
        }
        return true
    }

    func windowWillClose(_ notification: Notification) {
        guard notification.object as? NSWindow === tasksWindow else { return }
        tasksChannel?.invokeMethod("visibility", arguments: false)
        tasksChannel?.setMethodCallHandler(nil)
        tasksWindow?.contentViewController = nil
        tasksWindow = nil
        tasksController = nil
        tasksChannel = nil
        tasksReady = false
        pendingTaskId = nil
    }

    func windowDidMiniaturize(_ notification: Notification) {
        if notification.object as? NSWindow === tasksWindow {
            tasksChannel?.invokeMethod("visibility", arguments: false)
        }
    }

    func windowDidDeminiaturize(_ notification: Notification) {
        if notification.object as? NSWindow === tasksWindow {
            tasksChannel?.invokeMethod("visibility", arguments: true)
        }
    }

    func applicationDidHide(_ notification: Notification) {
        tasksChannel?.invokeMethod("visibility", arguments: false)
    }

    func applicationDidUnhide(_ notification: Notification) {
        if tasksWindow?.isVisible == true && tasksWindow?.isMiniaturized == false {
            tasksChannel?.invokeMethod("visibility", arguments: true)
        }
    }

    func activate(mode: String) {
        activationStart = ProcessInfo.processInfo.systemUptime
        log(["event": "activation_requested", "mode": mode])
        channel.invokeMethod("toggle", arguments: ["mode": mode])
    }

    func applyHotKeys(_ settings: StoredSettings) -> String? {
        if eventHandler == nil {
            var event = EventTypeSpec(eventClass: OSType(kEventClassKeyboard), eventKind: UInt32(kEventHotKeyPressed))
            let pointer = Unmanaged.passUnretained(self).toOpaque()
            let handlerStatus = InstallEventHandler(GetApplicationEventTarget(), { _, event, userData in
                guard let userData, let event else { return OSStatus(eventNotHandledErr) }
                var hotKeyID = EventHotKeyID()
                let status = GetEventParameter(
                    event,
                    EventParamName(kEventParamDirectObject),
                    EventParamType(typeEventHotKeyID),
                    nil,
                    MemoryLayout<EventHotKeyID>.size,
                    nil,
                    &hotKeyID
                )
                guard status == noErr else { return OSStatus(eventNotHandledErr) }
                let delegate = Unmanaged<AppDelegate>.fromOpaque(userData).takeUnretainedValue()
                switch hotKeyID.id {
                case 1: delegate.activate(mode: "currentApp")
                case 2: delegate.activate(mode: "universal")
                default: break
                }
                return noErr
            }, 1, &event, pointer, &eventHandler)
            guard handlerStatus == noErr else {
                return "Global shortcut handler failed (\(handlerStatus))"
            }
        }
        if settings.hotKeysConflict() {
            return "Current App and Universal Search use the same shortcut."
        }
        return registerHotKeys(settings)
    }

    func registerHotKeys(_ settings: StoredSettings) -> String? {
        if let hotKey = currentAppHotKey { UnregisterEventHotKey(hotKey); currentAppHotKey = nil }
        if let hotKey = universalHotKey { UnregisterEventHotKey(hotKey); universalHotKey = nil }
        let currentId = EventHotKeyID(signature: 0x4354584C, id: 1)
        let currentStatus = RegisterEventHotKey(settings.currentAppKeyCode, settings.currentAppModifiers, currentId,
                                        GetApplicationEventTarget(), 0, &currentAppHotKey)
        let universalId = EventHotKeyID(signature: 0x4354584C, id: 2)
        let universalStatus = RegisterEventHotKey(settings.universalKeyCode, settings.universalModifiers,
                                        universalId, GetApplicationEventTarget(), 0, &universalHotKey)
        var errors: [String] = []
        if currentStatus != noErr {
            errors.append("Current App shortcut is unavailable (\(currentStatus))")
            log(["event": "hotkey_registration_failed", "status": currentStatus, "mode": "currentApp"])
        }
        if universalStatus != noErr {
            errors.append("Universal shortcut is unavailable (\(universalStatus))")
            log(["event": "hotkey_registration_failed", "status": universalStatus, "mode": "universal"])
        }
        if !errors.isEmpty {
            return errors.joined(separator: "; ")
        }
        updateMenuTitles(settings)
        return nil
    }

    func updateMenuTitles(_ settings: StoredSettings) {
        currentAppItem?.title = "Current App  \(settings.shortcutLabel(code: settings.currentAppKeyCode, modifiers: settings.currentAppModifiers))"
        universalItem?.title = "Universal Search  \(settings.shortcutLabel(code: settings.universalKeyCode, modifiers: settings.universalModifiers))"
    }

    func applyLoginItem() -> String? {
        let want = stored.launchAtLogin
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
                stored.launchAtLogin = service.status == .enabled
                return "Open at login failed: \(error)"
            }
            let enabled = service.status == .enabled
            stored.launchAtLogin = enabled
            if want && !enabled {
                return "Open at login was not enabled."
            }
            return nil
        }
        guard let current = legacyLoginEnabled() else {
            return "Open at login could not be read."
        }
        if want == current {
            stored.launchAtLogin = want
            return nil
        }
        let path = Bundle.main.bundlePath
            .replacingOccurrences(of: "\\", with: "\\\\")
            .replacingOccurrences(of: "\"", with: "\\\"")
        let script = want
            ? "tell application \"System Events\" to make login item at end with properties {path:\"\(path)\", hidden:false}"
            : "tell application \"System Events\" to delete (every login item whose path is \"\(path)\")"
        let status = runOsascript(script)
        guard let enabled = legacyLoginEnabled() else {
            return "Open at login could not be confirmed."
        }
        stored.launchAtLogin = enabled
        if status != 0 || enabled != want {
            return "Open at login failed."
        }
        return nil
    }

    func loginItemIsEnabled() -> Bool? {
        if #available(macOS 13.0, *) {
            return SMAppService.mainApp.status == .enabled
        }
        return legacyLoginEnabled()
    }

    func legacyLoginEnabled() -> Bool? {
        let path = Bundle.main.bundlePath
        let output = runOsascriptOutput("""
        tell application "System Events"
            if (count of login items) is 0 then return ""
            set AppleScript's text item delimiters to linefeed
            return (path of every login item) as text
        end tell
        """)
        if osascriptStatus != 0 { return nil }
        return output.split(separator: "\n").contains(Substring(path))
    }

    @discardableResult
    func runOsascript(_ script: String) -> Int32 {
        _ = runOsascriptOutput(script)
        return osascriptStatus
    }

    private var osascriptStatus: Int32 = 0

    func runOsascriptOutput(_ script: String) -> String {
        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/usr/bin/osascript")
        process.arguments = ["-e", script]
        let pipe = Pipe()
        process.standardOutput = pipe
        process.standardError = Pipe()
        do {
            try process.run()
        } catch {
            osascriptStatus = 1
            return ""
        }
        process.waitUntilExit()
        osascriptStatus = process.terminationStatus
        let data = pipe.fileHandleForReading.readDataToEndOfFile()
        return String(data: data, encoding: .utf8)?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
    }

    func installMenu() {
        statusItem = NSStatusBar.system.statusItem(withLength: NSStatusItem.squareLength)
        statusItem.button?.image = NSImage(systemSymbolName: "sparkle.magnifyingglass", accessibilityDescription: "Anywhere")
        let menu = NSMenu()
        let current = NSMenuItem(title: "Current App", action: #selector(activateCurrentApp), keyEquivalent: "")
        current.target = self
        currentAppItem = current
        menu.addItem(current)
        let universal = NSMenuItem(title: "Universal Search", action: #selector(activateUniversal), keyEquivalent: "")
        universal.target = self
        universalItem = universal
        menu.addItem(universal)
        let tasks = NSMenuItem(title: "Running Tasks…", action: #selector(openTasks), keyEquivalent: "")
        tasks.target = self
        menu.addItem(tasks)
        menu.addItem(.separator())
        let settingsItem = NSMenuItem(title: "Settings…", action: #selector(openSettings), keyEquivalent: ",")
        settingsItem.target = self
        menu.addItem(settingsItem)
        menu.addItem(.separator())
        let quit = NSMenuItem(title: "Quit Anywhere", action: #selector(NSApplication.terminate(_:)), keyEquivalent: "q")
        quit.target = NSApp
        menu.addItem(quit)
        statusItem.menu = menu

        // Flutter text fields need the native Edit menu for standard ⌘V / ⌘C shortcuts.
        let main = NSMenu()
        let appItem = NSMenuItem()
        main.addItem(appItem)
        appItem.submenu = menu.copy() as? NSMenu
        let editItem = NSMenuItem(title: "Edit", action: nil, keyEquivalent: "")
        let edit = NSMenu(title: "Edit")
        for (title, action, key) in [("Undo", "undo:", "z"), ("Cut", "cut:", "x"), ("Copy", "copy:", "c"), ("Paste", "paste:", "v"), ("Select All", "selectAll:", "a")] {
            edit.addItem(NSMenuItem(title: title, action: Selector(action), keyEquivalent: key))
        }
        editItem.submenu = edit
        main.addItem(editItem)
        // Expose task commands through macOS menus. The local monitor reserves ⌘.;
        // Flutter owns Escape's clear-filter / hide logic.
        let taskItem = NSMenuItem(title: "Tasks", action: nil, keyEquivalent: "")
        let taskMenu = NSMenu(title: "Tasks")
        for (title, action, key) in [
            ("Find Task…", #selector(findTask), "f"),
            ("Stop Selected Task", #selector(stopTask), "."),
            ("Refresh Tasks", #selector(refreshTasks), "r"),
            ("Open Launcher", #selector(taskLauncher), "l"),
            ("Close Task Window", #selector(hideTasks), "")
        ] {
            let item = NSMenuItem(title: title, action: action, keyEquivalent: key)
            item.target = self
            taskMenu.addItem(item)
        }
        taskItem.submenu = taskMenu
        main.addItem(taskItem)
        let windowItem = NSMenuItem(title: "Window", action: nil, keyEquivalent: "")
        let windowMenu = NSMenu(title: "Window")
        for (title, action, key) in [
            ("Close Window", #selector(closeActiveWindow), "w"),
            ("Minimize", #selector(minimizeActiveWindow), "m"),
            ("Zoom", #selector(zoomActiveWindow), "")
        ] {
            let item = NSMenuItem(title: title, action: action, keyEquivalent: key)
            item.target = self
            windowMenu.addItem(item)
        }
        windowItem.submenu = windowMenu
        main.addItem(windowItem)
        NSApp.windowsMenu = windowMenu
        NSApp.mainMenu = main
    }

    func log(_ value: [String: Any]) {
        var value = value
        let formatter = ISO8601DateFormatter()
        formatter.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        value["timestamp"] = formatter.string(from: Date())
        if let data = try? JSONSerialization.data(withJSONObject: value, options: [.sortedKeys]),
           let line = String(data: data, encoding: .utf8) {
            FileHandle.standardError.write(Data((line + "\n").utf8))
        }
    }

    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool { false }
    func applicationShouldHandleReopen(_ sender: NSApplication, hasVisibleWindows flag: Bool) -> Bool {
        if !flag {
            if settingsOpen { openSettings() }
            else { activate(mode: "currentApp") }
        }
        return true
    }
    func applicationShouldTerminate(_ sender: NSApplication) -> NSApplication.TerminateReply {
        if terminationPending { return .terminateLater }
        terminationPending = true
        channel.invokeMethod("prepareToQuit", arguments: nil) { [weak self] result in
            let allow = result as? Bool == true
            if !allow { self?.terminationPending = false }
            NSApp.reply(toApplicationShouldTerminate: allow)
        }
        return .terminateLater
    }
    func applicationWillTerminate(_ notification: Notification) {
        if let taskStopKeyMonitor { NSEvent.removeMonitor(taskStopKeyMonitor) }
        if let currentAppHotKey { UnregisterEventHotKey(currentAppHotKey) }
        if let universalHotKey { UnregisterEventHotKey(universalHotKey) }
        if let eventHandler { RemoveEventHandler(eventHandler) }
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
        AppDelegate.supportDirectory().appendingPathComponent("settings.json")
    }

    static func load() -> StoredSettings {
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

    func save() -> String? {
        do {
            let data = try JSONEncoder().encode(self)
            try data.write(to: Self.fileURL, options: .atomic)
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
        var label = ""
        if modifiers & UInt32(controlKey) != 0 { label += "⌃" }
        if modifiers & UInt32(optionKey) != 0 { label += "⌥" }
        if modifiers & UInt32(shiftKey) != 0 { label += "⇧" }
        if modifiers & UInt32(cmdKey) != 0 { label += "⌘" }
        label += Self.keyName(code)
        return label
    }

    static func keyName(_ code: UInt32) -> String {
        switch Int(code) {
        case kVK_Space: return "Space"
        case kVK_Return: return "Return"
        case kVK_Tab: return "Tab"
        case kVK_Escape: return "Esc"
        case kVK_ANSI_A: return "A"
        case kVK_ANSI_B: return "B"
        case kVK_ANSI_C: return "C"
        case kVK_ANSI_D: return "D"
        case kVK_ANSI_E: return "E"
        case kVK_ANSI_F: return "F"
        case kVK_ANSI_G: return "G"
        case kVK_ANSI_H: return "H"
        case kVK_ANSI_I: return "I"
        case kVK_ANSI_J: return "J"
        case kVK_ANSI_K: return "K"
        case kVK_ANSI_L: return "L"
        case kVK_ANSI_M: return "M"
        case kVK_ANSI_N: return "N"
        case kVK_ANSI_O: return "O"
        case kVK_ANSI_P: return "P"
        case kVK_ANSI_Q: return "Q"
        case kVK_ANSI_R: return "R"
        case kVK_ANSI_S: return "S"
        case kVK_ANSI_T: return "T"
        case kVK_ANSI_U: return "U"
        case kVK_ANSI_V: return "V"
        case kVK_ANSI_W: return "W"
        case kVK_ANSI_X: return "X"
        case kVK_ANSI_Y: return "Y"
        case kVK_ANSI_Z: return "Z"
        default: return "Key"
        }
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
        if let value = value as? UInt32 { return value }
        if let value = value as? Int { return UInt32(value) }
        if let value = value as? Int64 { return UInt32(value) }
        if let value = value as? NSNumber { return value.uint32Value }
        return nil
    }
}

let application = NSApplication.shared
let delegate = AppDelegate()
application.delegate = delegate
application.run()
