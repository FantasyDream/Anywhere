import AppKit
import WebKit
import ObjectiveC

final class UnsupportedCanvasWebView: WKWebView {
    override func responds(to selector: Selector!) -> Bool {
        if NSStringFromSelector(selector) == "_setDrawsBackground:" { return false }
        return super.responds(to: selector)
    }
    override func setValue(_ value: Any?, forKey key: String) {
        precondition(key != "drawsBackground", "Unavailable WebKit SPI must not be called")
        super.setValue(value, forKey: key)
    }
}

final class LiveCanvasWebView: WKWebView {}

@main
struct NativeWebEditingTests {
    @MainActor static func main() {
        // Swift SDKs disagree on actor/sendable annotations for this callback.
        // Guard the stable Objective-C selector on this test subclass only;
        // keep the no-snapshot regression without guessing an SDK version.
        let selector = NSSelectorFromString("takeSnapshotWithConfiguration:completionHandler:")
        let method = class_getInstanceMethod(WKWebView.self, selector)!
        let rejectSnapshot: @convention(block) (AnyObject, AnyObject?, AnyObject) -> Void = { _, _, _ in
            preconditionFailure("Live canvas setup must not create an offscreen snapshot")
        }
        precondition(class_addMethod(LiveCanvasWebView.self, selector,
            imp_implementationWithBlock(rejectSnapshot), method_getTypeEncoding(method)))
        _ = NSApplication.shared
        let frame = NSRect(x: 0, y: 0, width: 400, height: 200)
        let window = NSWindow(contentRect: frame, styleMask: [.borderless], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        let content = NSView(frame: frame)
        window.contentView = content
        // Flutter may place the platform view beside its rendering surface.
        // Discover from this window's content root, not only the Flutter view.
        let flutterSurface = NSView(frame: frame)
        content.addSubview(flutterSurface)
        assert(NativeWebAppearance.configure(in: content) == nil)
        let config = WKWebViewConfiguration()
        config.websiteDataStore = .nonPersistent()
        let web = LiveCanvasWebView(frame: frame, configuration: config)
        content.addSubview(web)
        assert(NativeWebAppearance.configure(in: flutterSurface) == nil)
        let supported = web.responds(to: NSSelectorFromString("_setDrawsBackground:"))
        assert(!window.isKeyWindow)
        assert(NativeWebAppearance.configure(in: content) == supported)
        NativeWebEditing.focus(in: window, content: content)
        assert(window.firstResponder === web)
        if supported { assert(web.value(forKey: "drawsBackground") as? Bool == false) }
        if #available(macOS 12.0, *) { assert(web.underPageBackgroundColor.alphaComponent == 0) }
        web.removeFromSuperview()
        let fallback = UnsupportedCanvasWebView(frame: frame, configuration: config)
        content.addSubview(fallback)
        assert(NativeWebAppearance.configure(in: content) == false)
        NativeWebEditing.focus(in: window, content: content)
        assert(window.firstResponder === fallback)
        fallback.removeFromSuperview()
        print("Native Web editing checks passed (focus, canvas transparency, unsupported fallback)")
    }
}
