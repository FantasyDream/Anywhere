import AppKit
// Exercise the real platform executable; restore every original pasteboard type.
func checkClipboard() throws -> Int32 {
let board = NSPasteboard.general
let saved = (board.pasteboardItems ?? []).map { old -> NSPasteboardItem in
    let item = NSPasteboardItem()
    for type in old.types { if let data = old.data(forType: type) { item.setData(data, forType: type) } }
    return item
}
defer { board.clearContents(); board.writeObjects(saved) }
var failures: Int32 = 0
for locale in ["C", "en_US.UTF-8"] {
    for text in ["plain ASCII", "/Users/test/目录 A/hello world.txt", "中文 😀\nsecond line"] {
    let process = Process()
    process.executableURL = URL(fileURLWithPath: CommandLine.arguments[1])
    process.arguments = ["copy", text]
    var environment = ProcessInfo.processInfo.environment
    environment["LANG"] = locale
    environment["LC_ALL"] = locale
    process.environment = environment
    try process.run()
    process.waitUntilExit()
    let actual = board.string(forType: .string)
    let matches = actual == text
    print("locale=\(locale) exit=\(process.terminationStatus) matches=\(matches) bytes=\(actual?.utf8.count ?? 0)")
    if !matches || process.terminationStatus != 0 { failures += 1 }
    }
}
return failures
}
do { exit(try checkClipboard() == 0 ? 0 : 1) }
catch { fputs("Clipboard test failed: \(error)\n", stderr); exit(1) }
