import AppKit
import FlutterMacOS

/// Owns the task view/engine only; Rust owns the app-lifetime processes.
final class TasksWindowController: NSObject, NSWindowDelegate {
  var tasksWindow: NSWindow?
  var tasksController: FlutterViewController?
  var tasksChannel: FlutterMethodChannel?
  var tasksReady = false
  private var localeHost: LocaleHost?
  var pendingTaskId: String?
  let onLauncher: () -> Void
  init(onLauncher: @escaping () -> Void) { self.onLauncher = onLauncher }
  func showTasks(id: String?) -> Bool {
    pendingTaskId = id
    let created = tasksWindow == nil
    if created {
      let view = FlutterViewController(project: FlutterDartProject())
      localeHost = LocaleHost(messenger: view.engine.binaryMessenger)
      let channel = FlutterMethodChannel(
        name: "anywhere/executions",
        binaryMessenger: view.engine.binaryMessenger
      )
      channel.setMethodCallHandler { [weak self] call, reply in
        guard let self else {
          reply(nil)
          return
        }
        if call.method == "paths" {
          reply(["library": HostRuntime.libraryPath])
        } else if call.method == "capabilities" {
          reply(HostRuntime.capabilities)
        } else if call.method == "ready" {
          self.tasksReady = true
          reply(self.pendingTaskId)
        } else if call.method == "showLauncher" {
          reply(nil)
          self.onLauncher()
        } else if call.method == "hide" {
          reply(nil)
          DispatchQueue.main.async { [weak self] in self?.tasksWindow?.performClose(nil) }
        } else {
          reply(FlutterMethodNotImplemented)
        }
      }
      guard view.engine.run(withEntrypoint: "executionSessionsMain") else {
        channel.setMethodCallHandler(nil)
        localeHost = nil
        return false
      }
      let panel = NSWindow(
        contentRect: NSRect(x: 0, y: 0, width: 920, height: 600),
        styleMask: [.titled, .closable, .miniaturizable, .resizable, .fullSizeContentView],
        backing: .buffered,
        defer: false
      )
      panel.title = "Anywhere · \(NativeLocalization.text(.runningTasks))"
      panel.titleVisibility = .hidden
      panel.titlebarAppearsTransparent = true
      panel.appearance = NSAppearance(named: .darkAqua)
      panel.backgroundColor = .clear
      panel.isOpaque = false
      panel.minSize = NSSize(width: 760, height: 420)
      panel.isReleasedWhenClosed = false
      panel.delegate = self
      view.backgroundColor = .clear
      panel.contentViewController = WindowMaterialController(
        contentController: view,
        frame: NSRect(x: 0, y: 0, width: 920, height: 600),
        cornerRadius: 0
      )
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
      if let panel = tasksWindow {
        WindowInputFocus.restore(in: panel, content: view)
      }
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
    localeHost = nil
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

}
