import AppKit
import FlutterMacOS

/// Reparents the same view between transient Launcher and persistent Settings.
final class LauncherWindowCoordinator {
    let window: NSWindow
    let controller: FlutterViewController
    let host: WindowMaterialController
    let glass: WindowMaterial
    let launcherGlassMask: NSImage?
    weak var delegate: NSWindowDelegate?
    let tasksVisible: () -> Bool
    var settingsOpen = false
    var settingsWindow: NSWindow?
    init(window: NSWindow, controller: FlutterViewController, host: WindowMaterialController, glass: WindowMaterial,
         mask: NSImage?, delegate: NSWindowDelegate, tasksVisible: @escaping () -> Bool) {
        self.window = window; self.controller = controller; self.host = host; self.glass = glass
        launcherGlassMask = mask; self.delegate = delegate; self.tasksVisible = tasksVisible
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
            panel.delegate = delegate
            panel.standardWindowButton(.zoomButton)?.isEnabled = false
            panel.center()
            settingsWindow = panel
        }
        if settingsWindow?.contentViewController !== host {
            window.contentViewController = nil
            settingsWindow?.contentViewController = host
            glass.maskImage = nil
            glass.cornerRadius = 0
        }
        resizeWindow(height)
        if settingsWindow?.isMiniaturized == true { settingsWindow?.deminiaturize(nil) }
        settingsWindow?.makeKeyAndOrderFront(nil)
        settingsWindow?.makeFirstResponder(controller.view)
    }

    func restoreLauncherView() {
        guard settingsWindow?.contentViewController === host else { return }
        settingsWindow?.orderOut(nil)
        settingsWindow?.contentViewController = nil
        let frame = window.frame
        window.contentViewController = host
        window.setFrame(frame, display: false)
        glass.maskImage = launcherGlassMask
        glass.cornerRadius = 18
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
        if NSApp.isActive && !tasksVisible() { NSApp.hide(nil) }
    }


}
