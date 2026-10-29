import AppKit

/// A Flutter controller's root view is a wrapper, not its keyboard receiver.
/// Preserve an active editor; only find a fallback when content has no responder.
enum WindowInputFocus {
  /// NSTextView also offers Quick Look and precedes the window in the responder
  /// chain. Route list keys through the content receiver while our preview owns it.
  static func focusKeyboardReceiver(in window: NSWindow, content: NSView) -> Bool {
    guard content.window === window,
      let receiver = acceptingView(in: content, excludingTextViews: true)
    else { return false }
    return window.makeFirstResponder(receiver)
  }

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

  private static func acceptingView(in view: NSView, excludingTextViews: Bool = false) -> NSView? {
    if excludingTextViews && view is NSTextView { return nil }
    if view.acceptsFirstResponder { return view }
    for child in view.subviews {
      if let receiver = acceptingView(in: child, excludingTextViews: excludingTextViews) {
        return receiver
      }
    }
    return nil
  }
}
