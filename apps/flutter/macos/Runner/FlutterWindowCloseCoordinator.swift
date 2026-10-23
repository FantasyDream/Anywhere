import Foundation

/// Main-thread close handshake for a window with its own Dart isolate.
/// Let Flutter unmount editors and reply to input before shutting the engine down.
final class FlutterWindowCloseCoordinator {
  private enum Phase { case idle, preparing, prepared }
  private var phase = Phase.idle
  private var generation = 0
  private var fallback: DispatchWorkItem?
  private var reopen: (() -> Void)?
  private let timeout: TimeInterval

  init(timeout: TimeInterval = 2) { self.timeout = timeout }
  deinit { fallback?.cancel() }

  func shouldClose(
    prepare: (@escaping () -> Void) -> Void,
    close: @escaping () -> Void
  ) -> Bool {
    if phase == .prepared { return true }
    guard phase == .idle else { return false }
    phase = .preparing
    generation &+= 1
    let ticket = generation
    let finish = { [weak self] in
      guard let self, self.generation == ticket, self.phase == .preparing else { return }
      self.phase = .prepared
      self.fallback?.cancel()
      self.fallback = nil
      close()
    }
    let work = DispatchWorkItem(block: finish)
    fallback = work
    DispatchQueue.main.asyncAfter(deadline: .now() + timeout, execute: work)
    prepare(finish)
    return false
  }

  /// An open request during unmounting must create a fresh isolate after close.
  func openAfterClose(_ action: @escaping () -> Void) -> Bool {
    guard phase != .idle else { return false }
    reopen = action
    return true
  }

  func didClose() {
    generation &+= 1
    fallback?.cancel()
    fallback = nil
    phase = .idle
    let action = reopen
    reopen = nil
    if let action { DispatchQueue.main.async(execute: action) }
  }
}
