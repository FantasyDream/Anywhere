import FlutterMacOS
import Foundation

/// One process-level pressure source per Flutter engine. The channel carries a
/// severity only; candidates, documents and clipboard bodies never cross it.
final class MemoryPressureHost {
  private let channel: FlutterMethodChannel
  private var source: DispatchSourceMemoryPressure?

  init(messenger: FlutterBinaryMessenger) {
    channel = FlutterMethodChannel(
      name: "anywhere/memory-pressure",
      binaryMessenger: messenger
    )
    let pressure = DispatchSource.makeMemoryPressureSource(
      eventMask: [.warning, .critical],
      queue: .main
    )
    pressure.setEventHandler { [weak self, weak pressure] in
      guard let self, let pressure else { return }
      let level = pressure.data.contains(.critical) ? "critical" : "warning"
      self.channel.invokeMethod("memoryPressure", arguments: level)
    }
    pressure.resume()
    source = pressure
  }

  deinit {
    source?.setEventHandler {}
    source?.cancel()
    channel.setMethodCallHandler(nil)
  }
}
