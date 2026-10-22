import Foundation
import FlutterMacOS

/// Preference distribution to Flutter engines, Web views and native chrome.
/// No timers, filesystem polling, clipboard reads, or new engine ownership.
final class LocaleHost {
    static private(set) var preference = "system"
    static let changed = Notification.Name("AnywhereLanguageChanged")
    static func update(_ value: String?) {
        let next = ["system", "zh", "en"].contains(value ?? "") ? value! : "system"
        guard next != preference else { return }
        preference = next
        NativeLocalization.preference = next
        NotificationCenter.default.post(name: changed, object: nil)
    }
    private let channel: FlutterMethodChannel
    private var observer: NSObjectProtocol?
    init(messenger: FlutterBinaryMessenger) {
        channel = FlutterMethodChannel(name: "anywhere/locale", binaryMessenger: messenger)
        channel.setMethodCallHandler { call, reply in
            if call.method == "read" { reply(Self.preference) }
            else { reply(FlutterMethodNotImplemented) }
        }
        observer = NotificationCenter.default.addObserver(forName: Self.changed,
            object: nil, queue: .main) { [weak self] _ in
                self?.channel.invokeMethod("changed", arguments: Self.preference)
            }
    }
    deinit {
        if let observer { NotificationCenter.default.removeObserver(observer) }
        channel.setMethodCallHandler(nil)
    }
}
