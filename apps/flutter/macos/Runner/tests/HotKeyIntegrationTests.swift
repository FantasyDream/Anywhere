import AppKit
import Carbon

/// Opt-in integration test: real Carbon registration, no settings files or key synthesis.
@main
struct HotKeyIntegrationTests {
    enum ProbeFailure: Error { case assertion(String) }
    static func require(_ condition: @autoclosure () -> Bool, _ message: String = "Carbon registration assertion failed", line: Int = #line) throws {
        if !condition() { throw ProbeFailure.assertion("\(message) at line \(line)") }
    }
    static func main() {
        do {
            if CommandLine.arguments.contains("--competitor") { try holdCompetitor(); return }
            try run()
        }
        catch { fputs("Live Carbon failed: \(error)\n", stderr); exit(1) }
    }
    static func holdCompetitor() throws {
        _ = NSApplication.shared
        let competitor = CarbonHotKeyBackend()
        defer { competitor.shutdown() }
        try require(competitor.install(onActivate: { _ in }) == noErr)
        try require(competitor.register(code: UInt32(kVK_ANSI_Y), modifiers: UInt32(cmdKey | optionKey | controlKey | shiftKey), id: 800) == noErr)
        print("READY")
        fflush(stdout)
        _ = readLine() // Parent closes this pipe in its defer, even when an assertion fails.
    }
    static func run() throws {
        _ = NSApplication.shared
        let modifiers = UInt32(cmdKey | optionKey | controlKey | shiftKey)
        let settings = StoredSettings(currentAppKeyCode: UInt32(kVK_ANSI_Q), currentAppModifiers: modifiers,
            universalKeyCode: UInt32(kVK_ANSI_W), universalModifiers: modifiers,
            launchAtLogin: false, excludedApps: [],
            actionShortcuts: [StoredActionShortcut(actionId: "test.action", title: "Test action",
                keyCode: UInt32(kVK_ANSI_T), modifiers: modifiers, global: true),
                StoredActionShortcut(actionId: "test.second", title: "Second action",
                keyCode: UInt32(kVK_ANSI_X), modifiers: modifiers, global: true)])
        let backend = CarbonHotKeyBackend()
        let competitor = CarbonHotKeyBackend()
        let service = HotKeyService(backend: backend, supportedActionIds: [], onActivate: { _ in })
        defer { service.shutdown(); competitor.shutdown() }
        try require(service.apply(settings) == nil, "Temporary wake keys could not register")
        try require(competitor.install(onActivate: { _ in }) == noErr)
        try require(service.syncActionCatalog(["test.action", "test.second"]) == nil)
        let duplicate = competitor.register(code: UInt32(kVK_ANSI_T), modifiers: modifiers, id: 700)
        try require(duplicate != noErr, "Action registration did not occupy the key")
        service.setActionShortcutsSuspended(true)
        try require(competitor.register(code: UInt32(kVK_ANSI_T), modifiers: modifiers, id: 701) == noErr,
            "Suspending action keys did not release the key")
        try require(service.resumeForSettingsClose() != nil, "Resume conflict was not reported")
        try require(competitor.register(code: UInt32(kVK_ANSI_X), modifiers: modifiers, id: 710) == noErr,
            "Failed settings close retained a partially restored action key")
        competitor.clear()
        try require(service.resumeForSettingsClose() == nil, "Retrying settings close did not restore the group")
        try require(service.syncActionCatalog(["test.action"]) == nil)
        try require(service.lastError == nil, "A successful retry kept a stale resume error")
        try require(service.syncActionCatalog([]) == nil)
        try require(competitor.register(code: UInt32(kVK_ANSI_T), modifiers: modifiers, id: 702) == noErr,
            "Revoking an action did not release the system key")
        competitor.clear()
        try require(service.syncActionCatalog(["test.action"]) == nil)
        let child = Process()
        child.executableURL = URL(fileURLWithPath: CommandLine.arguments[0])
        child.arguments = ["--competitor"]
        let output = Pipe(), input = Pipe()
        child.standardOutput = output
        child.standardInput = input
        try child.run()
        try? output.fileHandleForWriting.close()
        try? input.fileHandleForReading.close()
        defer { try? input.fileHandleForWriting.close(); child.waitUntilExit() }
        let ready = try output.fileHandleForReading.read(upToCount: 6)
        try require(ready == Data("READY\n".utf8), "Competing process could not register its key")
        var proposed = settings
        proposed.actionShortcuts = [StoredActionShortcut(actionId: "test.action", title: "Test action", keyCode: UInt32(kVK_ANSI_Y), modifiers: modifiers, global: true)]
        let failed = service.update(proposed, previous: settings, supportedActionIds: ["test.action"])
        try require(failed.error != nil && failed.settings.actionShortcuts == settings.actionShortcuts)
        try require(competitor.register(code: UInt32(kVK_ANSI_T), modifiers: modifiers, id: 704) != noErr,
            "Conflict rollback did not restore the previous key")
        service.shutdown()
        try require(competitor.register(code: UInt32(kVK_ANSI_Q), modifiers: modifiers, id: 705) == noErr)
        try require(competitor.register(code: UInt32(kVK_ANSI_W), modifiers: modifiers, id: 706) == noErr)
        try require(competitor.register(code: UInt32(kVK_ANSI_T), modifiers: modifiers, id: 707) == noErr)
        print("Live Carbon: registration, duplicate rejection, suspension, failed-close partial rollback, conflict recovery, revocation, cross-process conflict rollback and shutdown passed")
    }
}
