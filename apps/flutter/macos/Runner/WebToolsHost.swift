import AppKit
import Carbon
import FlutterMacOS

/// Owns Web/Host focus mode and the shared engine's native Web tools channel.
final class WebToolsHost {
  private let channel: FlutterMethodChannel
  private weak var window: NSWindow?
  private weak var flutterContent: NSView?
  private let log: ([String: Any]) -> Void
  private(set) var isActive = false
  private var hostPanelVisible = false

  init(
    messenger: FlutterBinaryMessenger,
    window: NSWindow,
    content: NSView,
    log: @escaping ([String: Any]) -> Void
  ) {
    self.window = window
    flutterContent = content
    self.log = log
    channel = FlutterMethodChannel(name: "anywhere/web-tools", binaryMessenger: messenger)
    channel.setMethodCallHandler { [weak self] call, reply in
      guard let self else {
        reply(nil)
        return
      }
      switch call.method {
      case "active":
        guard let active = call.arguments as? Bool else {
          reply(FlutterMethodNotImplemented)
          return
        }
        self.isActive = active
        if !active {
          self.hostPanelVisible = false
          self.updateAccessibility()
        }
        // Background dragging otherwise steals WebKit mouse-up/click events.
        self.window?.isMovableByWindowBackground = !active
        reply(nil)
      case "focusHost" where self.isActive:
        self.hostPanelVisible = true
        self.updateAccessibility()
        // Search only Flutter's view; a sibling WebView must not retain focus.
        if let window = self.window, NSApp.keyWindow === window, let content = self.flutterContent {
          window.makeFirstResponder(nil)
          WindowInputFocus.restore(in: window, content: content)
        }
        reply(nil)
      case "focusWeb" where self.isActive:
        self.hostPanelVisible = false
        self.updateAccessibility()
        if let window = self.window, NSApp.keyWindow === window, let content = self.content {
          NativeWebEditing.focus(in: window, content: content)
        }
        reply(nil)
      case "configureWebAppearance" where self.isActive:
        self.updateAccessibility()
        let transparent = self.content.flatMap { NativeWebAppearance.configure(in: $0) }
        self.log([
          "event": "web_canvas_configured", "web_view_found": transparent != nil,
          "transparent": transparent ?? false,
        ])
        reply(transparent)
      default: reply(FlutterMethodNotImplemented)
      }
    }
  }

  private var content: NSView? { window?.contentView ?? flutterContent }
  private func updateAccessibility() {
    guard let content else { return }
    NativeWebView.exposeAccessibility(in: content, enabled: isActive && !hostPanelVisible)
  }
  func hideAccessibility() {
    guard let content else { return }
    NativeWebView.exposeAccessibility(in: content, enabled: false)
  }

  func performEdit(_ command: String) -> Bool {
    guard isActive, let window, NSApp.keyWindow === window else { return false }
    return NativeWebEditing.perform(command, in: window)
  }

  func handleKey(_ event: NSEvent) -> Bool {
    guard isActive, NSApp.isActive, let window,
      NSApp.keyWindow === window, event.window === window
    else { return false }
    let composing = (window.firstResponder as? NSTextInputClient)?.hasMarkedText() ?? false
    guard !composing else { return false }
    let flags = event.modifierFlags.intersection([.command, .option, .control, .shift])
    if event.keyCode == 53 {
      channel.invokeMethod("back", arguments: nil)
      return true
    }
    if flags == .command, event.keyCode == UInt16(kVK_ANSI_K) {
      channel.invokeMethod("actions", arguments: nil)
      return true
    }
    return false
  }

  deinit {
    channel.setMethodCallHandler(nil)
    hideAccessibility()
    window?.isMovableByWindowBackground = true
  }
}
