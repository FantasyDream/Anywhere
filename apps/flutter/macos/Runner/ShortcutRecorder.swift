import AppKit
import Carbon

/// One active settings recording. Uses the same physical key codes as Carbon;
/// never observes text or keys outside the recording window.
final class ShortcutRecorder {
  private var request: (id: Int, complete: ([String: Int]?) -> Void)?
  var isRecording: Bool { request != nil }

  func start(id: Int, complete: @escaping ([String: Int]?) -> Void) {
    cancel()
    request = (id, complete)
  }

  func cancel(id: Int? = nil) {
    guard let current = request, id == nil || id == current.id else { return }
    request = nil
    current.complete(nil)
  }

  /// Called only for keyDown events in the active Settings window.
  func handle(code: UInt16, flags: NSEvent.ModifierFlags, repeatKey: Bool) -> Bool {
    guard isRecording else { return false }
    if repeatKey { return true }
    if code == UInt16(kVK_Escape) {
      cancel()
      return true
    }
    guard NativeHotKeyMap.keys[UInt32(code)] != nil else { return true }
    let mask =
      (flags.contains(.command) ? cmdKey : 0)
      | (flags.contains(.option) ? optionKey : 0)
      | (flags.contains(.control) ? controlKey : 0)
      | (flags.contains(.shift) ? shiftKey : 0)
    return accept(code: UInt32(code), modifiers: UInt32(mask))
  }

  /// Registered wake keys arrive via Carbon instead of local keyDown.
  func accept(code: UInt32, modifiers: UInt32) -> Bool {
    guard let current = request else { return false }
    request = nil
    guard let usage = NativeHotKeyMap.portableKeys[code] else {
      current.complete(nil)
      return true
    }
    current.complete([
      "keyCode": Int(usage), "modifiers": Int(NativeHotKeyMap.portableMask(modifiers)),
    ])
    return true
  }
}
