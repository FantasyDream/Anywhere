import AppKit
import Foundation
import WebKit

@main
struct PresentationDiagnosticsTests {
  static func main() {
    let valid: [String: Any] = [
      "operation": "clipboard_query", "elapsed_us": 120_000, "rows": 4,
      "candidate_count": 12, "icon_cache_bytes": 4_096,
      "candidate_transfer_rows_last": 8,
      "query": "must not be exported", "body": "private clipboard body",
    ]
    precondition(PresentationDiagnostics.event(valid, visible: true, enabled: false) == nil)
    let event = PresentationDiagnostics.event(valid, visible: false, enabled: true)!
    precondition(
      Set(event.keys) == [
        "event", "operation", "elapsed_us", "rows", "native_visible",
        "candidate_count", "icon_cache_bytes", "candidate_transfer_rows_last",
      ]
    )
    precondition(event["candidate_count"] as? Int == 12)
    precondition(event["query"] == nil && event["body"] == nil)
    precondition(event["native_visible"] as? Bool == false)
    let root = NSView()
    let configuration = WKWebViewConfiguration()
    root.addSubview(WKWebView(frame: .zero, configuration: configuration))
    let surfaces = PresentationDiagnostics.nativeSurfaceCounts(in: [root])
    precondition(surfaces["native_web_view_count"] == 1)
    precondition(surfaces["native_user_content_controller_count"] == 1)
    precondition(surfaces["native_website_data_store_count"] == 1)
    for (key, value) in [
      ("operation", "unknown" as Any), ("elapsed_us", -1 as Any),
      ("elapsed_us", "1" as Any), ("rows", 1_000_001 as Any),
    ] {
      var invalid = valid
      invalid[key] = value
      precondition(PresentationDiagnostics.event(invalid, visible: true, enabled: true) == nil)
    }
    for operation in [
      "activation", "query", "file_query", "file_page_next",
      "file_page_previous", "clipboard_query", "shelf_query",
    ] {
      var args = valid
      args["operation"] = operation
      precondition(PresentationDiagnostics.event(args, visible: true, enabled: true) != nil)
    }
  }
}
