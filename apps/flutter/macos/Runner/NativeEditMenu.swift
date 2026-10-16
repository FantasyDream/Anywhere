import AppKit

/// Never let AppKit edit Flutter's accessibility NSTextView independently of
/// Dart's editing model. The selected Flutter engine applies these intentions.
final class NativeEditMenu: NSObject, NSMenuItemValidation {
    private let available: () -> Bool
    private let send: (String) -> Void
    private static let commands = ["undo", "redo", "cut", "copy", "paste", "selectAll"]
    init(available: @escaping () -> Bool, send: @escaping (String) -> Void) {
        self.available = available
        self.send = send
    }

    func makeMenu() -> NSMenu {
        let menu = NSMenu(title: "Edit")
        for (index, entry) in [("Undo", "z"), ("Redo", "z"), ("Cut", "x"),
                               ("Copy", "c"), ("Paste", "v"), ("Select All", "a")].enumerated() {
            let item = NSMenuItem(title: entry.0, action: #selector(performCommand(_:)), keyEquivalent: entry.1)
            item.representedObject = Self.commands[index]
            item.target = self
            item.keyEquivalentModifierMask = index == 1 ? [.command, .shift] : [.command]
            menu.addItem(item)
        }
        return menu
    }

    func validateMenuItem(_ item: NSMenuItem) -> Bool {
        guard let command = item.representedObject as? String else { return false }
        return Self.commands.contains(command) && available()
    }

    @objc func performCommand(_ item: NSMenuItem) {
        guard validateMenuItem(item), let command = item.representedObject as? String else { return }
        send(command)
    }
}
