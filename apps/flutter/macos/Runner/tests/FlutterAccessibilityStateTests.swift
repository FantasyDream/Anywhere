import Foundation

@main
struct FlutterAccessibilityStateTests {
  static func main() {
    let center = NotificationCenter()
    var state: FlutterAccessibilityState? = FlutterAccessibilityState(center: center)
    var observed: [Bool] = []
    let observer = center.addObserver(
      forName: FlutterAccessibilityState.notification,
      object: nil,
      queue: nil
    ) { event in
      if let enabled = event.userInfo?[FlutterAccessibilityState.key] as? Bool {
        observed.append(enabled)
      }
    }

    // No observed request must not opt the app into semantics.
    state?.synchronizeNewEngine()
    assert(observed.isEmpty)
    center.post(
      name: FlutterAccessibilityState.notification,
      object: nil,
      userInfo: [FlutterAccessibilityState.key: true]
    )
    observed.removeAll()
    // A late engine receives the already-active state, without another OS toggle.
    state?.synchronizeNewEngine()
    assert(observed == [true])

    center.post(
      name: FlutterAccessibilityState.notification,
      object: nil,
      userInfo: [FlutterAccessibilityState.key: false]
    )
    center.post(name: FlutterAccessibilityState.notification, object: nil, userInfo: [:])
    observed.removeAll()
    state?.synchronizeNewEngine()
    assert(observed == [false])

    // Teardown removes the observer and releases the retained state.
    weak var released: FlutterAccessibilityState?
    autoreleasepool { released = state }
    state = nil
    assert(released == nil)
    center.removeObserver(observer)
    print("Flutter accessibility: no forced enable, late engine sync, disable and release passed")
  }
}
