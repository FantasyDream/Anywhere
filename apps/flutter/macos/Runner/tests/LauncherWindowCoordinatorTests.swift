import AppKit
import Darwin

private func verify(_ condition: @autoclosure () -> Bool, line: UInt = #line) {
  if !condition() {
    fputs("Launcher windows failed at line \(line)\n", stderr)
    exit(EXIT_FAILURE)
  }
}

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
    var shown: [Bool] = []
    var hides = 0
    let coordinator = LauncherWindowCoordinator(
      window: launcher,
      controller: content,
      host: host,
      glass: host.material,
      mask: nil,
      delegate: delegate,
      tasksVisible: { false },
      willShow: { shown.append($0) },
      willHide: { hides += 1 }
    )

    weak var firstWindow: NSWindow?
    autoreleasepool {
      verify(coordinator.show(generation: 1, settings: true, height: 508))
      let first = coordinator.settingsWindow!
      firstWindow = first
      first.setFrameOrigin(NSPoint(x: 80, y: 90))
      verify(content.view.window === first && first.isVisible)
      verify(content.parent === host)

      verify(coordinator.show(generation: 2, settings: false, height: 200))
      verify(content.view.window === launcher)
      verify(coordinator.settingsWindow == nil)
      verify(!first.isVisible && first.contentViewController == nil)
    }
    verify(firstWindow == nil)

    verify(coordinator.show(generation: 3, settings: true, height: 508))
    let second = coordinator.settingsWindow!
    verify(content.view.window === second)
    verify(second.frame.origin == NSPoint(x: 80, y: 90))
    verify(content.parent === host && host.children.count == 1)
    verify(coordinator.hide(generation: 3))
    verify(coordinator.settingsWindow == nil && !second.isVisible)
    verify(content.view.window === launcher && host.material.isHidden)
    verify(coordinator.presentedGeneration == nil && !coordinator.settingsOpen)
    verify(shown == [true, false, true] && hides == 1)

    let hiddenFrame = launcher.frame
    verify(!coordinator.show(generation: 3, settings: false, height: 400))
    verify(!coordinator.resize(generation: 3, height: 400))
    verify(launcher.frame == hiddenFrame && shown.count == 3 && hides == 1)
    verify(coordinator.show(generation: 4, settings: false, height: 200, width: 500))
    let currentFrame = launcher.frame
    verify(!coordinator.resize(generation: 3, height: 400))
    verify(!coordinator.hide(generation: 3))
    verify(!coordinator.show(generation: 2, settings: true, height: 508))
    verify(launcher.frame == currentFrame && !coordinator.settingsOpen)
    verify(coordinator.presentedGeneration == 4 && hides == 1 && shown.count == 4)
    verify(coordinator.resize(generation: 4, height: 330, width: 600))
    verify(launcher.frame != currentFrame)
    verify(coordinator.dismissForFocusLoss() == 4)
    verify(coordinator.dismissForFocusLoss() == nil && hides == 2)
    verify(!coordinator.show(generation: 4, settings: false, height: 400))
    verify(coordinator.show(generation: 5, settings: true, height: 508))
    verify(coordinator.dismissForFocusLoss() == nil && hides == 2)
    verify(coordinator.settingsOpen && coordinator.presentedGeneration == 5)
    verify(coordinator.hide(generation: 5))
    // A hide may overtake a still-pending show. That show cannot restore a window.
    verify(coordinator.hide(generation: 8))
    verify(!coordinator.show(generation: 8, settings: false, height: 400))
    verify(coordinator.show(generation: 9, settings: false, height: 200))
    verify(coordinator.hide(generation: 9))
    verify(coordinator.presentedGeneration == nil && coordinator.settingsWindow == nil)
    print(
      "Launcher windows: shared content, Settings release, placement and stale presentation rejection passed"
    )
  }
}
