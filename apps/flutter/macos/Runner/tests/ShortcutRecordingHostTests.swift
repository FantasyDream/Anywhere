import AppKit
import Carbon
import Darwin
import FlutterMacOS

private func verify(_ condition: @autoclosure () -> Bool, line: UInt = #line) {
  if !condition() {
    fputs("Shortcut recording channel failed at line \(line)\n", stderr)
    exit(EXIT_FAILURE)
  }
}

@main
struct ShortcutRecordingHostTests {
  @MainActor static func main() {
    let messenger = HostChannelTestMessenger()
    let channel = "anywhere/shortcut-recorder"
    let codec = FlutterStandardMethodCodec.sharedInstance()
    let cancelled = codec.encodeSuccessEnvelope(nil)
    var available = false
    var host: ShortcutRecordingHost? = ShortcutRecordingHost(
      messenger: messenger,
      available: { available }
    )
    verify(messenger.request(channel, "record", ["id": "bad"]).responses == [cancelled])
    verify(messenger.request(channel, "unknown", ["id": 1]).responses == [nil])
    let rejected = messenger.request(channel, "record", ["id": 1])
    verify(
      rejected.responses == [
        codec.encodeErrorEnvelope(
          FlutterError(
            code: "recording_unavailable",
            message: "Open Settings to record a shortcut.",
            details: nil
          )
        )
      ]
    )
    verify(host?.isRecording == false)
    available = true
    let first = messenger.request(channel, "record", ["id": 2])
    verify(first.responses.isEmpty && host?.isRecording == true)
    let second = messenger.request(channel, "record", ["id": 3])
    verify(first.responses == [cancelled] && second.responses.isEmpty)
    verify(messenger.request(channel, "cancel", ["id": 2]).responses == [cancelled])
    verify(second.responses.isEmpty && host?.isRecording == true)
    verify(host?.handle(code: 17, flags: [.control, .option], repeatKey: true) == true)
    verify(second.responses.isEmpty)
    verify(host?.handle(code: 17, flags: [.control, .option], repeatKey: false) == true)
    verify(second.responses.count == 1 && second.responses[0] != nil)
    verify(
      codec.decodeEnvelope(second.responses[0]!) as? [String: Int] == [
        "keyCode": 23, "modifiers": 3,
      ]
    )
    verify(host?.isRecording == false)
    let wake = messenger.request(channel, "record", ["id": 4])
    verify(host?.accept(code: 49, modifiers: UInt32(cmdKey | optionKey)) == true)
    verify(wake.responses.count == 1 && wake.responses[0] != nil)
    verify(
      codec.decodeEnvelope(wake.responses[0]!) as? [String: Int] == [
        "keyCode": 44, "modifiers": 10,
      ]
    )
    let focusLoss = messenger.request(channel, "record", ["id": 5])
    host?.cancel()
    host?.cancel()
    verify(focusLoss.responses == [cancelled])
    let pending = messenger.request(channel, "record", ["id": 6])
    let stale = messenger.handlers[channel]
    let wasReleased = { [weak host] in host == nil }
    host = nil
    verify(wasReleased() && messenger.handlers[channel] == nil)
    verify(pending.responses == [cancelled])
    var replies: [Data?] = []
    stale?(codec.encode(FlutterMethodCall(methodName: "record", arguments: ["id": 7]))) {
      replies.append($0)
    }
    verify(replies == [cancelled])
    print(
      "Shortcut recording channel: eligibility, replacement, stale cancel, keys and release passed"
    )
  }
}
