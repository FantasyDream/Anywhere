import AppKit
import FlutterMacOS

@main
struct FlutterEngineLifecycleTests {
  @MainActor static func main() {
    _ = NSApplication.shared
    weak var releasedController: FlutterViewController?
    weak var releasedEngine: FlutterEngine?
    autoreleasepool {
      let controller = FlutterViewController(project: FlutterDartProject())
      let other = FlutterViewController(project: FlutterDartProject())
      let engine = controller.engine
      releasedController = controller
      releasedEngine = engine
      assert(engine.viewController === controller)
      assert(controller.attached())
      FlutterEngineLifecycle.detach(controller)
      assert(engine.viewController == nil)
      assert(!controller.attached())
      FlutterEngineLifecycle.detach(controller)
      FlutterEngineLifecycle.detach(nil)
      assert(other.engine.viewController === other)
      FlutterEngineLifecycle.detach(other)
    }
    assert(releasedController == nil)
    assert(releasedEngine == nil)
    print("Standalone Flutter engines: explicit detach, idempotence, isolation and release passed")
  }
}
