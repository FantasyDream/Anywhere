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
  private var visibilityObservation: NSKeyValueObservation?
  private weak var closeButton: NSButton?
  private weak var previousCloseTarget: AnyObject?
  private var previousCloseAction: Selector?
  private var closingPanel = false
  private var presentingPanel = false

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
          self.close(restoreFocus: false)
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
    ) { [weak self] _ in self?.close(restoreFocus: false) }
    keyObserver = NotificationCenter.default.addObserver(
      forName: NSWindow.didBecomeKeyNotification,
      object: nil,
      queue: .main
    ) { [weak self] notification in
      guard let self, self.ownsPanel, let window = notification.object as? NSWindow,
        window !== self.attachedWindow, !self.contains(window)
      else { return }
      self.close(restoreFocus: false)
    }
  }
  deinit {
    restoreCloseButton()
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
    presentingPanel = true
    defer { presentingPanel = false }
    if Self.current !== self { Self.current?.close(restoreFocus: false) }
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
    // The Host closes on deactivation. AppKit must not independently hide and
    // later restore a panel whose resource lease has already ended.
    panel.hidesOnDeactivate = false
    if visibilityObservation == nil {
      visibilityObservation = panel.observe(\.isVisible, options: [.new]) {
        [weak self] panel, change in
        if change.newValue == true, Self.current == nil {
          panel.order(.out, relativeTo: 0)
        } else if change.newValue == false {
          guard let self, self.ownsPanel, !self.closingPanel else { return }
          let expectedID = self.resourceID
          let expectedGeneration = self.generation
          // Recheck after controller transitions; an old hide must not close a
          // replacement, and native order-out does not necessarily send willClose.
          DispatchQueue.main.async { [weak self, weak panel] in
            guard let self, let panel, self.ownsPanel, !self.closingPanel,
              !self.presentingPanel, !panel.isVisible, self.resourceID == expectedID,
              self.generation == expectedGeneration
            else { return }
            self.finish()
          }
        }
      }
    }
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
      bindCloseButton(in: panel)
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
  /// A wake key clears stale preview state before normal activation when hidden.
  func returnToOwner() -> Bool {
    guard ownsPanel || (Self.current == nil && Self.visiblePanel != nil) else { return false }
    guard let window = attachedWindow ?? owner(), window.isVisible else {
      close(restoreFocus: false)
      return false
    }
    NSApp.activate(ignoringOtherApps: true)
    window.makeKeyAndOrderFront(nil)
    close()
    return true
  }

  private static var visiblePanel: QLPreviewPanel? {
    guard QLPreviewPanel.sharedPreviewPanelExists(), let panel = QLPreviewPanel.shared(),
      panel.isVisible
    else { return nil }
    return panel
  }

  @discardableResult static func dismissUnownedPanel() -> Bool {
    guard current == nil, QLPreviewPanel.sharedPreviewPanelExists(),
      let panel = QLPreviewPanel.shared()
    else { return false }
    // AppKit can restore an ordered panel when the application unhides, even
    // after its lease ended while the app was hidden.
    let wasVisible = panel.isVisible
    panel.order(.out, relativeTo: 0)
    panel.dataSource = nil
    panel.delegate = nil
    return wasVisible
  }

  private func bindCloseButton(in panel: QLPreviewPanel) {
    guard let button = panel.standardWindowButton(.closeButton), button.target !== self else {
      return
    }
    restoreCloseButton()
    closeButton = button
    previousCloseTarget = button.target
    previousCloseAction = button.action
    // Quick Look's button bypasses windowShouldClose and animates asynchronously.
    // Use the same synchronous close as Escape before another preview can open.
    button.target = self
    button.action = #selector(closeFromButton(_:))
  }

  @objc private func closeFromButton(_ sender: NSButton) {
    guard sender === closeButton else { return }
    close()
  }

  private func restoreCloseButton() {
    if let button = closeButton, button.action == #selector(closeFromButton(_:)),
      button.target == nil || button.target === self
    {
      button.target = previousCloseTarget
      button.action = previousCloseAction
    }
    closeButton = nil
    previousCloseTarget = nil
    previousCloseAction = nil
  }

  func close(restoreFocus: Bool = true) {
    guard !closingPanel else { return }
    closingPanel = true
    defer { closingPanel = false }
    // A renderer can leave the shared panel visible after its document has
    // detached. Clean it only when no other engine owns a newer preview.
    guard ownsPanel else {
      Self.dismissUnownedPanel()
      return
    }
    if QLPreviewPanel.sharedPreviewPanelExists(), let panel = QLPreviewPanel.shared() {
      if panel.isInFullScreenMode { panel.exitFullScreenMode(options: nil) }
      // Hide synchronously so a late close cannot dismiss a replacement.
      panel.order(.out, relativeTo: 0)
    }
    finish(restoreFocus: restoreFocus)
  }
  private func finish(restoreFocus: Bool = true) {
    guard let id = resourceID else { return }
    let window = attachedWindow
    let restoreFocus =
      restoreFocus && NSApp.isActive
      && (containsKeyWindow || window?.isKeyWindow == true)
    resourceID = nil
    items.removeAll()
    if Self.current === self {
      if let panel = QLPreviewPanel.shared(),
        panel.currentController as AnyObject? === panelController
      {
        panel.dataSource = nil
        panel.delegate = nil
      }
      Self.current = nil
    }
    detach()
    if let window, window.isVisible {
      if QLPreviewPanel.sharedPreviewPanelExists() { QLPreviewPanel.shared()?.updateController() }
      if restoreFocus { window.makeKeyAndOrderFront(nil) }
      // Hidden owners restore their input through the normal wake path. Giving
      // their editor Quick Look control here can resurrect a detached panel.
      if let input = previousInput as? NSView, input.window === window {
        window.makeFirstResponder(input)
      } else if restoreFocus, let content = window.contentView {
        WindowInputFocus.restore(in: window, content: content)
      }
    }
    previousInput = nil
    channel.invokeMethod("closed", arguments: ["id": id, "restoreFocus": restoreFocus])
  }
  private func detach() {
    restoreCloseButton()
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
  static func action(for event: NSEvent) -> String? {
    guard event.type == .keyDown else { return nil }
    let flags = event.modifierFlags.intersection([.command, .option, .control, .shift])
    if flags == .command {
      if event.keyCode == 37 { return "search" }
      if event.keyCode == 16 { return "close" }
    }
    guard flags.isEmpty else { return nil }
    switch event.keyCode {
    case 53, 49: return "close"
    case 125: return "next"
    case 126: return "previous"
    case 36, 76: return "activate"
    default: return nil
    }
  }
  private func route(_ event: NSEvent) -> Bool {
    guard ownsPanel, event.type == .keyDown, let id = resourceID else { return false }
    guard let action = Self.action(for: event) else { return false }
    if action == "close" {
      close()
    } else {
      channel.invokeMethod(action, arguments: ["id": id])
    }
    return true
  }
}
