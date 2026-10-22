import AppKit

/// Only native packaging and appearance information crosses this boundary.
enum HostRuntime {
    static var libraryPath: String {
        Bundle.main.bundleURL.appendingPathComponent("Contents/Frameworks/libanywhere_bridge_flutter.dylib").path
    }
    static var capabilities: [String: Any] {
        ["backdrop": !NSWorkspace.shared.accessibilityDisplayShouldReduceTransparency,
         "shortcutRecording": true, "fileSearchAccess": true]
    }
}
