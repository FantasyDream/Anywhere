import AppKit

@main
struct NativeEditMenuTests {
    @MainActor static func main() {
        _ = NSApplication.shared
        var active = true
        var received: [String] = []
        let bridge = NativeEditMenu(available: { active }, send: { received.append($0) })
        let menu = bridge.makeMenu()
        assert(menu.items.count == 6)
        for item in menu.items {
            assert(item.target === bridge)
            assert(bridge.validateMenuItem(item))
            bridge.performCommand(item)
        }
        assert(received == ["undo", "redo", "cut", "copy", "paste", "selectAll"])
        assert(menu.items[1].keyEquivalent == "z")
        assert(menu.items[1].keyEquivalentModifierMask == [.command, .shift])
        assert(menu.items[4].keyEquivalent == "v")
        active = false
        assert(!bridge.validateMenuItem(menu.items[4]))
        bridge.performCommand(menu.items[4])
        active = true
        let invalid = NSMenuItem()
        invalid.representedObject = "unknown"
        bridge.performCommand(invalid)
        assert(received.count == 6)
        print("Native edit menu: command mapping, shortcuts, missing destination and unknown command passed")
    }
}
