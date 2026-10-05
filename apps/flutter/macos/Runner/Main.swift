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
    lazy var hotKeys = HotKeyService(onActivate: { [weak self] mode in self?.activate(mode: mode) },
        onRegistered: { [weak self] settings in self?.updateMenuTitles(settings) },
        log: { [weak self] value in self?.log(value) })
    var activationStart: TimeInterval = 0
    var hotKeyError: String? {
        get { hotKeys.lastError }
        set { hotKeys.lastError = newValue }
    }
    var loginError: String?
    var currentAppItem: NSMenuItem?
    var universalItem: NSMenuItem?
    var presentedGeneration: Int?
    var latestGeneration = 0
    lazy var launcherWindows = LauncherWindowCoordinator(window: window, controller: controller,
        glass: glass, mask: launcherGlassMask, delegate: self,
        tasksVisible: { [weak self] in self?.tasksWindow?.isVisible == true })
    var settingsOpen: Bool {
        get { launcherWindows.settingsOpen }
        set { launcherWindows.settingsOpen = newValue }
    }
    var settingsWindow: NSWindow? { launcherWindows.settingsWindow }
    var launcherGlassMask: NSImage?
    lazy var taskWindows = TasksWindowController(onLauncher: { [weak self] in self?.activate(mode: "universal") })
    var tasksWindow: NSWindow? { taskWindows.tasksWindow }
    var tasksChannel: FlutterMethodChannel? { taskWindows.tasksChannel }
    var tasksReady: Bool { taskWindows.tasksReady }
    var taskStopKeyMonitor: Any?
    var terminationPending = false
    let settingsStore = SettingsStore()
    var stored: StoredSettings {
        get { settingsStore.current }
        set { settingsStore.current = newValue }
    }

    /// Application support directory for this app.
    ///
    /// On first launch under the new name the previous directory is adopted, so the
    /// usage history and plugins a user already has are kept.
    static func supportDirectory() -> URL { SettingsStore.supportDirectory() }

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
                let update = self.hotKeys.update(next, previous: previous)
                next = update.settings
                if let error = update.error { errors.append(error) }
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
                if let error = self.settingsStore.save() {
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

    func positionWindow() { launcherWindows.positionWindow() }
    func showSettingsWindow(height: Double) { launcherWindows.showSettingsWindow(height: height) }
    func restoreLauncherView() { launcherWindows.restoreLauncherView() }
    func resizeWindow(_ height: Double) { launcherWindows.resizeWindow(height) }
    func hideLauncherWindow() { launcherWindows.hideLauncherWindow() }

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
    func showTasks(id: String?) -> Bool { taskWindows.showTasks(id: id) }
    func applicationDidHide(_ notification: Notification) { taskWindows.applicationDidHide(notification) }
    func applicationDidUnhide(_ notification: Notification) { taskWindows.applicationDidUnhide(notification) }

    func activate(mode: String) {
        activationStart = ProcessInfo.processInfo.systemUptime
        log(["event": "activation_requested", "mode": mode])
        channel.invokeMethod("toggle", arguments: ["mode": mode])
    }

    func applyHotKeys(_ settings: StoredSettings) -> String? { hotKeys.apply(settings) }

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
        hotKeys.shutdown()
    }
}


@main
struct AnywhereHost {
    static func main() {
        let application = NSApplication.shared
        let delegate = AppDelegate()
        application.delegate = delegate
        withExtendedLifetime(delegate) { application.run() }
    }
}
