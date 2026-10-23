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

  func makeMenu(
    title: String = "Edit",
    labels: [String] = ["Undo", "Redo", "Cut", "Copy", "Paste", "Select All"]
  ) -> NSMenu {
    precondition(labels.count == Self.commands.count)
    let menu = NSMenu(title: title)
    for (index, key) in ["z", "z", "x", "c", "v", "a"].enumerated() {
      let item = NSMenuItem(
        title: labels[index],
        action: #selector(performCommand(_:)),
        keyEquivalent: key
      )
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
