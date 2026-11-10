import AppKit
import FlutterMacOS

/// Owns the recording channel and its outstanding reply for the shared engine.
final class ShortcutRecordingHost {
  private let channel: FlutterMethodChannel
  private let recorder = ShortcutRecorder()
  private let available: () -> Bool

  init(messenger: FlutterBinaryMessenger, available: @escaping () -> Bool) {
    self.available = available
    channel = FlutterMethodChannel(name: "anywhere/shortcut-recorder", binaryMessenger: messenger)
    channel.setMethodCallHandler { [weak self] call, reply in
      guard let self, let args = call.arguments as? [String: Any],
        let id = args["id"] as? Int
      else {
        reply(nil)
        return
      }
      switch call.method {
      case "record":
        guard self.available() else {
          reply(
            FlutterError(
              code: "recording_unavailable",
              message: "Open Settings to record a shortcut.",
              details: nil
            )
          )
          return
        }
        self.recorder.start(id: id) { reply($0) }
      case "cancel":
        self.recorder.cancel(id: id)
        reply(nil)
      default: reply(FlutterMethodNotImplemented)
      }
    }
  }

  var isRecording: Bool { recorder.isRecording }
  func cancel() { recorder.cancel() }
  func handle(code: UInt16, flags: NSEvent.ModifierFlags, repeatKey: Bool) -> Bool {
    recorder.handle(code: code, flags: flags, repeatKey: repeatKey)
  }
  func accept(code: UInt32, modifiers: UInt32) -> Bool {
    recorder.accept(code: code, modifiers: modifiers)
  }

  deinit {
    channel.setMethodCallHandler(nil)
    recorder.cancel()
  }
}
