import AppKit

@main
struct NativePreviewKeysTests {
  static func main() {
    _ = NSApplication.shared
    func event(_ code: UInt16, _ flags: NSEvent.ModifierFlags = []) -> NSEvent {
      NSEvent.keyEvent(
        with: .keyDown,
        location: .zero,
        modifierFlags: flags,
        timestamp: 0,
        windowNumber: 0,
        context: nil,
        characters: "",
        charactersIgnoringModifiers: "",
        isARepeat: false,
        keyCode: code
      )!
    }
    assert(NativePreviewHost.action(for: event(49)) == "close")
    assert(NativePreviewHost.action(for: event(53)) == "close")
    assert(NativePreviewHost.action(for: event(125)) == "next")
    assert(NativePreviewHost.action(for: event(126)) == "previous")
    assert(NativePreviewHost.action(for: event(36)) == "activate")
    assert(NativePreviewHost.action(for: event(37, .command)) == "search")
    assert(NativePreviewHost.action(for: event(16, .command)) == "close")
    for flags: NSEvent.ModifierFlags in [.command, .option, .control, .shift, [.command, .option]] {
      for code: UInt16 in [49, 53, 36, 76, 125, 126] {
        assert(
          NativePreviewHost.action(for: event(code, flags)) == nil,
          "Preview must preserve modified keys, including global wake shortcuts"
        )
      }
    }
    assert(NativePreviewHost.action(for: event(37, [.command, .shift])) == nil)
    print("Native preview keys: plain navigation and modified shortcut pass-through passed")
  }
}
