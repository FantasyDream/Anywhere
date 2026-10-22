import AppKit
import FlutterMacOS

/// Window lifetime and focus only. History, storage and all widgets remain in
/// Rust/Flutter. The engine is created on demand and destroyed on explicit close.
final class ToolWindowController: NSObject, NSWindowDelegate {
    private(set) var window: NSWindow?
    private(set) var controller: FlutterViewController?
    private var channel: FlutterMethodChannel?
    private var ready = false
    private var localeHost: LocaleHost?
    private let entrypoint: String
    private let channelName: String
    private let title: () -> String
    private var localeObservers: [NSObjectProtocol] = []
    private var activationObserver: NSObjectProtocol?
    private var destination: NSRunningApplication?
    private let focusReturner = ApplicationFocusReturner()

    init(entrypoint: String, channelName: String, title: @autoclosure @escaping () -> String) {
        self.entrypoint = entrypoint
        self.channelName = channelName
        self.title = title
        super.init()
        rememberDestination(NSWorkspace.shared.frontmostApplication)
        activationObserver = NSWorkspace.shared.notificationCenter.addObserver(
            forName: NSWorkspace.didActivateApplicationNotification, object: nil, queue: .main
        ) { [weak self] notification in
            self?.rememberDestination(notification.userInfo?[NSWorkspace.applicationUserInfoKey] as? NSRunningApplication)
        }
        for name in [LocaleHost.changed, NSLocale.currentLocaleDidChangeNotification] {
            localeObservers.append(NotificationCenter.default.addObserver(forName: name,
                object: nil, queue: .main) { [weak self] _ in
                    guard let self else { return }
                    self.window?.title = self.title()
                })
        }
    }

    deinit {
        if let activationObserver { NSWorkspace.shared.notificationCenter.removeObserver(activationObserver) }
        for observer in localeObservers { NotificationCenter.default.removeObserver(observer) }
    }

    private func rememberDestination(_ app: NSRunningApplication?) {
        guard let app, !app.isTerminated,
              app.processIdentifier != ProcessInfo.processInfo.processIdentifier,
              app.activationPolicy == .regular else { return }
        destination = app
    }

    func show() -> Bool {
        rememberDestination(NSWorkspace.shared.frontmostApplication)
        let pointer = NSEvent.mouseLocation
        let screen = NSScreen.screens.first { NSMouseInRect(pointer, $0.frame, false) }
            ?? window?.screen ?? NSScreen.main ?? NSScreen.screens.first
        if window == nil {
            let view = FlutterViewController(project: FlutterDartProject())
            localeHost = LocaleHost(messenger: view.engine.binaryMessenger)
            let transport = FlutterMethodChannel(name: channelName, binaryMessenger: view.engine.binaryMessenger)
            transport.setMethodCallHandler { [weak self] call, reply in
                guard let self else { reply(nil); return }
                switch call.method {
                case "paths": reply(["library": HostRuntime.libraryPath])
                case "capabilities": reply(HostRuntime.capabilities)
                case "ready":
                    self.ready = true
                    reply(self.isVisible)
                case "close":
                    reply(nil)
                    DispatchQueue.main.async { [weak self] in self?.window?.performClose(nil) }
                case "returnToTarget":
                    let origin = self.window
                    self.focusReturner.returnFocus(to: self.destination,
                        mayReturn: { [weak self, weak origin] in
                            guard let origin else { return false }
                            return self?.window === origin && origin.isKeyWindow
                        }, completion: { outcome in reply(outcome.rawValue) })
                default: reply(FlutterMethodNotImplemented)
                }
            }
            guard view.engine.run(withEntrypoint: entrypoint) else {
                transport.setMethodCallHandler(nil)
                localeHost = nil
                return false
            }
            let size = ToolWindowPlacement.preferredSize
            let panel = NSWindow(contentRect: NSRect(origin: .zero, size: size),
                styleMask: [.titled, .closable, .miniaturizable, .resizable, .fullSizeContentView],
                backing: .buffered, defer: false)
            panel.title = title()
            panel.titleVisibility = .hidden
            panel.titlebarAppearsTransparent = true
            panel.appearance = NSAppearance(named: .darkAqua)
            panel.level = .floating
            panel.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary]
            panel.backgroundColor = .clear
            panel.isOpaque = false
            panel.minSize = NSSize(width: 360, height: 360)
            panel.isReleasedWhenClosed = false
            panel.delegate = self
            view.backgroundColor = .clear
            panel.contentViewController = WindowMaterialController(contentController: view,
                frame: NSRect(origin: .zero, size: size), cornerRadius: 0)
            panel.setContentSize(size)
            window = panel
            controller = view
            channel = transport
        }
        NSApp.unhide(nil)
        NSApp.activate(ignoringOtherApps: true)
        if window?.isMiniaturized == true { window?.deminiaturize(nil) }
        if let window, let screen {
            window.setFrame(ToolWindowPlacement.bottomTrailing(size: window.frame.size,
                            in: screen.visibleFrame), display: false)
        }
        window?.makeKeyAndOrderFront(nil)
        if let window, let content = controller?.view {
            WindowInputFocus.restore(in: window, content: content)
        }
        if ready { channel?.invokeMethod("present", arguments: nil) }
        return true
    }

    private var isVisible: Bool {
        window?.isVisible == true && window?.isMiniaturized == false && !NSApp.isHidden
    }
    private func publishVisibility() {
        if ready { channel?.invokeMethod("visibility", arguments: isVisible) }
    }
    func windowDidMiniaturize(_ notification: Notification) { publishVisibility() }
    func windowDidDeminiaturize(_ notification: Notification) { publishVisibility() }
    func applicationVisibilityChanged() { publishVisibility() }

    func windowWillClose(_ notification: Notification) {
        guard notification.object as? NSWindow === window else { return }
        channel?.invokeMethod("visibility", arguments: false)
        channel?.setMethodCallHandler(nil)
        window?.contentViewController = nil
        window = nil
        localeHost = nil
        controller = nil
        channel = nil
        ready = false
    }
}
