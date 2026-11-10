import AppKit
import Darwin
import FlutterMacOS
import WebKit

private func verify(_ condition: @autoclosure () -> Bool, line: UInt = #line) {
  if !condition() {
    fputs("Web tools channel failed at line \(line)\n", stderr)
    exit(EXIT_FAILURE)
  }
}

@main
struct WebToolsHostTests {
  @MainActor static func main() {
    _ = NSApplication.shared
    let frame = NSRect(x: 0, y: 0, width: 400, height: 200)
    let window = NSWindow(
      contentRect: frame,
      styleMask: [.borderless],
      backing: .buffered,
      defer: false
    )
    window.isReleasedWhenClosed = false
    window.isMovableByWindowBackground = true
    let content = NativeAccessibilityContainer(frame: frame)
    window.contentView = content
    let editor = NSTextView(frame: frame)
    content.addSubview(editor)
    window.makeFirstResponder(editor)
    let messenger = HostChannelTestMessenger()
    let channel = "anywhere/web-tools"
    let codec = FlutterStandardMethodCodec.sharedInstance()
    let success = codec.encodeSuccessEnvelope(nil)
    var logs: [[String: Any]] = []
    var host: WebToolsHost? = WebToolsHost(
      messenger: messenger,
      window: window,
      content: content,
      log: { logs.append($0) }
    )
    for method in ["focusHost", "focusWeb", "configureWebAppearance", "unknown"] {
      verify(messenger.request(channel, method).responses == [nil])
    }
    verify(messenger.request(channel, "active", "yes").responses == [nil])
    verify(host?.isActive == false && window.isMovableByWindowBackground)
    verify(messenger.request(channel, "active", true).responses == [success])
    verify(host?.isActive == true && !window.isMovableByWindowBackground)
    verify(messenger.request(channel, "configureWebAppearance").responses == [success])
    verify(logs.last?["web_view_found"] as? Bool == false)
    let config = WKWebViewConfiguration()
    config.websiteDataStore = .nonPersistent()
    let web = WKWebView(frame: frame, configuration: config)
    content.addSubview(web)
    let originalParent = web.accessibilityParent() as AnyObject?
    let configured = messenger.request(channel, "configureWebAppearance")
    verify(configured.responses.count == 1 && configured.responses[0] != nil)
    verify(
      codec.decodeEnvelope(configured.responses[0]!) as? Bool
        == web.responds(to: NSSelectorFromString("_setDrawsBackground:"))
    )
    verify(web.accessibilityParent() as AnyObject? === content)
    verify(window.firstResponder === editor)
    verify(messenger.request(channel, "focusHost").responses == [success])
    verify(web.accessibilityParent() as AnyObject? === originalParent)
    verify(messenger.request(channel, "focusWeb").responses == [success])
    verify(web.accessibilityParent() as AnyObject? === content)
    verify(window.firstResponder === editor, line: #line)
    verify(host?.performEdit("copy") == false)
    verify(messenger.request(channel, "active", false).responses == [success])
    verify(web.accessibilityParent() as AnyObject? === originalParent)
    verify(host?.isActive == false && window.isMovableByWindowBackground)
    _ = messenger.request(channel, "active", true)
    _ = messenger.request(channel, "configureWebAppearance")
    host?.hideAccessibility()
    verify(web.accessibilityParent() as AnyObject? === originalParent)
    _ = messenger.request(channel, "configureWebAppearance")
    let released = { [weak host] in host == nil }
    host = nil
    verify(released() && messenger.handlers[channel] == nil)
    verify(web.accessibilityParent() as AnyObject? === originalParent)
    verify(window.isMovableByWindowBackground)
    print("Web tools channel: active mode, exposure, canvas, non-key focus and release passed")
  }
}
