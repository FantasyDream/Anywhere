import AppKit
import FlutterMacOS
import QuickLookUI
import os

/// One native preview panel for the process; each engine supplies its own owner and events.
final class NativePreviewHost: NSObject, QLPreviewPanelDataSource, QLPreviewPanelDelegate {
  private static weak var current: NativePreviewHost?
  private static let log = Logger(subsystem: "dev.anywhere", category: "preview")
  private let channel: FlutterMethodChannel
  private let owner: () -> NSWindow?
  private var resourceID: String?
  private final class Item: NSObject, QLPreviewItem {
    let previewItemURL: URL?
    let previewItemTitle: String?
    init(path: String, title: String) {
      previewItemURL = URL(fileURLWithPath: path)
      previewItemTitle = title.isEmpty ? previewItemURL?.lastPathComponent : title
    }
  }
  private var items: [Item] = []
  private var keyMonitor: Any?
  private var generation = 0
  // Quick Look may retain its last controller while hidden. Keep that retained
  // responder independent of the host, its channel and the Flutter engine.
  private final class PanelController: NSResponder {
    weak var host: NativePreviewHost?
    override func acceptsPreviewPanelControl(_ panel: QLPreviewPanel!) -> Bool {
      host?.ownsPanel == true
    }
    override func beginPreviewPanelControl(_ panel: QLPreviewPanel!) {
      panel.dataSource = host
      panel.delegate = host
    }
    override func endPreviewPanelControl(_ panel: QLPreviewPanel!) {
      panel.dataSource = nil
      panel.delegate = nil
    }
  }
  private let panelController = PanelController()
  private weak var attachedWindow: NSWindow?
  private var previousResponder: NSResponder?
  private weak var previousInput: NSResponder?
  private var closeObserver: NSObjectProtocol?
  private var resignObserver: NSObjectProtocol?
  private var keyObserver: NSObjectProtocol?

