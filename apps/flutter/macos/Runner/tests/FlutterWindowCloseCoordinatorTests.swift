import Foundation

@main
struct FlutterWindowCloseCoordinatorTests {
  @MainActor static func main() {
    let coordinator = FlutterWindowCloseCoordinator()
    var finish: (() -> Void)?
    var preparations = 0
    var closes = 0
    let prepare: (@escaping () -> Void) -> Void = {
      preparations += 1
      finish = $0
    }
    let close = { closes += 1 }
    assert(!coordinator.openAfterClose({ assertionFailure("Not closing") }))
    assert(!coordinator.shouldClose(prepare: prepare, close: close))
    assert(!coordinator.shouldClose(prepare: prepare, close: close))
    assert(preparations == 1 && closes == 0)
    var reopened = 0
    assert(coordinator.openAfterClose { reopened = 1 })
    assert(coordinator.openAfterClose { reopened = 2 })
    let oldFinish = finish!
    oldFinish()
    oldFinish()
    assert(closes == 1)
    assert(coordinator.shouldClose(prepare: prepare, close: close))
    coordinator.didClose()
    RunLoop.main.run(until: Date(timeIntervalSinceNow: 0.02))
    assert(reopened == 2)

    assert(!coordinator.shouldClose(prepare: prepare, close: close))
    oldFinish()
    assert(closes == 1, "A stale reply cannot close the next window")
    finish?()
    assert(closes == 2)
    coordinator.didClose()

    let stalled = FlutterWindowCloseCoordinator(timeout: 0.01)
    assert(!stalled.shouldClose(prepare: { _ in }, close: close))
    let deadline = Date(timeIntervalSinceNow: 1)
    while closes == 2 && Date() < deadline {
      RunLoop.main.run(until: Date(timeIntervalSinceNow: 0.01))
    }
    assert(closes == 3, "A failed Dart isolate must not trap the window")
    stalled.didClose()
    print("Tool close: acknowledgement, duplicate/stale replies, reopen and timeout passed")
  }
}
