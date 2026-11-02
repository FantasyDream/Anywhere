import Foundation

@main
struct PresentationDiagnosticsTests {
  static func main() {
    let valid: [String: Any] = [
      "operation": "clipboard_query", "elapsed_us": 120_000, "rows": 4,
      "query": "must not be exported", "body": "private clipboard body",
    ]
    precondition(PresentationDiagnostics.event(valid, visible: true, enabled: false) == nil)
    let event = PresentationDiagnostics.event(valid, visible: false, enabled: true)!
    precondition(
      Set(event.keys) == ["event", "operation", "elapsed_us", "rows", "native_visible"]
    )
    precondition(event["native_visible"] as? Bool == false)
    for (key, value) in [
      ("operation", "unknown" as Any), ("elapsed_us", -1 as Any),
      ("elapsed_us", "1" as Any), ("rows", 1_000_001 as Any),
    ] {
      var invalid = valid
      invalid[key] = value
      precondition(PresentationDiagnostics.event(invalid, visible: true, enabled: true) == nil)
    }
    for operation in ["activation", "query", "file_query", "clipboard_query", "shelf_query"] {
      var args = valid
      args["operation"] = operation
      precondition(PresentationDiagnostics.event(args, visible: true, enabled: true) != nil)
    }
  }
}
