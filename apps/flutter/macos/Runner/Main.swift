import AppKit
import ApplicationServices
import Carbon
import FlutterMacOS
import flutter_inappwebview_macos

@_silgen_name("anywhere_install_flutter_surface_guard")
private func installFlutterSurfaceGuard() -> Int32

// macOS window lifecycle only. Context capture and every action stay in Rust.
final class LauncherWindow: NSWindow {
  override var canBecomeKey: Bool { true }
  override var canBecomeMain: Bool { true }
}

final class AppDelegate: NSObject, NSApplicationDelegate, NSWindowDelegate, NSMenuItemValidation {
  private let accessibilityState = FlutterAccessibilityState.shared
  var window: LauncherWindow!
  var controller: FlutterViewController!
  private var localeHost: LocaleHost?
  private var localeObservers: [NSObjectProtocol] = []
  private var previewHost: NativePreviewHost?
  private var webToolsHost: WebToolsHost!
  private let webPackagePicker = WebPackagePicker()
  var glass: WindowMaterial!
  var materialController: WindowMaterialController!
  var channel: FlutterMethodChannel!
  var statusItem: NSStatusItem!
  lazy var hotKeys = HotKeyService(
    supportedActionIds: [],
    onActivate: { [weak self] mode in self?.activate(mode: mode) },
    onRegistered: { [weak self] settings in self?.updateMenuTitles(settings) },
    log: { [weak self] value in self?.log(value) }
  )
  var activationStart: TimeInterval = 0
  private lazy var settingsHost = SettingsHost(
    hotKeys: hotKeys,
    loginItem: LoginItemService(log: { [weak self] in self?.log($0) })
  )
  var currentAppItem: NSMenuItem?
  var universalItem: NSMenuItem?
  lazy var launcherWindows = LauncherWindowCoordinator(
    window: window,
    controller: controller,
    host: materialController,
    glass: glass,
    mask: launcherGlassMask,
    delegate: self,
    tasksVisible: { [weak self] in
      self?.tasksWindow?.isVisible == true || self?.clipboardWindow.window?.isVisible == true
    },
    willShow: { [weak self] settings in
      self?.hotKeys.setActionShortcutsSuspended(settings)
    },
    willHide: { [weak self] in
      guard let self else { return }
      self.shortcutRecorder.cancel()
      self.hotKeys.setActionShortcutsSuspended(false)
      self.previewHost?.close(restoreFocus: false)
      self.webToolsHost.hideAccessibility()
    }
  )
  var settingsOpen: Bool { launcherWindows.settingsOpen }
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
      if self.webToolsHost?.performEdit(command) == true { return }
      FlutterMethodChannel(
        name: "anywhere/editing",
        binaryMessenger: destination.engine.binaryMessenger
      )
      .invokeMethod("perform", arguments: command)
    }
  )
  private var shortcutRecorder: ShortcutRecordingHost!
  var terminationPending = false
  var stored: StoredSettings { settingsHost.current }

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
    previewHost = NativePreviewHost(
      messenger: controller.engine.binaryMessenger,
      owner: { [weak self] in self?.window }
    )
    webToolsHost = WebToolsHost(
      messenger: controller.engine.binaryMessenger,
      window: window,
      content: controller.view,
      log: { [weak self] in self?.log($0) }
    )
    channel = FlutterMethodChannel(
      name: "anywhere/window",
      binaryMessenger: controller.engine.binaryMessenger
    )
    shortcutRecorder = ShortcutRecordingHost(messenger: controller.engine.binaryMessenger) {
      [weak self] in
      guard let self else { return false }
      return self.settingsOpen && NSApp.isActive && NSApp.keyWindow === self.settingsWindow
    }
    channel.setMethodCallHandler { [weak self] call, reply in
      guard let self else { return }
      if self.settingsHost.handle(call, reply: reply) { return }
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
          "hotkeyError": self.settingsHost.hotKeyError ?? "",
          "loginError": self.settingsHost.loginError ?? "",
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
      case "ready":
        reply(nil)
        self.activate(mode: "currentApp")
      case "show":
        guard let args = call.arguments as? [String: Any],
          let generation = args["generation"] as? Int
        else {
          reply(nil)
          return
        }
        let settings = args["settings"] as? Bool == true
        guard
          self.launcherWindows.show(
            generation: generation,
            settings: settings,
            height: args["height"] as? Double ?? (settings ? 508 : 64),
            width: args["width"] as? Double ?? 648
          )
        else {
          reply(nil)
          return
        }
        if NativePreviewHost.dismissUnownedPanel() {
          self.log(["event": "unowned_preview_dismissed", "stage": "show"])
        }
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
          let height = args["height"] as? Double
        {
          self.launcherWindows.resize(
            generation: generation,
            height: height,
            width: args["width"] as? Double ?? 648
          )
        }
        reply(nil)
      case "hide":
        if let args = call.arguments as? [String: Any],
          let generation = args["generation"] as? Int
        {
          self.launcherWindows.hide(generation: generation)
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
      if self?.webToolsHost?.handleKey(event) == true { return nil }
      guard let self, self.tasksReady,
        let tasks = self.tasksWindow, NSApp.keyWindow === tasks,
        event.keyCode == UInt16(kVK_ANSI_Period),
        event.modifierFlags.intersection([.command, .option, .control, .shift]) == .command
      else { return event }
      self.stopTask()
      return nil
    }
    settingsHost.start()
    // A hidden view never receives viewWillAppear, which normally starts the engine.
    if !controller.engine.run(withEntrypoint: nil) {
      log(["event": "flutter_engine_start_failed"])
      NSApp.terminate(nil)
    } else {
      accessibilityState.synchronizeNewEngine()
    }
  }

  func dismissForFocusLoss() {
    guard !terminationPending, let generation = launcherWindows.dismissForFocusLoss() else {
      return
    }
    log(["event": "launcher_dismissed", "reason": "focus_lost", "generation": generation])
    channel.invokeMethod("dismiss", arguments: ["generation": generation])
  }

  func windowDidResignKey(_ notification: Notification) {
    if notification.object as? NSWindow === settingsWindow { shortcutRecorder.cancel() }
    if notification.object as? NSWindow === window {
      DispatchQueue.main.async { [weak self] in
        guard let self, !self.window.isKeyWindow, self.previewHost?.containsKeyWindow != true else {
          return
        }
        self.dismissForFocusLoss()
      }
    }
  }
  func applicationDidResignActive(_ notification: Notification) {
    shortcutRecorder.cancel()
    dismissForFocusLoss()
  }

  func applicationDidBecomeActive(_ notification: Notification) {
    DispatchQueue.main.async { [weak self] in
      if NativePreviewHost.dismissUnownedPanel() {
        self?.log(["event": "unowned_preview_dismissed", "stage": "activation"])
      }
    }
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
    if ["currentApp", "universal"].contains(mode) {
      if launcherWindows.presentedGeneration == nil {
        previewHost?.close(restoreFocus: false)
      } else if previewHost?.returnToOwner() == true {
        return
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

  func updateMenuTitles(_ settings: StoredSettings) {
    currentAppItem?.title =
      "\(NativeLocalization.text(.currentApp))  \(settings.shortcutLabel(code: settings.currentAppKeyCode, modifiers: settings.currentAppModifiers))"
    universalItem?.title =
      "\(NativeLocalization.text(.universalSearch))  \(settings.shortcutLabel(code: settings.universalKeyCode, modifiers: settings.universalModifiers))"
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
