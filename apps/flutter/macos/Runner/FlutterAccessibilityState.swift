import Foundation

/// Replays the last AppKit accessibility state to engines created after the
/// original notification. This is an adapter for the pinned Flutter macOS
/// notification contract, not a new permission or an always-on semantics tree.
/// Recheck FlutterEngine.mm's onAccessibilityStatusChanged when upgrading Flutter.
final class FlutterAccessibilityState {
  static let shared = FlutterAccessibilityState()
  static let notification = Notification.Name(
    "NSApplicationDidChangeAccessibilityEnhancedUserInterfaceNotification"
  )
  static let key = "AXEnhancedUserInterface"
  private let center: NotificationCenter
  private var observer: NSObjectProtocol?
  private var enabled: Bool?
  private var replaying = false

  init(center: NotificationCenter = .default) {
    self.center = center
    observer = center.addObserver(forName: Self.notification, object: nil, queue: .main) {
      [weak self] event in
      guard let enabled = event.userInfo?[Self.key] as? Bool else { return }
      self?.enabled = enabled
    }
  }

  deinit {
    if let observer { center.removeObserver(observer) }
  }

  func synchronizeNewEngine() {
    guard let enabled, !replaying else { return }
    replaying = true
    defer { replaying = false }
    center.post(name: Self.notification, object: nil, userInfo: [Self.key: enabled])
  }
}
