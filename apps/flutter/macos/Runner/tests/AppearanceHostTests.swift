import AppKit
import FlutterMacOS
import Foundation

private func verify(_ condition: @autoclosure () -> Bool, line: UInt = #line) {
  if !condition() {
    fputs("Appearance Host failed at line \(line)\n", stderr)
    exit(EXIT_FAILURE)
  }
}

private final class MutableAppearance {
  var state = HostAppearanceState(
    brightness: "dark",
    increasedContrast: false,
    reduceTransparency: false,
    reduceMotion: false,
    accentColor: 0xff33_66cc,
    systemColors: ["windowBackground": 0xff20_2020]
  )
}

@main
struct AppearanceHostTests {
  @MainActor static func main() {
    _ = NSApplication.shared
    let frame = NSRect(x: 0, y: 0, width: 648, height: 300)
    let window = NSWindow(
      contentRect: frame,
      styleMask: [.borderless],
      backing: .buffered,
      defer: false
    )
    let material = WindowMaterial(
      frame: frame,
      content: NSView(frame: frame),
      cornerRadius: 18,
      preferLiquidGlass: false
    )
    window.contentView = material
    let messenger = HostChannelTestMessenger()
    let mutable = MutableAppearance()
    let host = AppearanceHost(
      messenger: messenger,
      window: window,
      material: material,
      readSystem: { _ in mutable.state }
    )
    let codec = FlutterStandardMethodCodec.sharedInstance()

    let initialReply = messenger.request("anywhere/appearance", "watchAppearance")
    verify(initialReply.responses.count == 1 && initialReply.responses[0] != nil)
    let initial = codec.decodeEnvelope(initialReply.responses[0]!) as! [String: Any]
    verify(initial["sourceId"] as? String == host.sourceId)
    verify(initial["windowToken"] as? String == host.windowToken)
    verify(initial["revision"] as? Int == 1)
    verify(initial["brightness"] as? String == "dark")
    verify(initial["surface"] as? String == "systemManaged")
    let capabilities = initial["capabilities"] as! [String: Any]
    verify(capabilities["protocolVersion"] as? Int == 1)
    verify(capabilities["systemSurface"] as? Bool == true)

    mutable.state = HostAppearanceState(
      brightness: "light",
      increasedContrast: true,
      reduceTransparency: true,
      reduceMotion: true,
      accentColor: 0xffcc_6633,
      systemColors: ["windowBackground": 0xfff0_f0f0]
    )
    material.onEffectiveAppearanceChanged?()
    RunLoop.current.run(until: Date(timeIntervalSinceNow: 0.05))
    verify(messenger.events.count == 1)
    let event = messenger.events[0]
    verify(event.0 == "anywhere/appearance")
    verify(event.1.method == "appearanceChanged")
    let changed = event.1.arguments as! [String: Any]
    verify(changed["revision"] as? Int == 2)
    verify(changed["brightness"] as? String == "light")
    verify(changed["contrast"] as? String == "increased")
    verify(changed["reduceTransparency"] as? Bool == true)
    verify(changed["reduceMotion"] as? Bool == true)
    verify(changed["surface"] as? String == "opaque")
    verify(changed["fallbackReason"] as? String == "reduceTransparency")

    let refreshed = messenger.request(
      "anywhere/appearance",
      "refreshAppearance",
      [
        "windowToken": host.windowToken,
        "sourceId": host.sourceId,
        "afterRevision": 1,
      ]
    )
    verify(refreshed.responses.count == 1 && refreshed.responses[0] != nil)
    let refreshBody = codec.decodeEnvelope(refreshed.responses[0]!) as! [String: Any]
    verify(refreshBody["revision"] as? Int == 2)

    let stale = messenger.request(
      "anywhere/appearance",
      "refreshAppearance",
      [
        "windowToken": "old-window",
        "sourceId": host.sourceId,
        "afterRevision": 2,
      ]
    )
    verify(stale.responses.count == 1 && stale.responses[0] != nil)
    let error = codec.decodeEnvelope(stale.responses[0]!) as? FlutterError
    verify(error?.code == "staleAppearance")
    withExtendedLifetime(host) {}
    print("Appearance Host: snapshot, capability, event, refresh and stale-source guards passed")
  }
}
