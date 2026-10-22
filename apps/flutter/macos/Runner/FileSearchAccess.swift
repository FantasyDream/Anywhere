import AppKit
import Darwin

/// Main-thread UI ownership; blocking bookmark/filesystem work has one serial worker.
final class FileSearchAccess {
    typealias Request = ([URL], @escaping ([URL]) -> Void) -> Void
    typealias Work = (@escaping () -> Void) -> Void
    private let request: Request?
    private let work: Work
    private let storage: FileSearchAccessStorage
    private let epoch = FileSearchAccessEpoch()
    private var reviewedScope: [String]?
    private var panel: NSOpenPanel?
    private(set) var status = ""
    private(set) var ready = false

    init(storage: URL = SettingsStore.supportDirectory().appendingPathComponent("file-search-access.plist"),
         request: Request? = nil, work: Work? = nil) {
        self.storage = FileSearchAccessStorage(storage)
        self.request = request
        let queue = DispatchQueue(label: "anywhere.file-access", qos: .userInitiated)
        self.work = work ?? { operation in queue.async(execute: DispatchWorkItem(block: operation)) }
    }

    static func contains(_ root: URL, _ target: URL) -> Bool {
        let base = root.standardizedFileURL.resolvingSymlinksInPath().pathComponents
        return target.standardizedFileURL.resolvingSymlinksInPath().pathComponents.starts(with: base)
    }

    private func deliver(_ operation: @escaping () -> Void) {
        if Thread.isMainThread { operation() } else { DispatchQueue.main.async(execute: operation) }
    }

    func prepare(enabled: Bool, roots: [String], force: Bool = false,
                 completion: @escaping ([String: Any]) -> Void) {
        let version = epoch.advance()
        panel?.cancel(nil)
        panel = nil
        ready = false
        let scope = (roots.isEmpty ? [FileManager.default.homeDirectoryForCurrentUser.path] : roots)
            .map { URL(fileURLWithPath: $0, isDirectory: true).standardizedFileURL }
        let signature = scope.map { $0.path }
        let reviewed = !force && reviewedScope == signature
        if !enabled { reviewedScope = nil; status = "" }
        let store = storage, gate = epoch
        // This watchdog ends only the waiting UI. The physical worker retains its
        // serial slot until I/O really returns, so timeouts cannot pile up threads.
        var replied = false
        let reply: ([String: Any]) -> Void = { result in
            guard !replied else { return }; replied = true; completion(result)
        }
        let timeout = DispatchWorkItem { [weak self] in
            guard let self, gate.isCurrent(version), !replied else { return }
            _ = gate.advance()
            self.ready = false
            self.status = "Directory access check timed out. Review Search settings."
            self.work { _ = store.release() }
            reply(["roots": [], "restricted": true, "message": self.status])
        }
        DispatchQueue.main.asyncAfter(deadline: .now() + 5, execute: timeout)
        work { [weak self] in
            guard gate.isCurrent(version) else {
                self?.deliver { timeout.cancel(); reply(["roots": [], "superseded": true]) }; return
            }
            let result = enabled ? store.restore(scope) : store.release()
            if !gate.isCurrent(version) { _ = store.release() }
            self?.deliver { [weak self] in
                timeout.cancel()
                guard let self, gate.isCurrent(version) else {
                    reply(["roots": [], "superseded": true]); return
                }
                guard enabled else { reply(["roots": [], "restricted": true]); return }
                if !force && (result.complete || reviewed) {
                    self.finish(result, reply: reply); return
                }
                self.reviewedScope = signature
                let accepted: ([URL]) -> Void = { [weak self] urls in
                    guard let self, gate.isCurrent(version) else {
                        reply(["roots": [], "superseded": true]); return
                    }
                    self.panel = nil
                    let approvalTimeout = DispatchWorkItem { [weak self] in
                        guard let self, gate.isCurrent(version), !replied else { return }
                        _ = gate.advance()
                        self.ready = false
                        self.status = "Directory access check timed out. Review Search settings."
                        self.work { _ = store.release() }
                        reply(["roots": [], "restricted": true, "message": self.status])
                    }
                    DispatchQueue.main.asyncAfter(deadline: .now() + 5, execute: approvalTimeout)
                    self.work { [weak self] in
                        guard gate.isCurrent(version) else {
                            self?.deliver { approvalTimeout.cancel(); reply(["roots": [], "superseded": true]) }; return
                        }
                        store.save(urls, scope: scope)
                        let result = store.restore(scope)
                        if !gate.isCurrent(version) { _ = store.release() }
                        self?.deliver { [weak self] in
                            approvalTimeout.cancel()
                            guard let self, gate.isCurrent(version) else {
                                reply(["roots": [], "superseded": true]); return
                            }
                            self.finish(result, reply: reply)
                        }
                    }
                }
                if let request = self.request { request(scope, accepted); return }
                let panel = NSOpenPanel()
                self.panel = panel
                panel.title = NativeLocalization.text(.authorizeFileSearch)
                panel.message = NativeLocalization.text(.fileSearchAccessMessage)
                panel.prompt = NativeLocalization.text(.authorize)
                panel.canChooseDirectories = true
                panel.canChooseFiles = false
                panel.allowsMultipleSelection = true
                panel.canCreateDirectories = false
                panel.directoryURL = scope.first
                let delegate = FileSearchScopeDelegate(scope: scope)
                panel.delegate = delegate
                NSApp.activate(ignoringOtherApps: true)
                panel.begin { response in
                    _ = delegate
                    accepted(response == .OK ? panel.urls : [])
                }
            }
        }
    }

