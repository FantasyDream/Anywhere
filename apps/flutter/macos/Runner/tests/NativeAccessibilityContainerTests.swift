import AppKit

@main
struct NativeAccessibilityContainerTests {
  @MainActor static func main() {
    _ = NSApplication.shared
    let frame = NSRect(x: 0, y: 0, width: 400, height: 200)
    let host = NativeAccessibilityContainer(frame: frame)
    let framework = NSView(frame: frame)
    let firstNode = NSAccessibilityElement()
    let replacementNode = NSAccessibilityElement()
    host.addSubview(framework)
    host.setAccessibilityChildren([firstNode])
    let child = NSView(frame: frame)
    child.setAccessibilityElement(true)
    child.setAccessibilityRole(.group)
    framework.addSubview(child)
    let originalParent = child.accessibilityParent() as AnyObject?
    host.setEmbeddedAccessibilityChild(child)
    assert(host.isAccessibilityElement() && host.accessibilityRole() == .group)
    assert(child.accessibilityParent() as AnyObject? === host)
    assert(host.accessibilityChildren()?.count == 2)
    assert(host.accessibilityChildren()?.first as AnyObject? === firstNode)
    assert(host.accessibilityChildren()?.last as AnyObject? === child)

    // Framework semantics can be rebuilt while the native document stays open.
    // Read the current tree instead of retaining the old nodes or duplicating WebKit.
    host.setAccessibilityChildren([replacementNode])
    host.setEmbeddedAccessibilityChild(child)
    assert(host.accessibilityChildren()?.count == 2)
    assert(host.accessibilityChildren()?.first as AnyObject? === replacementNode)
    host.setAccessibilityChildren([replacementNode, child])
    assert(host.accessibilityChildren()?.count == 2)
    host.setAccessibilityChildren([replacementNode])

    child.isHidden = true
    assert(host.accessibilityChildren()?.count == 1)
    child.isHidden = false
    host.isHidden = true
    assert(host.accessibilityChildren()?.count == 1)
    host.isHidden = false
    assert(host.accessibilityChildren()?.count == 2)
    host.setEmbeddedAccessibilityChild(nil)
    assert(child.accessibilityParent() as AnyObject? === originalParent)
    assert(host.accessibilityChildren()?.count == 1)

    // Reject another window's view and stop exposing detached platform views.
    let unrelated = NSView(frame: frame)
    host.setEmbeddedAccessibilityChild(unrelated)
    assert(host.accessibilityChildren()?.count == 1)
    host.setEmbeddedAccessibilityChild(child)
    child.removeFromSuperview()
    assert(host.accessibilityChildren()?.count == 1)
    host.setEmbeddedAccessibilityChild(nil)
    assert(child.accessibilityParent() as AnyObject? === originalParent)

    // Neither side of the supplementary connection may keep an editor alive.
    weak var departedChild: NSView?
    autoreleasepool {
      let temporary = NSView(frame: frame)
      host.addSubview(temporary)
      host.setEmbeddedAccessibilityChild(temporary)
      departedChild = temporary
      temporary.removeFromSuperview()
    }
    assert(departedChild == nil)
    assert(host.accessibilityChildren()?.count == 1)

    // Moving the shared material into another window clears the old AX parent.
    framework.addSubview(child)
    host.setEmbeddedAccessibilityChild(child)
    host.viewWillMove(toWindow: NSWindow())
    assert(child.accessibilityParent() as AnyObject? === originalParent)
    assert(host.accessibilityChildren()?.count == 1)
    print(
      "Native accessibility: live children, visibility, parent restoration and weak ownership passed"
    )
  }
}
