import AppKit
import WebKit

/// Presentation must not depend on the app being key or steal keyboard focus.
enum NativeWebAppearance {
    private static func find(in content: NSView) -> WKWebView? {
        if let web = content as? WKWebView { return web }
        for child in content.subviews {
            if let web = find(in: child) { return web }
        }
        return nil
    }

    static func configure(in content: NSView) -> Bool? {
        guard let web = find(in: content) else { return nil }
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