    private func finish(_ result: FileSearchAccessStorage.Result, reply: ([String: Any]) -> Void) {
        ready = !result.roots.isEmpty
        status = result.roots.isEmpty
            ? "File search needs directory authorization. Use Authorize directories in Search settings."
            : result.complete ? "" : "File search uses only authorized directories. Review access in Search settings to extend the range."
        if !result.saveError.isEmpty { status = [status, result.saveError].filter { !$0.isEmpty }.joined(separator: "\n") }
        reply(["roots": result.roots, "restricted": true, "message": status])
    }
}

private final class FileSearchAccessEpoch {
    private let lock = NSLock()
    private var value: UInt64 = 0
    func advance() -> UInt64 { lock.lock(); defer { lock.unlock() }; value &+= 1; return value }
    func isCurrent(_ version: UInt64) -> Bool { lock.lock(); defer { lock.unlock() }; return version == value }
}

/// Accessed exclusively on the serial worker, including lease release.
private final class FileSearchAccessStorage {
    struct Result { let roots: [String]; let complete: Bool; let saveError: String }
    private let file: URL
    private var bookmarks: [String: Data]?
    private var leases: [URL] = []
    private var saveError = ""
    init(_ file: URL) { self.file = file }
    func release() -> Result {
        leases.forEach { $0.stopAccessingSecurityScopedResource() }; leases = []
        return Result(roots: [], complete: false, saveError: "")
    }
    private func load() -> [String: Data] {
        if let bookmarks { return bookmarks }
        let decoded = (try? Data(contentsOf: file)).flatMap {
            try? PropertyListDecoder().decode([String: Data].self, from: $0)
        } ?? [:]
        bookmarks = decoded
        return decoded
    }
    func restore(_ scope: [URL]) -> Result {
        _ = release()
        var grants: [URL] = []
        for (_, data) in load() {
            var stale = false
            guard let url = try? URL(resolvingBookmarkData: data, options: [.withSecurityScope, .withoutUI, .withoutMounting],
                                     relativeTo: nil, bookmarkDataIsStale: &stale), !stale,
                  scope.contains(where: { FileSearchAccess.contains($0, url) || FileSearchAccess.contains(url, $0) }) else { continue }
            if url.startAccessingSecurityScopedResource() { leases.append(url) }
            grants.append(url)
        }
        var effective: [String] = []
        for root in scope {
            let candidates = grants.contains(where: { FileSearchAccess.contains($0, root) })
                ? [root] : grants.filter { FileSearchAccess.contains(root, $0) }
            for candidate in candidates {
                guard let directory = opendir(candidate.path) else { continue }
                closedir(directory)
                let canonical = candidate.resolvingSymlinksInPath().path
                if !effective.contains(canonical) { effective.append(canonical) }
            }
        }
        return Result(roots: Array(effective.prefix(RuntimePolicy.maxSearchRoots)),
                      complete: scope.allSatisfy { effective.contains($0.resolvingSymlinksInPath().path) }, saveError: saveError)
    }
    func save(_ urls: [URL], scope: [URL]) {
        guard !urls.isEmpty else { return }
        var updated = load().filter { path, _ in
            scope.contains { FileSearchAccess.contains($0, URL(fileURLWithPath: path)) || FileSearchAccess.contains(URL(fileURLWithPath: path), $0) }
        }
        for url in urls.prefix(RuntimePolicy.maxSearchRoots) where scope.contains(where: { FileSearchAccess.contains($0, url) }) {
            if let data = try? url.bookmarkData(options: [.withSecurityScope, .securityScopeAllowOnlyReadAccess],
                                               includingResourceValuesForKeys: nil, relativeTo: nil) { updated[url.path] = data }
        }
        var bounded: [String: Data] = [:]
        for key in urls.map({ $0.path }) + updated.keys.sorted() where bounded.count < 16 {
            if let data = updated[key] { bounded[key] = data }
        }
        bookmarks = bounded
        do { try PropertyListEncoder().encode(bounded).write(to: file, options: .atomic); saveError = "" }
        catch { saveError = "Directory access could not be saved and must be authorized again next launch." }
    }
    deinit { _ = release() }
}

private final class FileSearchScopeDelegate: NSObject, NSOpenSavePanelDelegate {
    let scope: [URL]
    init(scope: [URL]) { self.scope = scope }
    func panel(_ sender: Any, validate url: URL) throws {
        // Fast lexical validation here; canonical/symlink validation runs on worker.
        guard scope.contains(where: { url.standardizedFileURL.pathComponents.starts(with: $0.standardizedFileURL.pathComponents) }) else {
            throw NSError(domain: "Anywhere.FileSearchAccess", code: 1, userInfo: [NSLocalizedDescriptionKey:
                "Choose a folder inside the configured search range. Save a new range in Search settings to authorize other folders."])
        }
    }
}
