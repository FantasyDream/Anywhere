import AppKit
import ApplicationServices
import Carbon
import FlutterMacOS

// macOS window lifecycle only. Context capture and every action stay in Rust.
final class LauncherWindow: NSWindow {
    override var canBecomeKey: Bool { true }
    override var canBecomeMain: Bool { true }
}

final class AppDelegate: NSObject, NSApplicationDelegate, NSWindowDelegate {
    var window: LauncherWindow!
    var controller: FlutterViewController!
    var channel: FlutterMethodChannel!
    var statusItem: NSStatusItem!
    var currentAppHotKey: EventHotKeyRef?
    var universalHotKey: EventHotKeyRef?
    var eventHandler: EventHandlerRef?
    var activationStart: TimeInterval = 0
    var hotKeyError: String?
    var presentedGeneration: Int?
    var latestGeneration = 0

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
        window.contentView?.layer?.cornerRadius = 12
        window.contentView?.layer?.masksToBounds = true
        // Blur the desktop behind the transparent Flutter surface. Keep Flutter's
        // controller and responder chain intact so keyboard/IME handling stays native.
        let glass = NSVisualEffectView(frame: controller.view.bounds)
        glass.autoresizingMask = [.width, .height]
        glass.material = .hudWindow
        glass.blendingMode = .behindWindow
        glass.state = .active
        glass.appearance = NSAppearance(named: .darkAqua)
        // Mask the effect itself as well as the content; a layer corner alone can
        // leave the window-server backdrop rectangular. Stretch only the center.
        let mask = NSImage(size: NSSize(width: 25, height: 25), flipped: false) { rect in
            NSColor.white.setFill()
            NSBezierPath(roundedRect: rect, xRadius: 12, yRadius: 12).fill()
            return true
        }
        mask.capInsets = NSEdgeInsets(top: 12, left: 12, bottom: 12, right: 12)
        mask.resizingMode = .stretch
        glass.maskImage = mask
        controller.view.addSubview(glass, positioned: .below, relativeTo: nil)
        channel = FlutterMethodChannel(name: "anywhere/window", binaryMessenger: controller.engine.binaryMessenger)
        channel.setMethodCallHandler { [weak self] call, reply in
            guard let self else { return }
            switch call.method {
            case "paths":
                let data = Self.supportDirectory()
                reply(["data": data.path,
                       "plugins": Bundle.main.resourceURL!.appendingPathComponent("plugins", isDirectory: true).path,
                       "hotkeyError": self.hotKeyError ?? ""])
            case "ready":
                reply(nil)
                self.activate(mode: "currentApp")
            case "show":
                guard let args = call.arguments as? [String: Any],
                      let generation = args["generation"] as? Int,
                      generation >= self.latestGeneration else { reply(nil); return }
                self.latestGeneration = generation
                self.resizeWindow(args["height"] as? Double ?? 64)
                self.positionWindow()
                NSApp.unhide(nil)
                NSApp.activate(ignoringOtherApps: true)
                self.window.makeKeyAndOrderFront(nil)
                self.presentedGeneration = generation
                let elapsed = (ProcessInfo.processInfo.systemUptime - self.activationStart) * 1000
                self.log(["event": "launcher_visible", "hotkey_to_visible_ms": elapsed,
                          "window_width": self.window.frame.width, "window_height": self.window.frame.height,
                          "ax_trusted": AXIsProcessTrusted()])
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
                    self.window.orderOut(nil)
                    if NSApp.isActive { NSApp.hide(nil) }
                }
                reply(nil)
            default: reply(FlutterMethodNotImplemented)
            }
        }
        installMenu()
        registerHotKey()
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

    func resizeWindow(_ requestedHeight: Double) {
        guard requestedHeight.isFinite else { return }
        let screen = window.screen ?? NSScreen.main!
        let height = min(max(requestedHeight, 64), min(520, screen.visibleFrame.height))
        guard window.frame.size != NSSize(width: 648, height: height) else { return }
        var frame = window.frame
        frame.origin.y = max(screen.visibleFrame.minY, frame.maxY - height)
        frame.size = NSSize(width: 648, height: height)
        window.setFrame(frame, display: true)
        window.invalidateShadow()
    }

    func dismissForFocusLoss() {
        guard let generation = presentedGeneration else { return }
        presentedGeneration = nil
        latestGeneration = generation + 1
        window.orderOut(nil)
        if NSApp.isActive { NSApp.hide(nil) }
        log(["event": "launcher_dismissed", "reason": "focus_lost", "generation": generation])
        channel.invokeMethod("dismiss", arguments: ["generation": generation])
    }

    func windowDidResignKey(_ notification: Notification) { dismissForFocusLoss() }
    func applicationDidResignActive(_ notification: Notification) { dismissForFocusLoss() }

    @objc func activateCurrentApp() { activate(mode: "currentApp") }
    @objc func activateUniversal() { activate(mode: "universal") }

    func activate(mode: String) {
        activationStart = ProcessInfo.processInfo.systemUptime
        log(["event": "activation_requested", "mode": mode])
        channel.invokeMethod("toggle", arguments: ["mode": mode])
    }

    func registerHotKey() {
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
            hotKeyError = "Global shortcut handler failed (\(handlerStatus)); use the menu bar icon."
            return
        }
        let currentId = EventHotKeyID(signature: 0x4354584C, id: 1)
        let currentStatus = RegisterEventHotKey(UInt32(kVK_Space), UInt32(cmdKey | optionKey), currentId,
                                        GetApplicationEventTarget(), 0, &currentAppHotKey)
        let universalId = EventHotKeyID(signature: 0x4354584C, id: 2)
        let universalStatus = RegisterEventHotKey(UInt32(kVK_Space), UInt32(cmdKey | optionKey | shiftKey),
                                        universalId, GetApplicationEventTarget(), 0, &universalHotKey)
        var errors: [String] = []
        if currentStatus != noErr {
            errors.append("⌘⌥Space is unavailable (\(currentStatus))")
            log(["event": "hotkey_registration_failed", "status": currentStatus, "mode": "currentApp"])
        }
        if universalStatus != noErr {
            errors.append("⌘⌥⇧Space is unavailable (\(universalStatus))")
            log(["event": "hotkey_registration_failed", "status": universalStatus, "mode": "universal"])
        }
        if !errors.isEmpty {
            hotKeyError = errors.joined(separator: "; ") + "; use the menu bar icon or release the shortcut in another app."
        }
    }

    func installMenu() {
        statusItem = NSStatusBar.system.statusItem(withLength: NSStatusItem.squareLength)
        statusItem.button?.image = NSImage(systemSymbolName: "sparkle.magnifyingglass", accessibilityDescription: "Anywhere")
        let menu = NSMenu()
        let current = NSMenuItem(title: "Current App  ⌘⌥Space", action: #selector(activateCurrentApp), keyEquivalent: "")
        current.target = self
        menu.addItem(current)
        let universal = NSMenuItem(title: "Universal Search  ⌘⌥⇧Space", action: #selector(activateUniversal), keyEquivalent: "")
        universal.target = self
        menu.addItem(universal)
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
    func applicationWillTerminate(_ notification: Notification) {
        if let currentAppHotKey { UnregisterEventHotKey(currentAppHotKey) }
        if let universalHotKey { UnregisterEventHotKey(universalHotKey) }
        if let eventHandler { RemoveEventHandler(eventHandler) }
    }
}

let application = NSApplication.shared
let delegate = AppDelegate()
application.delegate = delegate
application.run()
