import Foundation

/// Opt-in, content-free samples shared by the Launcher and lazy tool windows.
enum PresentationDiagnostics {
  static var enabled: Bool {
    ProcessInfo.processInfo.environment["ANYWHERE_PROFILE_UI"] == "1"
  }

  static func event(
    _ arguments: Any?,
    visible: Bool,
    enabled: Bool = PresentationDiagnostics.enabled
  ) -> [String: Any]? {
    guard enabled, let args = arguments as? [String: Any],
      let operation = args["operation"] as? String,
      ["activation", "query", "file_query", "clipboard_query", "shelf_query"].contains(operation),
      let elapsed = args["elapsed_us"] as? Int, elapsed >= 0,
      let rows = args["rows"] as? Int, (0...1_000_000).contains(rows)
    else { return nil }
    return [
      "event": "ui_performance", "operation": operation, "elapsed_us": elapsed,
      "rows": rows, "native_visible": visible,
    ]
  }
}
