import AppKit

/// A Flutter controller's root view is a wrapper, not its keyboard receiver.
/// Preserve an active editor; only find a fallback when content has no responder.
enum WindowInputFocus {
  static func restore(in window: NSWindow, content: NSView) {
    guard content.window === window else { return }
    if let current = window.firstResponder as? NSView,
      current.acceptsFirstResponder,
      current === content || current.isDescendant(of: content)
    {
      return
    }
    if let receiver = acceptingView(in: content) {
      window.makeFirstResponder(receiver)
    }
  }

  private static func acceptingView(in view: NSView) -> NSView? {
    if view.acceptsFirstResponder { return view }
    for child in view.subviews {
      if let receiver = acceptingView(in: child) { return receiver }
    }
    return nil
  }
}
