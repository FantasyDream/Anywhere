import AppKit
import Foundation
import WebKit

/// Opt-in, content-free samples shared by the Launcher and lazy tool windows.
enum PresentationDiagnostics {
  private static let numericKeys: Set<String> = [
    "candidate_count", "candidate_chunk_count", "result_action_count",
    "context_candidate_count", "retained_session", "icon_cache_entries",
    "icon_cache_bytes", "icon_cache_pending", "icon_cache_active",
    "icon_cache_queued", "icon_cache_failed", "flutter_engine_count",
    "image_cache_entries", "image_cache_bytes", "image_cache_live",
    "image_cache_pending", "candidate_transfer_batches",
    "candidate_transfer_rows_total", "candidate_transfer_rows_last",
    "candidate_transfer_kind_last", "clipboard_item_count",
    "native_preview_open", "web_document_count",
    "native_web_view_count", "native_user_content_controller_count",
    "native_website_data_store_count",
  ]

  static var enabled: Bool {
    ProcessInfo.processInfo.environment["ANYWHERE_PROFILE_UI"] == "1"
  }

  /// Counts object identities reachable from app-owned native view trees.
  /// WebKit internals and system panels remain outside this attribution.
  static func nativeSurfaceCounts(in roots: [NSView]) -> [String: Int] {
    var pending = roots
    var webViews: [WKWebView] = []
    while let view = pending.popLast() {
      if let web = view as? WKWebView {
        webViews.append(web)
      } else {
        pending.append(contentsOf: view.subviews)
      }
    }
    let controllers = Set(
      webViews.map { ObjectIdentifier($0.configuration.userContentController) }
    )
    let dataStores = Set(
      webViews.map { ObjectIdentifier($0.configuration.websiteDataStore) }
    )
    return [
      "native_web_view_count": webViews.count,
      "native_user_content_controller_count": controllers.count,
      "native_website_data_store_count": dataStores.count,
    ]
  }

  static func event(
    _ arguments: Any?,
    visible: Bool,
    enabled: Bool = PresentationDiagnostics.enabled
  ) -> [String: Any]? {
    guard enabled, let args = arguments as? [String: Any],
      let operation = args["operation"] as? String,
      [
        "activation", "query", "file_query", "file_page_next",
        "file_page_previous", "clipboard_query", "shelf_query",
      ].contains(operation),
      let elapsed = args["elapsed_us"] as? Int, elapsed >= 0,
      let rows = args["rows"] as? Int, (0...1_000_000).contains(rows)
    else { return nil }
    var event: [String: Any] = [
      "event": "ui_performance", "operation": operation, "elapsed_us": elapsed,
      "rows": rows, "native_visible": visible,
    ]
    for key in numericKeys {
      guard let value = args[key] else { continue }
      if let number = value as? Int, (0...1_000_000_000).contains(number) {
        event[key] = number
      }
    }
    return event
  }
}
