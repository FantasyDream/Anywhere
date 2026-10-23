import AppKit

private final class Delegate: NSObject, NSWindowDelegate {}

@main
struct LauncherWindowCoordinatorTests {
  @MainActor static func main() {
    _ = NSApplication.shared
    let frame = NSRect(x: 100, y: 100, width: 648, height: 300)
    let launcher = NSWindow(
      contentRect: frame,
      styleMask: [.borderless],
      backing: .buffered,
      defer: false
    )
    launcher.isReleasedWhenClosed = false
    let content = NSViewController()
    content.view = NSView(frame: NSRect(origin: .zero, size: frame.size))
    let host = WindowMaterialController(
      contentController: content,
      frame: content.view.frame,
      cornerRadius: 18
    )
    launcher.contentViewController = host
    let delegate = Delegate()
    let coordinator = LauncherWindowCoordinator(
      window: launcher,
      controller: content,
      host: host,
      glass: host.material,
      mask: nil,
      delegate: delegate,
      tasksVisible: { false }
    )

    weak var firstWindow: NSWindow?
    autoreleasepool {
      coordinator.settingsOpen = true
      coordinator.showSettingsWindow(height: 508)
      let first = coordinator.settingsWindow!
      firstWindow = first
      first.setFrameOrigin(NSPoint(x: 80, y: 90))
      assert(content.view.window === first && first.isVisible)
      assert(content.parent === host)

      coordinator.settingsOpen = false
      coordinator.restoreLauncherView()
      assert(content.view.window === launcher)
      assert(coordinator.settingsWindow == nil)
      assert(!first.isVisible && first.contentViewController == nil)
    }
    assert(firstWindow == nil, "The empty Settings window must be released")

    coordinator.settingsOpen = true
    coordinator.showSettingsWindow(height: 508)
    let second = coordinator.settingsWindow!
    assert(content.view.window === second)
    assert(second.frame.origin == NSPoint(x: 80, y: 90))
    assert(content.parent === host && host.children.count == 1)
    coordinator.settingsOpen = false
    coordinator.hideLauncherWindow()
    assert(coordinator.settingsWindow == nil && !second.isVisible)
    assert(content.view.window === launcher)
    assert(host.material.isHidden)
    print(
      "Launcher windows: shared content survives; empty Settings windows close; placement is retained"
    )
  }
}
