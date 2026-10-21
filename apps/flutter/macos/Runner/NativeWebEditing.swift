import AppKit
import WebKit

/// WebKit owns its own input connection. Never route its Edit commands through
/// Flutter's accessibility editor or change the ordinary native-tool edit path.
enum NativeWebEditing {
    static func focus(in window: NSWindow, content: NSView) {
        if let web = content as? WKWebView {
            window.makeFirstResponder(web)
            return
        }
        for child in content.subviews {
            focus(in: window, content: child)
        }
    }
    static func perform(_ command: String, in window: NSWindow) -> Bool {
        guard var view = window.firstResponder as? NSView else { return false }
        while !(view is WKWebView) {
            guard let parent = view.superview else { return false }
            view = parent
        }
        let selectors = ["undo": "undo:", "redo": "redo:", "cut": "cut:",
                         "copy": "copy:", "paste": "paste:", "selectAll": "selectAll:"]
        guard let selector = selectors[command] else { return false }
        _ = NSApp.sendAction(NSSelectorFromString(selector), to: nil, from: nil)
        return true
    }
}
