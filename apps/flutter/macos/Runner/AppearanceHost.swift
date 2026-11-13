import AppKit
import FlutterMacOS
import Foundation

struct HostAppearanceState {
  let brightness: String
  let increasedContrast: Bool
  let reduceTransparency: Bool
  let reduceMotion: Bool
  let accentColor: Int
  let systemColors: [String: Int]
}

enum HostSystemAppearance {
  static func read(_ appearance: NSAppearance) -> HostAppearanceState {
    let workspace = NSWorkspace.shared
    return HostAppearanceState(
      brightness: appearance.bestMatch(from: [.aqua, .darkAqua]) == .darkAqua ? "dark" : "light",
      increasedContrast: workspace.accessibilityDisplayShouldIncreaseContrast,
      reduceTransparency: workspace.accessibilityDisplayShouldReduceTransparency,
      reduceMotion: workspace.accessibilityDisplayShouldReduceMotion,
      accentColor: argb(NSColor.controlAccentColor),
      systemColors: [
        "windowBackground": argb(NSColor.windowBackgroundColor),
        "label": argb(NSColor.labelColor),
        "secondaryLabel": argb(NSColor.secondaryLabelColor),
        "selectedContentBackground": argb(NSColor.selectedContentBackgroundColor),
        "keyboardFocusIndicator": argb(NSColor.keyboardFocusIndicatorColor),
      ]
    )
  }

  private static func argb(_ color: NSColor) -> Int {
    guard let rgb = color.usingColorSpace(.sRGB) else { return 0 }
    let alpha = Int((rgb.alphaComponent * 255).rounded())
    let red = Int((rgb.redComponent * 255).rounded())
    let green = Int((rgb.greenComponent * 255).rounded())
    let blue = Int((rgb.blueComponent * 255).rounded())
    return (alpha << 24) | (red << 16) | (green << 8) | blue
  }
}

/// One versioned appearance source for one Flutter engine's native window.
final class AppearanceHost {
  let channel: FlutterMethodChannel
  let sourceId = UUID().uuidString.lowercased()
  let windowToken = UUID().uuidString.lowercased()
  private weak var window: NSWindow?
  private weak var material: WindowMaterial?
  private let readSystem: (NSAppearance) -> HostAppearanceState
  private var observers: [NSObjectProtocol] = []
  private var revision = 0
  private var publishedSignature: String?
  private var publishScheduled = false

  init(
    messenger: FlutterBinaryMessenger,
    window: NSWindow,
    material: WindowMaterial,
    readSystem: @escaping (NSAppearance) -> HostAppearanceState = HostSystemAppearance.read
  ) {
    self.window = window
    self.material = material
    self.readSystem = readSystem
    channel = FlutterMethodChannel(name: "anywhere/appearance", binaryMessenger: messenger)
    material.onEffectiveAppearanceChanged = { [weak self] in self?.schedulePublish() }
    observers.append(
      NSWorkspace.shared.notificationCenter.addObserver(
        forName: NSWorkspace.accessibilityDisplayOptionsDidChangeNotification,
        object: nil,
        queue: .main
      ) { [weak self] _ in self?.schedulePublish() }
    )
    observers.append(
      NotificationCenter.default.addObserver(
        forName: NSColor.systemColorsDidChangeNotification,
        object: nil,
        queue: .main
      ) { [weak self] _ in self?.schedulePublish() }
    )
    observers.append(
      NotificationCenter.default.addObserver(
        forName: NSApplication.didBecomeActiveNotification,
        object: nil,
        queue: .main
      ) { [weak self] _ in self?.schedulePublish() }
    )
    channel.setMethodCallHandler { [weak self] call, reply in
      guard let self else {
        reply(nil)
        return
      }
      switch call.method {
      case "watchAppearance":
        reply(self.currentSnapshot())
      case "refreshAppearance":
        self.refresh(call.arguments, reply: reply)
      default:
        reply(FlutterMethodNotImplemented)
      }
    }
  }

  deinit {
    channel.setMethodCallHandler(nil)
    material?.onEffectiveAppearanceChanged = nil
    for observer in observers {
      NSWorkspace.shared.notificationCenter.removeObserver(observer)
      NotificationCenter.default.removeObserver(observer)
    }
  }

  private func refresh(_ arguments: Any?, reply: @escaping FlutterReply) {
    guard let args = arguments as? [String: Any],
      args["windowToken"] as? String == windowToken,
      args["sourceId"] as? String == sourceId
    else {
      reply(
        FlutterError(code: "staleAppearance", message: "Appearance source changed.", details: nil)
      )
      return
    }
    let snapshot = currentSnapshot()
    let after = args["afterRevision"] as? Int ?? -1
    if after < revision {
      schedulePublish()
    }
    reply(snapshot)
  }

  private func currentSnapshot() -> [String: Any] {
    let appearance =
      window?.effectiveAppearance ?? material?.effectiveAppearance ?? NSApp.effectiveAppearance
    let system = readSystem(appearance)
    let materialAvailable = material?.usesSystemMaterial == true
    let surface = materialAvailable && !system.reduceTransparency ? "systemManaged" : "opaque"
    let fallbackReason: String? =
      if !materialAvailable {
        "nativeSurfaceUnavailable"
      } else if system.reduceTransparency {
        "reduceTransparency"
      } else {
        nil
      }
    let signature = [
      system.brightness,
      String(system.increasedContrast),
      String(system.reduceTransparency),
      String(system.reduceMotion),
      String(system.accentColor),
      system.systemColors.sorted { $0.key < $1.key }.map { "\($0.key)=\($0.value)" }.joined(
        separator: ","
      ),
      surface,
      fallbackReason ?? "",
    ].joined(separator: "|")
    if signature != publishedSignature || revision == 0 {
      revision += 1
      publishedSignature = signature
    }
    return [
      "sourceId": sourceId,
      "windowToken": windowToken,
      "revision": revision,
      "brightness": system.brightness,
      "contrast": system.increasedContrast ? "increased" : "standard",
      "reduceTransparency": system.reduceTransparency,
      "reduceMotion": system.reduceMotion,
      "surface": surface,
      "accentColor": system.accentColor,
      "systemColors": system.systemColors,
      "fallbackReason": fallbackReason as Any,
      "capabilities": [
        "protocolVersion": 1,
        "systemSurface": materialAvailable,
        "brightness": true,
        "contrast": true,
        "reduceTransparency": true,
        "reduceMotion": true,
        "systemColors": true,
      ],
    ]
  }

  private func schedulePublish() {
    guard !publishScheduled else { return }
    publishScheduled = true
    DispatchQueue.main.async { [weak self] in
      guard let self else { return }
      self.publishScheduled = false
      let before = self.revision
      let snapshot = self.currentSnapshot()
      if self.revision != before {
        self.channel.invokeMethod("appearanceChanged", arguments: snapshot)
      }
    }
  }

  /// Re-read on presentation recovery even when NSApplication was already active.
  func publishIfChanged() {
    schedulePublish()
  }
}
