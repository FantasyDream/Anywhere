import AppKit
import WebKit

/// Presentation must not depend on the app being key or steal keyboard focus.
enum NativeWebAppearance {
  static func configure(in content: NSView) -> Bool? {
    guard let web = NativeWebView.unique(in: content) else { return nil }
    return configure(web)
  }

  /// flutter_inappwebview_macos 1.1.2 clears only the scroll-bounce color.
  /// No public macOS canvas switch exists. Isolate the guarded WebKit SPI;
  /// an unavailable selector keeps the Host's generated opaque fallback.
  static func configure(_ web: WKWebView) -> Bool {
    if #available(macOS 12.0, *) { web.underPageBackgroundColor = .clear }
    guard web.responds(to: NSSelectorFromString("_setDrawsBackground:")) else { return false }
    web.setValue(false, forKey: "drawsBackground")
    return true
  }
}
