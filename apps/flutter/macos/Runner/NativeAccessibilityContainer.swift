import AppKit

/// Adds an embedded native view to a framework-owned accessibility tree without
/// retaining that view or taking a snapshot of the framework's changing nodes.
class NativeAccessibilityContainer: NSView {
  private weak var embeddedChild: NSView?
  private weak var originalParent: AnyObject?

  func setEmbeddedAccessibilityChild(_ child: NSView?) {
    let next = child.flatMap { $0 !== self && $0.isDescendant(of: self) ? $0 : nil }
    guard embeddedChild !== next else { return }
    let previous = embeddedChild
    let parent = originalParent
    embeddedChild = nil
    originalParent = nil
    if let previous, previous.accessibilityParent() as AnyObject? === self {
      previous.setAccessibilityParent(parent)
    }
    // Capture before exposing this container as a group: AppKit computes the
    // default parent from the current ignored/unignored ancestor chain.
    originalParent = next?.accessibilityParent() as AnyObject?
    embeddedChild = next
    next?.setAccessibilityParent(self)
    NSAccessibility.post(element: self, notification: .layoutChanged)
  }

  private var accessibleChild: NSView? {
    guard let child = embeddedChild, child.isDescendant(of: self),
      !child.isHiddenOrHasHiddenAncestor
    else { return nil }
    return child
  }

  override func isAccessibilityElement() -> Bool {
    accessibleChild != nil || super.isAccessibilityElement()
  }

  override func accessibilityRole() -> NSAccessibility.Role? {
    accessibleChild != nil ? .group : super.accessibilityRole()
  }

  override func accessibilityChildren() -> [Any]? {
    let children = super.accessibilityChildren()
    guard let child = accessibleChild else { return children }
    let existing = children ?? []
    if existing.contains(where: { $0 as AnyObject === child }) { return existing }
    return existing + [child]
  }

  override func viewWillMove(toWindow newWindow: NSWindow?) {
    if window !== newWindow { setEmbeddedAccessibilityChild(nil) }
    super.viewWillMove(toWindow: newWindow)
  }
}
