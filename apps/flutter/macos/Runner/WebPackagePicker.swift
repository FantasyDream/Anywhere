import AppKit
import UniformTypeIdentifiers

/// User-initiated selection only; this is not a filesystem capability for plugin JS.
final class WebPackagePicker {
  private var panel: NSOpenPanel?
  func choose(in window: NSWindow, completion: @escaping (String?) -> Void) {
    guard panel == nil else {
      completion(nil)
      return
    }
    let picker = NSOpenPanel()
    picker.title = NativeLocalization.text(.importLocalWebTool)
    picker.message = NativeLocalization.text(.inspectTrustedPackage)
    picker.canChooseFiles = true
    picker.canChooseDirectories = false
    picker.allowsMultipleSelection = false
    picker.allowedContentTypes = [UTType(filenameExtension: "awplugin") ?? .data]
    panel = picker
    picker.beginSheetModal(for: window) { [weak self] result in
      self?.panel = nil
      completion(result == .OK ? picker.url?.path : nil)
    }
  }
}