  init(messenger: FlutterBinaryMessenger, owner: @escaping () -> NSWindow?) {
    self.owner = owner
    channel = FlutterMethodChannel(name: "anywhere/preview", binaryMessenger: messenger)
    super.init()
    panelController.host = self
    channel.setMethodCallHandler { [weak self] call, reply in
      guard let self else {
        reply(nil)
        return
      }
      let args = call.arguments as? [String: Any] ?? [:]
      switch call.method {
      case "capabilities":
        reply(["supported": true])
      case "show":
        guard let sequence = args["generation"] as? Int, sequence > self.generation,
          let id = args["id"] as? String,
          let paths = args["paths"] as? [String], !paths.isEmpty, paths.count <= 256,
          paths.allSatisfy({ $0.hasPrefix("/") }),
          let window = self.owner(), window.isVisible, NSApp.isActive
        else {
          reply(
            FlutterError(code: "preview_expired", message: "Preview is unavailable.", details: nil)
          )
          return
        }
        self.generation = sequence
        if self.show(id: id, paths: paths, title: args["title"] as? String ?? "", window: window) {
          reply(nil)
        } else {
          self.close()
          reply(
            FlutterError(
              code: "preview_unavailable",
              message: "Preview is unavailable.",
              details: nil
            )
          )
        }
      case "close":
        if let sequence = args["generation"] as? Int, sequence > self.generation {
          self.generation = sequence
          self.close()
        }
        reply(nil)
      default: reply(FlutterMethodNotImplemented)
      }
    }
    closeObserver = NotificationCenter.default.addObserver(
      forName: NSWindow.willCloseNotification,
      object: nil,
      queue: .main
    ) { [weak self] notification in
      guard let self, self.ownsPanel,
        notification.object as? NSWindow === QLPreviewPanel.shared()
      else { return }
      self.finish()
    }
    keyMonitor = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { [weak self] event in
      guard let self, self.containsKeyWindow, event.window === QLPreviewPanel.shared() else {
        return event
      }
      if let client = NSApp.keyWindow?.firstResponder as? NSTextInputClient, client.hasMarkedText()
      {
        return event
      }
      if let editor = NSApp.keyWindow?.firstResponder as? NSTextView,
        editor.isEditable && !event.modifierFlags.contains(.command)
      {
        return event
      }
      let command = event.modifierFlags.contains(.command)
      if [49, 53, 36, 76].contains(event.keyCode) || (command && [37, 16].contains(event.keyCode)) {
        return self.route(event) ? nil : event
      }
      return event
    }
    resignObserver = NotificationCenter.default.addObserver(
      forName: NSApplication.didResignActiveNotification,
      object: nil,
      queue: .main
    ) { [weak self] _ in self?.close() }
    keyObserver = NotificationCenter.default.addObserver(
      forName: NSWindow.didBecomeKeyNotification,
      object: nil,
      queue: .main
    ) { [weak self] notification in
      guard let self, self.ownsPanel, let window = notification.object as? NSWindow,
        window !== self.attachedWindow, !self.contains(window)
      else { return }
      self.close()
    }
  }
  deinit {
    channel.setMethodCallHandler(nil)
    if let keyMonitor { NSEvent.removeMonitor(keyMonitor) }
    if let closeObserver { NotificationCenter.default.removeObserver(closeObserver) }
    if let resignObserver { NotificationCenter.default.removeObserver(resignObserver) }
    if let keyObserver { NotificationCenter.default.removeObserver(keyObserver) }
  }
  var ownsPanel: Bool { Self.current === self && resourceID != nil }
  var containsKeyWindow: Bool {
    contains(NSApp.keyWindow)
  }
  private func contains(_ window: NSWindow?) -> Bool {
    guard ownsPanel, QLPreviewPanel.sharedPreviewPanelExists() else { return false }
    let panel = QLPreviewPanel.shared()
    var current = window
    while let candidate = current {
      if candidate === panel { return true }
      current = candidate.sheetParent ?? candidate.parent
    }
    return false
  }
  private func show(id: String, paths: [String], title: String, window: NSWindow) -> Bool {
    if Self.current !== self { Self.current?.close() }
    if attachedWindow !== window {
      detach()
      previousResponder = window.nextResponder
      panelController.nextResponder = previousResponder
      window.nextResponder = panelController
      attachedWindow = window
    }
    Self.current = self
    resourceID = id
    items = paths.map { Item(path: $0, title: paths.count == 1 ? title : "") }
    guard let panel = QLPreviewPanel.shared() else { return false }
    if let input = window.firstResponder,
      input.responds(to: #selector(NSObject.acceptsPreviewPanelControl(_:))),
      input.acceptsPreviewPanelControl(panel), let content = window.contentView
    {
      previousInput = input
      guard WindowInputFocus.focusKeyboardReceiver(in: window, content: content) else {
        return false
      }
    }
    // Quick Look acquires its responder-chain controller when the panel is
    // presented. Asking for the controller before presentation can return nil.
    panel.orderFront(nil)
    panel.updateController()
    if panel.currentController as AnyObject? === panelController {
      // A hidden shared panel can keep the controller but discard its data
      // source. Restore both even when beginPreviewPanelControl is not repeated.
      panel.dataSource = self
      panel.delegate = self
      panel.reloadData()
      panel.currentPreviewItemIndex = 0
      // Keep search/list keyboard navigation in the owning window by default.
      panel.level = .floating
      channel.invokeMethod("shown", arguments: ["id": id])
      return true
    }
    Self.log.error(
      "preview_control_unavailable key=\(window.isKeyWindow) main=\(window.isMainWindow)"
    )
    return false
  }
  func close() {
    guard ownsPanel else { return }
    if QLPreviewPanel.sharedPreviewPanelExists(), let panel = QLPreviewPanel.shared() {
      if panel.isInFullScreenMode { panel.exitFullScreenMode(options: nil) }
      // QLPreviewPanel.orderOut/close animate asynchronously: their late close
      // can dismiss a replacement shown in the meantime. Hide synchronously.
      panel.order(.out, relativeTo: 0)
    }
    finish()
  }
  private func finish() {
    guard let id = resourceID else { return }
    let window = attachedWindow
    let restoreFocus = NSApp.isActive && (containsKeyWindow || window?.isKeyWindow == true)
    resourceID = nil
    if Self.current === self {
      if let panel = QLPreviewPanel.shared(),
        panel.currentController as AnyObject? === panelController
      {
        panel.dataSource = nil
        panel.delegate = nil
      }
      Self.current = nil
    }
    items.removeAll()
    detach()
    // Refresh when possible; the retained controller has only a weak host link.
    if QLPreviewPanel.sharedPreviewPanelExists() { QLPreviewPanel.shared()?.updateController() }
    if restoreFocus, let window {
      window.makeKeyAndOrderFront(nil)
      if let input = previousInput as? NSView, input.window === window {
        window.makeFirstResponder(input)
      } else if let content = window.contentView {
        WindowInputFocus.restore(in: window, content: content)
      }
    }
    previousInput = nil
    channel.invokeMethod("closed", arguments: ["id": id, "restoreFocus": restoreFocus])
  }
  private func detach() {
    if let window = attachedWindow, window.nextResponder === panelController {
      window.nextResponder = previousResponder
    }
    attachedWindow = nil
    previousResponder = nil
    panelController.nextResponder = nil
  }
  func numberOfPreviewItems(in panel: QLPreviewPanel!) -> Int { items.count }
  func previewPanel(_ panel: QLPreviewPanel!, previewItemAt index: Int) -> QLPreviewItem! {
    guard items.indices.contains(index) else { return nil }
    return items[index]
  }
  func previewPanel(_ panel: QLPreviewPanel!, handle event: NSEvent!) -> Bool {
    route(event)
  }
  private func route(_ event: NSEvent) -> Bool {
    guard ownsPanel, event.type == .keyDown, let id = resourceID else { return false }
    let command = event.modifierFlags.contains(.command)
    let action: String
    switch event.keyCode {
    case 53, 49: action = "close"
    case 125: action = "next"
    case 126: action = "previous"
    case 36, 76: action = "activate"
    case 37 where command: action = "search"
    case 16 where command: action = "close"
    default: return false
    }
    if action == "close" {
      close()
    } else {
      channel.invokeMethod(action, arguments: ["id": id])
    }
    return true
  }
}
