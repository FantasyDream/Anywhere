import FlutterMacOS

/// Only for windows that own a standalone engine, never the shared Launcher.
enum FlutterEngineLifecycle {
  static func detach(_ controller: FlutterViewController?) {
    guard let controller, controller.attached() else { return }
    // Remove the view and its VSync waiter while the controller is still alive.
    // The implicit engine has allowHeadlessExecution=false, so the public setter
    // also shuts it down. Releasing the controller first leaves teardown to deinit.
    controller.engine.viewController = nil
  }
}
