import AppKit

protocol ApplicationReturnTarget: AnyObject {
    var processIdentifier: pid_t { get }
    var bundleURL: URL? { get }
    var isTerminated: Bool { get }
}
extension NSRunningApplication: ApplicationReturnTarget {}

/// A bounded, asynchronous focus handoff. Completion means the original process
/// is actually frontmost, not merely that LaunchServices accepted a request.
final class ApplicationFocusReturner {
    enum Outcome: String { case returned, skipped, unavailable, failed }
    typealias Open = (URL, @escaping (pid_t?) -> Void) -> Void
    typealias Schedule = (TimeInterval, @escaping () -> Void) -> Void

    private let open: Open
    private let frontmost: () -> pid_t?
    private let schedule: Schedule

    init(open: @escaping Open = ApplicationFocusReturner.openApplication,
         frontmost: @escaping () -> pid_t? = { NSWorkspace.shared.frontmostApplication?.processIdentifier },
         schedule: @escaping Schedule = { delay, work in
             DispatchQueue.main.asyncAfter(deadline: .now() + delay, execute: work)
         }) {
        self.open = open
        self.frontmost = frontmost
        self.schedule = schedule
    }

    func returnFocus(to target: ApplicationReturnTarget?,
                     mayReturn: @escaping () -> Bool,
                     completion: @escaping (Outcome) -> Void) {
        guard mayReturn() else { completion(.skipped); return }
        guard let target, !target.isTerminated, let url = target.bundleURL else {
            completion(.unavailable); return
        }
        let pid = target.processIdentifier
        if frontmost() == pid { completion(.returned); return }
        var finished = false
        func finish(_ outcome: Outcome) {
            guard !finished else { return }
            finished = true
            completion(outcome)
        }
        func verify() {
            guard !finished else { return }
            if target.isTerminated { finish(.unavailable) }
            else if frontmost() == pid { finish(.returned) }
            else if !mayReturn() { finish(.skipped) }
            else { schedule(0.05, verify) }
        }
        // Both a missing completion and an accepted-but-ineffective activation
        // must release Flutter's pending command. Late callbacks are harmless.
        schedule(2) {
            if target.isTerminated { finish(.unavailable) }
            else if self.frontmost() == pid { finish(.returned) }
            else { finish(mayReturn() ? .failed : .skipped) }
        }
        open(url) { openedPID in
            guard !finished else { return }
            guard !target.isTerminated else { finish(.unavailable); return }
            if self.frontmost() == pid { finish(.returned); return }
            guard mayReturn() else { finish(.skipped); return }
            // Never claim that a replacement process is the captured target.
            guard openedPID == pid else { finish(.failed); return }
            verify()
        }
    }

    private static func openApplication(_ url: URL, completion: @escaping (pid_t?) -> Void) {
        let configuration = NSWorkspace.OpenConfiguration()
        configuration.activates = true
        configuration.createsNewApplicationInstance = false
        configuration.promptsUserIfNeeded = false
        NSWorkspace.shared.openApplication(at: url, configuration: configuration) { app, error in
            DispatchQueue.main.async {
                completion(error == nil ? app?.processIdentifier : nil)
            }
        }
    }
}
