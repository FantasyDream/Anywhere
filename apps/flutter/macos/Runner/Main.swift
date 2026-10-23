import AppKit
import ApplicationServices
import Carbon
import FlutterMacOS
import ServiceManagement
import flutter_inappwebview_macos

@_silgen_name("anywhere_install_flutter_surface_guard")
private func installFlutterSurfaceGuard() -> Int32

// macOS window lifecycle only. Context capture and every action stay in Rust.
final class LauncherWindow: NSWindow {
  override var canBecomeKey: Bool { true }
  override var canBecomeMain: Bool { true }
}

final class AppDelegate: NSObject, NSApplicationDelegate, NSWindowDelegate, NSMenuItemValidation {
  var window: LauncherWindow!
  var controller: FlutterViewController!
  private var localeHost: LocaleHost?
  private var localeObservers: [NSObjectProtocol] = []
  private var webToolsChannel: FlutterMethodChannel?
  private var webToolsActive = false
  private let webPackagePicker = WebPackagePicker()
  var glass: WindowMaterial!
  var materialController: WindowMaterialController!
  var channel: FlutterMethodChannel!
  var statusItem: NSStatusItem!
  lazy var fileSearchAccess = FileSearchAccess()
  lazy var hotKeys = HotKeyService(
    supportedActionIds: [],
    onActivate: { [weak self] mode in self?.activate(mode: mode) },
    onRegistered: { [weak self] settings in self?.updateMenuTitles(settings) },
    log: { [weak self] value in self?.log(value) }
  )
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
  lazy var launcherWindows = LauncherWindowCoordinator(
    window: window,
    controller: controller,
    host: materialController,
    glass: glass,
    mask: launcherGlassMask,
    delegate: self,
    tasksVisible: { [weak self] in
      self?.tasksWindow?.isVisible == true || self?.clipboardWindow.window?.isVisible == true
    }
  )
  var settingsOpen: Bool {
    get { launcherWindows.settingsOpen }
    set { launcherWindows.settingsOpen = newValue }
  }
  var settingsWindow: NSWindow? { launcherWindows.settingsWindow }
  var launcherGlassMask: NSImage?
  lazy var taskWindows = TasksWindowController(onLauncher: { [weak self] in
    self?.activate(mode: "universal")
  })
  lazy var clipboardWindow = ToolWindowController(
    entrypoint: "clipboardShelfMain",
    channelName: "anywhere/clipboard-shelf",
    title: "Anywhere · \(NativeLocalization.text(.clipboardShelf))",
    onPerformanceEvent: { [weak self] event in self?.log(event) }
  )
  var tasksWindow: NSWindow? { taskWindows.tasksWindow }
  var tasksChannel: FlutterMethodChannel? { taskWindows.tasksChannel }
  var tasksReady: Bool { taskWindows.tasksReady }
  var localKeyMonitor: Any?
  private var editingController: FlutterViewController? {
    guard NSApp.isActive, let key = NSApp.keyWindow else { return nil }
    if key === tasksWindow { return taskWindows.tasksController }
    if key === clipboardWindow.window { return clipboardWindow.controller }
    return key === window || key === settingsWindow ? controller : nil
  }
  private var nativePanelEditing: Bool { NSApp.isActive && NSApp.keyWindow is NSPanel }
  lazy var editMenu = NativeEditMenu(
    available: { [weak self] in
      self?.editingController != nil || self?.nativePanelEditing == true
    },
    send: { [weak self] command in
      guard let self else { return }
      if self.nativePanelEditing {
        _ = NSApp.sendAction(NSSelectorFromString(command + ":"), to: nil, from: nil)
        return
      }
      guard let destination = self.editingController else { return }
      if self.webToolsActive, NSApp.keyWindow === self.window,
        NativeWebEditing.perform(command, in: self.window)
      {
        return
      }
      FlutterMethodChannel(
        name: "anywhere/editing",
        binaryMessenger: destination.engine.binaryMessenger
      )
      .invokeMethod("perform", arguments: command)
    }
  )
  let shortcutRecorder = ShortcutRecorder()
  var shortcutRecordingChannel: FlutterMethodChannel!
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
    log(["event": "flutter_surface_guard", "installed": installFlutterSurfaceGuard() == 1])
    NSApp.setActivationPolicy(.accessory)
    _ = clipboardWindow  // Observe paste destinations before Launcher takes focus; no engine yet.
    LocaleHost.update(stored.uiLanguage)
    controller = FlutterViewController(project: FlutterDartProject())
    localeHost = LocaleHost(messenger: controller.engine.binaryMessenger)
    InAppWebViewFlutterPlugin.register(
      with: controller.registrar(forPlugin: "InAppWebViewFlutterPlugin")
    )
    webToolsChannel = FlutterMethodChannel(
      name: "anywhere/web-tools",
      binaryMessenger: controller.engine.binaryMessenger
    )
    webToolsChannel?.setMethodCallHandler { [weak self] call, reply in
      if call.method == "focusHost", let self, self.webToolsActive,
        NSApp.keyWindow === self.window
      {
        self.window.makeFirstResponder(nil)
        WindowInputFocus.restore(in: self.window, content: self.controller.view)
        reply(nil)
      } else if call.method == "focusWeb", let self, self.webToolsActive,
        NSApp.keyWindow === self.window
      {
        NativeWebEditing.focus(
          in: self.window,
          content: self.window.contentView ?? self.controller.view
        )
        reply(nil)
      } else if call.method == "configureWebAppearance", let self, self.webToolsActive {
        let transparent = NativeWebAppearance.configure(
          in: self.window.contentView ?? self.controller.view
        )
        self.log([
          "event": "web_canvas_configured", "web_view_found": transparent != nil,
          "transparent": transparent ?? false,
        ])
        reply(transparent)
      } else if call.method == "active", let active = call.arguments as? Bool {
        self?.webToolsActive = active
        // Background dragging otherwise steals WebKit mouse-up/click events.
        self?.window.isMovableByWindowBackground = !active
        reply(nil)
      } else {
        reply(FlutterMethodNotImplemented)
      }
    }
    controller.backgroundColor = .clear
    window = LauncherWindow(
      contentRect: NSRect(x: 0, y: 0, width: 648, height: 64),
      styleMask: [.borderless],
      backing: .buffered,
      defer: false
    )
    window.title = "Anywhere"
    window.isExcludedFromWindowsMenu = true
    let windowFrame = window.frame
    materialController = WindowMaterialController(
      contentController: controller,
      frame: NSRect(origin: .zero, size: windowFrame.size),
      cornerRadius: 18
    )
    glass = materialController.material
    window.contentViewController = materialController
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
    log([
      "event": "window_material", "backend": glass.usesLiquidGlass ? "liquidGlass" : "visualEffect",
    ])
    channel = FlutterMethodChannel(
      name: "anywhere/window",
      binaryMessenger: controller.engine.binaryMessenger
    )
    shortcutRecordingChannel = FlutterMethodChannel(
      name: "anywhere/shortcut-recorder",
      binaryMessenger: controller.engine.binaryMessenger
    )
    shortcutRecordingChannel.setMethodCallHandler { [weak self] call, reply in
      guard let self, let args = call.arguments as? [String: Any],
        let id = args["id"] as? Int
      else {
        reply(nil)
        return
      }
      switch call.method {
      case "record":
        guard self.settingsOpen, NSApp.isActive, NSApp.keyWindow === self.settingsWindow else {
          reply(
            FlutterError(
              code: "recording_unavailable",
              message: "Open Settings to record a shortcut.",
              details: nil
            )
          )
          return
        }
        self.shortcutRecorder.start(id: id) { reply($0) }
      case "cancel":
        self.shortcutRecorder.cancel(id: id)
        reply(nil)
      default: reply(FlutterMethodNotImplemented)
      }
    }
    channel.setMethodCallHandler { [weak self] call, reply in
      guard let self else { return }
      switch call.method {
      case "chooseWebPackage":
        guard self.settingsOpen, let settings = self.settingsWindow,
          NSApp.keyWindow === settings
        else {
          reply(nil)
          return
        }
        self.webPackagePicker.choose(in: settings) { reply($0) }
      case "capabilities":
        reply(HostRuntime.capabilities)
      case "paths":
        let data = Self.supportDirectory()
        reply([
          "library": HostRuntime.libraryPath, "data": data.path,
          "plugins": Bundle.main.resourceURL!.appendingPathComponent("plugins", isDirectory: true)
            .path,
          "hotkeyError": self.hotKeyError ?? "",
          "loginError": self.loginError ?? "",
          "excludedApps": self.stored.excludedApps.joined(separator: "\n"),
          "captureFinderSelection": self.stored.captureFinderSelection == false ? "0" : "1",
          "captureSelectedText": self.stored.captureSelectedText == false ? "0" : "1",
          "captureBrowserCurrentPage": self.stored.captureBrowserCurrentPage == false ? "0" : "1",
          "captureClipboard": self.stored.captureClipboard == false ? "0" : "1",
          "profileUi": PresentationDiagnostics.enabled ? "1" : "0",
        ])
      case "performanceSample":
        if let event = PresentationDiagnostics.event(
          call.arguments,
          visible: self.window.isVisible
        ) {
          self.log(event)
        }
        reply(nil)
      case "settings":
        if let enabled = self.loginItemIsEnabled() {
          self.stored.launchAtLogin = enabled
        }
        var snapshot = self.stored.asMap()
        snapshot["fileSearchAccessReady"] = self.fileSearchAccess.ready
        reply(snapshot)
      case "syncActionShortcutCatalog":
        guard let args = call.arguments as? [String: Any],
          let ids = args["supportedActionIds"] as? [String],
          ids.count <= RuntimePolicy.maxShortcutCatalogRequestIds,
          ids.allSatisfy({ !$0.isEmpty && $0.utf8.count <= RuntimePolicy.maxActionIdBytes }),
          let snapshot = args["settings"] as? [String: Any]
        else {
          reply(FlutterError(code: "shortcuts", message: "Invalid shortcut catalog.", details: nil))
          return
        }
        var expected = self.stored
        guard expected.apply(snapshot) == nil,
          expected.sameActionShortcutConfiguration(as: self.stored)
        else {
          reply(
            FlutterError(
              code: "shortcuts",
              message: "Shortcut configuration changed. Refresh settings and try again.",
              details: nil
            )
          )
          return
        }
        if let error = self.hotKeys.syncActionCatalog(Set(ids)) {
          reply(FlutterError(code: "shortcuts", message: error, details: nil))
        } else {
          reply(nil)
        }
      case "prepareFileSearchAccess":
        guard let args = call.arguments as? [String: Any],
          let enabled = args["enabled"] as? Bool,
          let roots = args["roots"] as? [String], roots.count <= RuntimePolicy.maxSearchRoots,
          roots.allSatisfy({
            $0.hasPrefix("/") && $0.utf8.count <= RuntimePolicy.maxSearchPathBytes
          })
        else {
          reply(FlutterError(code: "file_access", message: "Invalid search scope.", details: nil))
          return
        }
        self.fileSearchAccess.prepare(
          enabled: enabled,
          roots: roots,
          force: args["force"] as? Bool == true
        ) { reply($0) }
      case "settingsStatus":
        reply([
          "hotkeyError": self.hotKeyError ?? "",
          "loginError": self.loginError ?? "",
          "fileAccessError": self.fileSearchAccess.status,
        ])
      case "resumeActionShortcuts":
        if let error = self.hotKeys.resumeForSettingsClose() {
          reply(FlutterError(code: "shortcuts", message: error, details: nil))
        } else {
          reply(nil)
        }
      case "openPrivacySettings":
        guard let args = call.arguments as? [String: Any],
          let pane = args["pane"] as? String,
          ["accessibility", "automation", "clipboard", "fullDiskAccess"].contains(pane)
        else {
          reply(FlutterError(code: "privacy", message: "Unknown privacy setting.", details: nil))
          return
        }
        let app = NSWorkspace.shared.urlForApplication(
          withBundleIdentifier: "com.apple.systempreferences"
        )
        let anchor =
          pane == "accessibility"
          ? "Privacy_Accessibility"
          : pane == "fullDiskAccess" ? "Privacy_AllFiles" : "Privacy_Automation"
        // Clipboard preferences vary by macOS version. Open System Settings
        // itself rather than guessing an unsupported pane identifier.
        let link =
          pane == "clipboard"
          ? app : URL(string: "x-apple.systempreferences:com.apple.preference.security?\(anchor)")
        if let link, NSWorkspace.shared.open(link) {
          reply(nil)
        } else if let app, NSWorkspace.shared.open(app) {
          reply(nil)
        } else {
          reply(
            FlutterError(
              code: "privacy",
              message: "System Settings could not be opened.",
              details: nil
            )
          )
        }
      case "showToolWindow":
        let args = call.arguments as? [String: Any]
        guard args?["id"] as? String == "clipboard.shelf" else {
          reply(FlutterError(code: "tool", message: "Unknown tool window.", details: nil))
          return
        }
        if self.clipboardWindow.show() {
          reply(nil)
        } else {
          reply(
            FlutterError(code: "tool", message: "Clipboard Shelf could not start.", details: nil)
          )
        }
      case "showExecutionSessions":
        let args = call.arguments as? [String: Any]
        if self.showTasks(id: args?["id"] as? String) {
          reply(nil)
        } else {
          reply(
            FlutterError(code: "tasks", message: "Running Tasks could not start.", details: nil)
          )
        }
      case "saveSettings":
        guard let args = call.arguments as? [String: Any] else {
          reply(nil)
          return
        }
        let previous = self.stored
        var next = previous
        if let error = next.apply(args) {
          reply(FlutterError(code: "settings", message: error, details: nil))
          return
        }
        var errors: [String] = []
        var support: Set<String>? = nil
        if let raw = args["supportedActionIds"] {
          guard let ids = raw as? [String], ids.count <= RuntimePolicy.maxShortcutCatalogRequestIds,
            ids.allSatisfy({ !$0.isEmpty && $0.utf8.count <= RuntimePolicy.maxActionIdBytes })
          else {
            reply(
              FlutterError(code: "settings", message: "Invalid shortcut catalog.", details: nil)
            )
            return
          }
          support = Set(ids)
        }
        let update = self.hotKeys.update(next, previous: previous, supportedActionIds: support)
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
          self.stored.uiLanguage = previous.uiLanguage
          errors.append(error)
        } else {
          LocaleHost.update(next.uiLanguage)
        }
        if errors.isEmpty {
          reply(nil)
        } else {
          reply(
            FlutterError(code: "settings", message: errors.joined(separator: "\n"), details: nil)
          )
        }
      case "ready":
        reply(nil)
        self.activate(mode: "currentApp")
      case "show":
        guard let args = call.arguments as? [String: Any],
          let generation = args["generation"] as? Int,
          generation >= self.latestGeneration
        else {
          reply(nil)
          return
        }
        self.latestGeneration = generation
        self.settingsOpen = args["settings"] as? Bool == true
        self.hotKeys.setActionShortcutsSuspended(self.settingsOpen)
        self.glass.isHidden = false
        NSApp.unhide(nil)
        NSApp.activate(ignoringOtherApps: true)
        if self.settingsOpen {
          self.showSettingsWindow(height: args["height"] as? Double ?? 508)
        } else {
          self.restoreLauncherView()
          self.resizeWindow(args["height"] as? Double ?? 64, width: args["width"] as? Double ?? 648)
          self.positionWindow()
          self.window.makeKeyAndOrderFront(nil)
          WindowInputFocus.restore(in: self.window, content: self.controller.view)
        }
        self.presentedGeneration = generation
        let elapsed = (ProcessInfo.processInfo.systemUptime - self.activationStart) * 1000
        let visibleFrame =
          (self.settingsOpen ? self.settingsWindow?.frame : self.window.frame) ?? self.window.frame
        var visibility: [String: Any] = [
          "event": self.settingsOpen ? "settings_visible" : "launcher_visible",
          "window_width": visibleFrame.width,
          "window_height": visibleFrame.height,
          "ax_trusted": AXIsProcessTrusted(),
        ]
        if !self.settingsOpen { visibility["hotkey_to_visible_ms"] = elapsed }
        self.log(visibility)
        reply(nil)
      case "resize":
        if let args = call.arguments as? [String: Any],
          let generation = args["generation"] as? Int,
          generation == self.presentedGeneration,
          let height = args["height"] as? Double
        {
          self.resizeWindow(height, width: args["width"] as? Double ?? 648)
        }
        reply(nil)
      case "hide":
        if let args = call.arguments as? [String: Any],
          let generation = args["generation"] as? Int,
          generation >= self.latestGeneration
        {
          self.latestGeneration = generation + 1
          self.presentedGeneration = nil
          self.settingsOpen = false
          self.shortcutRecorder.cancel()
          self.hotKeys.setActionShortcutsSuspended(false)
          self.hideLauncherWindow()
        }
        reply(nil)
      default: reply(FlutterMethodNotImplemented)
      }
    }
    installMenu()
    for name in [LocaleHost.changed, NSLocale.currentLocaleDidChangeNotification] {
      localeObservers.append(
        NotificationCenter.default.addObserver(
          forName: name,
          object: nil,
          queue: .main
        ) { [weak self] _ in
          guard let self else { return }
          self.installMenu()
          self.updateMenuTitles(self.stored)
          self.settingsWindow?.title = "Anywhere · \(NativeLocalization.text(.settings))"
          self.tasksWindow?.title = "Anywhere · \(NativeLocalization.text(.runningTasks))"
        }
      )
    }
    // AppKit can route ⌘. as cancelOperation before key-equivalent menus.
    // Its characters may already be Escape, so match the period key code.
    // Shortcut recording consumes Settings keys only while explicitly requested.
    // Outside recording, only the task-window stop shortcut is consumed.
    localKeyMonitor = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { [weak self] event in
      if let self, self.shortcutRecorder.isRecording, self.settingsOpen,
        NSApp.isActive, let settings = self.settingsWindow,
        event.window === settings, NSApp.keyWindow === settings,
        self.shortcutRecorder.handle(
          code: event.keyCode,
          flags: event.modifierFlags,
          repeatKey: event.isARepeat
        )
      {
        return nil
      }
      if let self, self.webToolsActive, NSApp.isActive,
        NSApp.keyWindow === self.window, event.window === self.window
      {
        let flags = event.modifierFlags.intersection([.command, .option, .control, .shift])
        let composing = (self.window.firstResponder as? NSTextInputClient)?.hasMarkedText() ?? false
        if !composing, event.keyCode == 53 {
          self.webToolsChannel?.invokeMethod("back", arguments: nil)
          return nil
        }
        if !composing, flags == .command, event.keyCode == UInt16(kVK_ANSI_K) {
          self.webToolsChannel?.invokeMethod("actions", arguments: nil)
          return nil
        }
      }
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
  func resizeWindow(_ height: Double, width: Double = 648) {
    launcherWindows.resizeWindow(height, width: width)
  }
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
    if notification.object as? NSWindow === settingsWindow { shortcutRecorder.cancel() }
    if notification.object as? NSWindow === window { dismissForFocusLoss() }
  }
  func applicationDidResignActive(_ notification: Notification) {
    shortcutRecorder.cancel()
    dismissForFocusLoss()
  }

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
    if item.action == #selector(closeActiveWindow) || item.action == #selector(minimizeActiveWindow)
    {
      guard let key = NSApp.keyWindow else { return false }
      return key === settingsWindow || key === tasksWindow || key === clipboardWindow.window
    }
    if item.action == #selector(zoomActiveWindow) {
      guard let key = NSApp.keyWindow else { return false }
      return key === tasksWindow || key === clipboardWindow.window
    }
    let taskActions: [Selector] = [
      #selector(findTask), #selector(stopTask),
      #selector(refreshTasks), #selector(taskLauncher), #selector(hideTasks),
    ]
    if let action = item.action, taskActions.contains(action) {
      return tasksWindow?.isKeyWindow == true && tasksReady
    }
    return true
  }

  /// Lazy second Flutter engine: releasing its window never destroys Rust process sessions.
  func showTasks(id: String?) -> Bool { taskWindows.showTasks(id: id) }
  func applicationDidHide(_ notification: Notification) {
    taskWindows.applicationDidHide(notification)
    clipboardWindow.applicationVisibilityChanged()
  }
  func applicationDidUnhide(_ notification: Notification) {
    taskWindows.applicationDidUnhide(notification)
    clipboardWindow.applicationVisibilityChanged()
  }

  func activate(mode: String) {
    guard !terminationPending else { return }
    if settingsOpen, shortcutRecorder.isRecording {
      switch mode {
      case "currentApp":
        _ = shortcutRecorder.accept(
          code: stored.currentAppKeyCode,
          modifiers: stored.currentAppModifiers
        )
        return
      case "universal":
        _ = shortcutRecorder.accept(
          code: stored.universalKeyCode,
          modifiers: stored.universalModifiers
        )
        return
      default: break
      }
    }
    activationStart = ProcessInfo.processInfo.systemUptime
    log(["event": "activation_requested", "mode": mode])
    if mode.hasPrefix("action:") {
      guard !settingsOpen, !terminationPending else { return }
      // Do not foreground Anywhere here: Rust must capture the real source app first.
      let id = String(mode.dropFirst(7))
      guard
        let binding = stored.actionShortcuts?.first(where: {
          $0.actionId == id && $0.global == true
        })
      else { return }
      channel.invokeMethod(
        "actionShortcut",
        arguments: ["actionId": id, "binding": binding.asMap()]
      )
    } else {
      channel.invokeMethod("toggle", arguments: ["mode": mode])
    }
  }

  func applyHotKeys(_ settings: StoredSettings) -> String? { hotKeys.apply(settings) }

  func updateMenuTitles(_ settings: StoredSettings) {
    currentAppItem?.title =
      "\(NativeLocalization.text(.currentApp))  \(settings.shortcutLabel(code: settings.currentAppKeyCode, modifiers: settings.currentAppModifiers))"
    universalItem?.title =
      "\(NativeLocalization.text(.universalSearch))  \(settings.shortcutLabel(code: settings.universalKeyCode, modifiers: settings.universalModifiers))"
  }

  func applyLoginItem() -> String? {
    if ProcessInfo.processInfo.environment["ANYWHERE_DATA_DIRECTORY"] != nil { return nil }
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
    let script =
      want
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
    if ProcessInfo.processInfo.environment["ANYWHERE_DATA_DIRECTORY"] != nil {
      return stored.launchAtLogin
    }
    if #available(macOS 13.0, *) {
      return SMAppService.mainApp.status == .enabled
    }
    return legacyLoginEnabled()
  }

  func legacyLoginEnabled() -> Bool? {
    let path = Bundle.main.bundlePath
    let output = runOsascriptOutput(
      """
      tell application "System Events"
          if (count of login items) is 0 then return ""
          set AppleScript's text item delimiters to linefeed
          return (path of every login item) as text
      end tell
      """
    )
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
    return String(data: data, encoding: .utf8)?.trimmingCharacters(in: .whitespacesAndNewlines)
      ?? ""
  }

  func installMenu() {
    if let statusItem { NSStatusBar.system.removeStatusItem(statusItem) }
    statusItem = NSStatusBar.system.statusItem(withLength: NSStatusItem.squareLength)
    statusItem.button?.image = NSImage(
      systemSymbolName: "magnifyingglass",
      accessibilityDescription: "Anywhere"
    )?
    .withSymbolConfiguration(NSImage.SymbolConfiguration(pointSize: 15, weight: .semibold))
    let menu = NSMenu()
    let current = NSMenuItem(
      title: NativeLocalization.text(.currentApp),
      action: #selector(activateCurrentApp),
      keyEquivalent: ""
    )
    current.target = self
    currentAppItem = current
    menu.addItem(current)
    let universal = NSMenuItem(
      title: NativeLocalization.text(.universalSearch),
      action: #selector(activateUniversal),
      keyEquivalent: ""
    )
    universal.target = self
    universalItem = universal
    menu.addItem(universal)
    let tasks = NSMenuItem(
      title: NativeLocalization.text(.runningTasksMenu),
      action: #selector(openTasks),
      keyEquivalent: ""
    )
    tasks.target = self
    menu.addItem(tasks)
    menu.addItem(.separator())
    let settingsItem = NSMenuItem(
      title: NativeLocalization.text(.settingsMenu),
      action: #selector(openSettings),
      keyEquivalent: ","
    )
    settingsItem.target = self
    menu.addItem(settingsItem)
    menu.addItem(.separator())
    let quit = NSMenuItem(
      title: NativeLocalization.text(.quitAnywhere),
      action: #selector(NSApplication.terminate(_:)),
      keyEquivalent: "q"
    )
    quit.target = NSApp
    menu.addItem(quit)
    statusItem.menu = menu

    // Native menu commands are applied to Flutter's focused editing model.
    let main = NSMenu()
    let appItem = NSMenuItem()
    main.addItem(appItem)
    appItem.submenu = menu.copy() as? NSMenu
    let editItem = NSMenuItem(title: NativeLocalization.text(.edit), action: nil, keyEquivalent: "")
    editItem.submenu = editMenu.makeMenu(
      title: NativeLocalization.text(.edit),
      labels: [NativeText.undo, .redo, .cut, .copy, .paste, .selectAll].map {
        NativeLocalization.text($0)
      }
    )
    main.addItem(editItem)
    // Expose task commands through macOS menus. The local monitor reserves ⌘.;
    // Flutter owns Escape's clear-filter / hide logic.
    let taskItem = NSMenuItem(
      title: NativeLocalization.text(.tasks),
      action: nil,
      keyEquivalent: ""
    )
    let taskMenu = NSMenu(title: NativeLocalization.text(.tasks))
    for (title, action, key) in [
      (NativeLocalization.text(.findTaskMenu), #selector(findTask), "f"),
      (NativeLocalization.text(.stopSelectedTask), #selector(stopTask), "."),
      (NativeLocalization.text(.refreshTasks), #selector(refreshTasks), "r"),
      (NativeLocalization.text(.openLauncher), #selector(taskLauncher), "l"),
      (NativeLocalization.text(.closeTaskWindow), #selector(hideTasks), ""),
    ] {
      let item = NSMenuItem(title: title, action: action, keyEquivalent: key)
      item.target = self
      taskMenu.addItem(item)
    }
    taskItem.submenu = taskMenu
    main.addItem(taskItem)
    let windowItem = NSMenuItem(
      title: NativeLocalization.text(.window),
      action: nil,
      keyEquivalent: ""
    )
    let windowMenu = NSMenu(title: NativeLocalization.text(.window))
    for (title, action, key) in [
      (NativeLocalization.text(.closeWindow), #selector(closeActiveWindow), "w"),
      (NativeLocalization.text(.minimize), #selector(minimizeActiveWindow), "m"),
      (NativeLocalization.text(.zoom), #selector(zoomActiveWindow), ""),
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
      let line = String(data: data, encoding: .utf8)
    {
      FileHandle.standardError.write(Data((line + "\n").utf8))
    }
  }

  func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool { false }
  func applicationShouldHandleReopen(_ sender: NSApplication, hasVisibleWindows flag: Bool) -> Bool
  {
    if !flag {
      if settingsOpen { openSettings() } else { activate(mode: "currentApp") }
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
    shortcutRecorder.cancel()
    for observer in localeObservers { NotificationCenter.default.removeObserver(observer) }
    localeObservers.removeAll()
    if let localKeyMonitor { NSEvent.removeMonitor(localKeyMonitor) }
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
