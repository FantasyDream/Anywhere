import AppKit

/// Reparents the same view between transient Launcher and persistent Settings.
final class LauncherWindowCoordinator {
  let window: NSWindow
  let controller: NSViewController
  let host: WindowMaterialController
  let glass: WindowMaterial
  let launcherGlassMask: NSImage?
  weak var delegate: NSWindowDelegate?
  let tasksVisible: () -> Bool
  private let willShow: (Bool) -> Void
  private let willHide: () -> Void
  private var latestGeneration = 0
  private(set) var presentedGeneration: Int?
  private(set) var settingsOpen = false
  private(set) var settingsWindow: NSWindow?
  private var settingsOrigin: NSPoint?
  init(
    window: NSWindow,
    controller: NSViewController,
    host: WindowMaterialController,
    glass: WindowMaterial,
    mask: NSImage?,
    delegate: NSWindowDelegate,
    tasksVisible: @escaping () -> Bool,
    willShow: @escaping (Bool) -> Void = { _ in },
    willHide: @escaping () -> Void = {}
  ) {
    self.window = window
    self.controller = controller
    self.host = host
    self.glass = glass
    launcherGlassMask = mask
    self.delegate = delegate
    self.tasksVisible = tasksVisible
    self.willShow = willShow
    self.willHide = willHide
  }

  /// Reject stale requests before changing windows, hotkeys or input ownership.
  @discardableResult
  func show(generation: Int, settings: Bool, height: Double, width: Double = 648) -> Bool {
    guard generation >= latestGeneration else { return false }
    latestGeneration = generation
    settingsOpen = settings
    willShow(settings)
    glass.isHidden = false
    NSApp.unhide(nil)
    NSApp.activate(ignoringOtherApps: true)
    if settings {
      showSettingsWindow(height: height)
    } else {
      restoreLauncherView()
      resizeWindow(height, width: width)
      positionWindow()
      window.makeKeyAndOrderFront(nil)
      WindowInputFocus.restore(in: window, content: controller.view)
    }
    presentedGeneration = generation
    return true
  }

  @discardableResult
  func resize(generation: Int, height: Double, width: Double = 648) -> Bool {
    guard generation == presentedGeneration else { return false }
    resizeWindow(height, width: width)
    return true
  }

  @discardableResult
  func hide(generation: Int) -> Bool {
    guard generation >= latestGeneration else { return false }
    dismiss(generation: generation)
    return true
  }

  func dismissForFocusLoss() -> Int? {
    guard !settingsOpen, let generation = presentedGeneration else { return nil }
    dismiss(generation: generation)
    return generation
  }

  private func dismiss(generation: Int) {
    latestGeneration = generation + 1
    presentedGeneration = nil
    settingsOpen = false
    willHide()
    hideLauncherWindow()
  }

  private func positionWindow() {
    let pointer = NSEvent.mouseLocation
    let screen =
      NSScreen.screens.first { NSMouseInRect(pointer, $0.frame, false) } ?? NSScreen.main!
    let frame = screen.visibleFrame
    window.setFrameOrigin(
      NSPoint(
        x: frame.midX - window.frame.width / 2,
        y: max(frame.minY, frame.maxY - frame.height * 0.20 - window.frame.height)
      )
    )
  }

  // Settings and Launcher share one engine, but have distinct native window lifecycles.
  // Moving the view avoids keeping an extra Impeller engine just for preferences.
  private func showSettingsWindow(height: Double) {
    window.orderOut(nil)
    if settingsWindow == nil {
      let panel = NSWindow(
        contentRect: NSRect(x: 0, y: 0, width: 648, height: 508),
        styleMask: [.titled, .closable, .miniaturizable, .fullSizeContentView],
        backing: .buffered,
        defer: false
      )
      panel.title = "Anywhere · \(NativeLocalization.text(.settings))"
      panel.titleVisibility = .hidden
      panel.titlebarAppearsTransparent = true
      panel.appearance = NSAppearance(named: .darkAqua)
      panel.backgroundColor = .clear
      panel.isOpaque = false
      panel.isReleasedWhenClosed = false
      panel.delegate = delegate
      panel.standardWindowButton(.zoomButton)?.isEnabled = false
      if let origin = settingsOrigin { panel.setFrameOrigin(origin) } else { panel.center() }
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
    if let panel = settingsWindow {
      WindowInputFocus.restore(in: panel, content: controller.view)
    }
  }

  private func restoreLauncherView() {
    guard let panel = settingsWindow, panel.contentViewController === host else { return }
    settingsOrigin = panel.frame.origin
    panel.orderOut(nil)
    panel.contentViewController = nil
    let frame = window.frame
    window.contentViewController = host
    window.setFrame(frame, display: false)
    glass.maskImage = launcherGlassMask
    glass.cornerRadius = 18
    // Only the shared content needs to survive. Keeping an empty Settings
    // window retains its backing store and lets window restoration raise a
    // blank native window independently of the Launcher presentation.
    panel.close()
    settingsWindow = nil
  }

  private func resizeWindow(_ requestedHeight: Double, width requestedWidth: Double = 648) {
    guard requestedWidth.isFinite, requestedHeight.isFinite else { return }
    if settingsOpen, let panel = settingsWindow {
      let screen = panel.screen ?? NSScreen.main!
      let height = min(max(requestedHeight, 300), screen.visibleFrame.height)
      panel.setContentSize(NSSize(width: 648, height: height))
      WindowInputFocus.restore(in: panel, content: controller.view)
      return
    }
    let screen = window.screen ?? NSScreen.main!
    let height = min(max(requestedHeight, 64), min(600, screen.visibleFrame.height))
    let width = min(max(requestedWidth, 400), screen.visibleFrame.width)
    // Navigation can remove the old editor even without a frame change.
    // Preserve any surviving editor; otherwise restore a keyboard receiver.
    defer { WindowInputFocus.restore(in: window, content: controller.view) }
    guard window.frame.size != NSSize(width: width, height: height) else { return }
    var frame = window.frame
    frame.origin.y = max(screen.visibleFrame.minY, frame.maxY - height)
    frame.origin.x = min(
      max(screen.visibleFrame.minX, frame.midX - width / 2),
      screen.visibleFrame.maxX - width
    )
    frame.size = NSSize(width: width, height: height)
    window.setFrame(frame, display: window.isVisible)
    window.invalidateShadow()
  }

  private func hideLauncherWindow() {
    restoreLauncherView()
    window.orderOut(nil)
    glass.isHidden = true
    // The hidden window need not retain backing surfaces for seven result rows.
    // `show` restores the requested height before ordering it front again.
    resizeWindow(64)
    if NSApp.isActive && !tasksVisible() { NSApp.hide(nil) }
  }

}
