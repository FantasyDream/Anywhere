import AppKit

private final class ContentFocusView: NSView {
  override var acceptsFirstResponder: Bool { true }
}

@main
struct WindowMaterialTests {
  @MainActor static func main() {
    _ = NSApplication.shared
    let frame = NSRect(x: 0, y: 0, width: 648, height: 300)
    let fallbackContent = NSView(frame: frame)
    let fallback = WindowMaterial(
      frame: frame,
      content: fallbackContent,
      cornerRadius: 18,
      preferLiquidGlass: false
    )
    assert(!fallback.usesLiquidGlass)
    let visual = fallback.subviews.first as! NSVisualEffectView
    assert(fallbackContent.superview === visual)
    assert(visual.blendingMode == .behindWindow)
    let mask = NSImage(size: NSSize(width: 37, height: 37))
    fallback.maskImage = mask
    assert(visual.maskImage === mask)
    fallback.cornerRadius = 0
    fallback.maskImage = nil
    assert(visual.maskImage == nil && fallback.layer?.cornerRadius == 0)

    let contentController = NSViewController()
    contentController.view = NSView(frame: frame)
    let host = WindowMaterialController(
      contentController: contentController,
      frame: frame,
      cornerRadius: 18
    )
    assert(contentController.parent === host)
    #if compiler(>=6.2)
      if #available(macOS 26.0, *) {
        assert(host.material.usesLiquidGlass)
        let glass = host.material.subviews.first as! NSGlassEffectView
        assert(glass.contentView === contentController.view)
        assert(glass.style == .regular && glass.cornerRadius == 18)
        host.material.cornerRadius = 0
        assert(glass.cornerRadius == 0)
      }
    #endif

    // Settings reuses the host and child controller; resizing must reach the content.
    let launcher = NSWindow(
      contentRect: frame,
      styleMask: [.borderless],
      backing: .buffered,
      defer: false
    )
    let settings = NSWindow(
      contentRect: frame,
      styleMask: [.titled, .fullSizeContentView],
      backing: .buffered,
      defer: false
    )
    launcher.isReleasedWhenClosed = false
    settings.isReleasedWhenClosed = false
    launcher.contentViewController = host
    assert(contentController.view.window === launcher)
    launcher.contentViewController = nil
    settings.contentViewController = host
    settings.setContentSize(NSSize(width: 648, height: 508))
    settings.contentView?.layoutSubtreeIfNeeded()
    assert(contentController.parent === host && contentController.view.window === settings)
    assert(contentController.view.bounds.size == NSSize(width: 648, height: 508))
    settings.contentViewController = nil
    let launcherFrame = launcher.frame
    launcher.contentViewController = host
    launcher.setFrame(launcherFrame, display: false)
    host.material.cornerRadius = 18
    launcher.contentView?.layoutSubtreeIfNeeded()
    assert(contentController.view.window === launcher)
    assert(contentController.view.bounds.size == frame.size)

    // Closing a dedicated window must release its native content and responder
    // while the controller (and, in production, its engine) is still alive.
    let editor = ContentFocusView(frame: frame)
    contentController.view.addSubview(editor)
    assert(launcher.makeFirstResponder(editor))
    assert(launcher.firstResponder === editor)
    host.detachContent()
    assert(contentController.parent == nil && host.children.isEmpty)
    assert(contentController.view.window == nil && contentController.view.superview == nil)
    assert(launcher.firstResponder !== editor)
    #if compiler(>=6.2)
      if #available(macOS 26.0, *) {
        assert((host.material.subviews.first as? NSGlassEffectView)?.contentView == nil)
      }
    #endif
    host.detachContent()
    assert(host.children.isEmpty)
    fallback.detachContent(fallbackContent)
    assert(fallbackContent.superview == nil)
    print(
      "Window material: containment, fallback, resize, reparenting and input teardown passed"
    )
  }
}
