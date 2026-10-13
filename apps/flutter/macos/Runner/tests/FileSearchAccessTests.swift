import AppKit

@main
struct FileSearchAccessTests {
    static func main() throws {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString, isDirectory: true)
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: root) }
        let first = root.appendingPathComponent("first", isDirectory: true)
        let second = root.appendingPathComponent("second", isDirectory: true)
        try FileManager.default.createDirectory(at: first, withIntermediateDirectories: true)
        try FileManager.default.createDirectory(at: second, withIntermediateDirectories: true)
        assert(FileSearchAccess.contains(first, first.appendingPathComponent("child")))
        assert(!FileSearchAccess.contains(first, root.appendingPathComponent("first-other")))
        assert(!FileSearchAccess.contains(first, first.appendingPathComponent("../second")))
        let link = first.appendingPathComponent("outside-link")
        try FileManager.default.createSymbolicLink(at: link, withDestinationURL: second)
        assert(!FileSearchAccess.contains(first, link), "A link cannot widen authorization")
        var requests = 0
        let storage = root.appendingPathComponent("grants.plist")
        let access = FileSearchAccess(storage: storage, request: { _, reply in requests += 1; reply([first]) }, work: { $0() })
        var result: [String: Any] = [:]
        access.prepare(enabled: true, roots: [first.path]) { result = $0 }
        assert(requests == 1 && result["roots"] as? [String] == [first.path])
        access.prepare(enabled: true, roots: [first.path]) { result = $0 }
        assert(requests == 1)
        let restored = FileSearchAccess(storage: storage, request: { _, _ in fatalError("Restored grant must not prompt") }, work: { $0() })
        restored.prepare(enabled: true, roots: [first.path]) { result = $0 }
        assert(result["roots"] as? [String] == [first.path])
        // Existing authorization cannot silently widen to a sibling scope.
        access.prepare(enabled: true, roots: [second.path]) { result = $0 }
        assert(requests == 2 && result["roots"] as? [String] == [])
        access.prepare(enabled: true, roots: [second.path]) { result = $0 }
        assert(requests == 2, "Do not re-prompt for the same denied scope during settings saves")
        access.prepare(enabled: false, roots: [first.path]) { result = $0 }
        assert(result["roots"] as? [String] == [])
        let declined = FileSearchAccess(storage: root.appendingPathComponent("none.plist"), request: { _, reply in reply([]) }, work: { $0() })
        declined.prepare(enabled: true, roots: [first.path]) { result = $0 }
        assert(result["roots"] as? [String] == [] && !declined.status.isEmpty)
        var late: (([URL]) -> Void)?
        let raceFile = root.appendingPathComponent("race.plist")
        let racing = FileSearchAccess(storage: raceFile, request: { _, callback in late = callback }, work: { $0() })
        var superseded = false
        racing.prepare(enabled: true, roots: [first.path]) { superseded = $0["superseded"] as? Bool == true }
        racing.prepare(enabled: false, roots: [second.path]) { result = $0 }
        late!([first])
        assert(superseded && !racing.ready && racing.status.isEmpty)
        assert(!FileManager.default.fileExists(atPath: raceFile.path), "Obsolete approval must not save bookmarks")
        // Slow worker admission never blocks the caller; superseded queued work is skipped.
        var queued: [() -> Void] = []
        let asynchronous = FileSearchAccess(storage: raceFile, request: { _, _ in fatalError("Old work must not prompt") }, work: { queued.append($0) })
        asynchronous.prepare(enabled: true, roots: [first.path]) { result = $0 }
        asynchronous.prepare(enabled: false, roots: [second.path]) { result = $0 }
        assert(queued.count == 2 && !asynchronous.ready)
        queued.forEach { $0() }
        assert(!asynchronous.ready && result["roots"] as? [String] == [])
        print("File search access: scope boundaries, restore, limited grants, cancellation and prompt deduplication passed")
    }
}
