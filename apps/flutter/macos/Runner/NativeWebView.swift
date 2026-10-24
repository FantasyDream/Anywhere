import AppKit
import WebKit

/// A departing platform view may briefly overlap its replacement. Native host
/// operations must not guess which document should receive focus or exposure.
enum NativeWebView {
  static func unique(in content: NSView) -> WKWebView? {
    var pending = [content]
    var result: WKWebView?
    while let view = pending.popLast() {
      if let web = view as? WKWebView {
        guard result == nil else { return nil }
        result = web
      } else {
        pending.append(contentsOf: view.subviews)
      }
    }
    return result
  }

  /// Flutter 3.47's wrapper exposes only its rendering view as an AX child.
  /// Bridge WebKit through our outer container using public AppKit APIs.
  static func exposeAccessibility(in content: NSView, enabled: Bool) {
    guard let container = content as? NativeAccessibilityContainer else { return }
    container.setEmbeddedAccessibilityChild(enabled ? unique(in: content) : nil)
  }
}
