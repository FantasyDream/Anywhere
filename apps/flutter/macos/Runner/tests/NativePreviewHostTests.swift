import AppKit
import FlutterMacOS
import QuickLookUI

private final class PreviewMessenger: NSObject, FlutterBinaryMessenger {
  var handler: FlutterBinaryMessageHandler?
  var events: [FlutterMethodCall] = []
  private let codec = FlutterStandardMethodCodec.sharedInstance()

  func send(onChannel channel: String, message: Data?) {
    if let message { events.append(codec.decodeMethodCall(message)) }
  }
  func send(onChannel channel: String, message: Data?, binaryReply callback: FlutterBinaryReply?) {
    send(onChannel: channel, message: message)
    callback?(nil)
  }
  func setMessageHandlerOnChannel(
    _ channel: String,
    binaryMessageHandler handler: FlutterBinaryMessageHandler?
  ) -> FlutterBinaryMessengerConnection {
    self.handler = handler
    return 1
  }
  func cleanUpConnection(_ connection: FlutterBinaryMessengerConnection) {}

  func show(_ id: String, generation: Int, path: String) {
    let call = FlutterMethodCall(
      methodName: "show",
      arguments: [
        "generation": generation, "id": id, "paths": [path], "title": id,
      ]
    )
    var replied = false
    handler?(codec.encode(call)) { reply in
      assert(reply == self.codec.encodeSuccessEnvelope(nil), "Preview show failed: \(id)")
      replied = true
    }
    assert(replied)
    assert(events.last?.method == "shown")
    assert((events.last?.arguments as? [String: Any])?["id"] as? String == id)
  }
}

private final class PreviewKeyboardReceiver: NSView {
  override var acceptsFirstResponder: Bool { true }
}

@main
struct NativePreviewHostTests {
  @MainActor static func main() {
    let app = NSApplication.shared
    app.setActivationPolicy(.regular)
    let window = NSWindow(
      contentRect: NSRect(x: 100, y: 100, width: 400, height: 160),
      styleMask: [.titled],
      backing: .buffered,
      defer: false
    )
    window.isReleasedWhenClosed = false
    let root = NSView(frame: window.contentView!.bounds)
    let receiver = PreviewKeyboardReceiver(frame: root.bounds)
    let editor = NSTextView(frame: root.bounds)
    root.addSubview(receiver)
    root.addSubview(editor)
    let content = NSViewController()
    content.view = root
    window.contentViewController = content
    window.makeFirstResponder(editor)
    let originalResponder = window.nextResponder
    let messenger = PreviewMessenger()
    var host: NativePreviewHost? = NativePreviewHost(messenger: messenger, owner: { window })
    let hostWasReleased = { [weak host] in host == nil }
    let directory = FileManager.default.temporaryDirectory
      .appendingPathComponent("anywhere-preview-test-\(UUID().uuidString)", isDirectory: true)
    try! FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
    let file = directory.appendingPathComponent("preview.txt")
    try! "Synthetic preview regression".write(to: file, atomically: true, encoding: .utf8)

    DispatchQueue.main.asyncAfter(deadline: .now() + 0.3) {
      app.activate(ignoringOtherApps: true)
      window.makeKeyAndOrderFront(nil)
    }
    DispatchQueue.main.asyncAfter(deadline: .now() + 1) {
      assert(app.isActive && window.isKeyWindow, "Live preview test requires an active desktop")
      autoreleasepool {
        let panel = QLPreviewPanel.shared()!
        // NSTextView is a competing Quick Look controller, including Flutter's
        // native editor. The host must keep it from claiming our list preview.
        assert(editor.acceptsPreviewPanelControl(panel))
        for index in 1...3 {
          assert(window.firstResponder === editor)
          messenger.show("preview-\(index)", generation: index, path: file.path)
          assert(window.firstResponder === receiver)
          assert(panel.dataSource === host)
          assert(panel.currentPreviewItem?.previewItemURL == file)
          RunLoop.current.run(until: Date().addingTimeInterval(0.35))
          assert(panel.isVisible && host!.ownsPanel)
          if index == 3 {
            panel.makeKeyAndOrderFront(nil)
            assert(host!.returnToOwner())
            assert(window.isKeyWindow)
            assert(!host!.returnToOwner())
          } else {
            host!.close(restoreFocus: index != 2)
          }
          assert(!panel.isVisible)
          assert(window.firstResponder === editor)
          assert(window.nextResponder === originalResponder)
          assert(panel.dataSource == nil && panel.delegate == nil)
          // Deliberately reopen immediately: no delay may hide a late close.
        }
        messenger.show("replacement", generation: 4, path: file.path)
        messenger.show("updated", generation: 5, path: file.path)
        assert(panel.currentPreviewItem?.previewItemTitle == "updated")
        assert(window.firstResponder === receiver)
        let wake = NSEvent.keyEvent(
          with: .keyDown,
          location: .zero,
          modifierFlags: .command,
          timestamp: 0,
          windowNumber: panel.windowNumber,
          context: nil,
          characters: " ",
          charactersIgnoringModifiers: " ",
          isARepeat: false,
          keyCode: 49
        )!
        assert(!host!.previewPanel(panel, handle: wake), "Preview must not consume Command Space")
        assert(panel.isVisible && host!.ownsPanel)
        // The system close button follows Quick Look's own close lifecycle.
        panel.close()
        RunLoop.current.run(until: Date().addingTimeInterval(0.5))
        assert(!host!.ownsPanel && !panel.isVisible)
        assert(window.firstResponder === editor)
        messenger.show("after-native-close", generation: 6, path: file.path)
        RunLoop.current.run(until: Date().addingTimeInterval(0.35))
        assert(panel.isVisible)
        host!.close()
        assert(messenger.events.filter { $0.method == "closed" }.count == 5)
        messenger.show("hidden-owner", generation: 7, path: file.path)
        panel.makeKeyAndOrderFront(nil)
        window.orderOut(nil)
        assert(!window.isVisible && panel.isVisible)
        assert(!host!.returnToOwner())
        assert(!panel.isVisible, "Wake must clear preview even when its owner is hidden")
        assert(!window.isVisible, "Clearing an expired preview must not revive its hidden owner")
        window.makeKeyAndOrderFront(nil)
        window.makeFirstResponder(editor)
        messenger.show("lifecycle-hide", generation: 8, path: file.path)
        panel.makeKeyAndOrderFront(nil)
        window.orderOut(nil)
        host!.close(restoreFocus: false)
        RunLoop.current.run(until: Date().addingTimeInterval(16))
        assert(
          !panel.isVisible && !window.isVisible,
          "Hidden previews must stay closed past retention"
        )
        host = nil
      }
      // Check after AppKit's event autorelease pool has drained as well.
      DispatchQueue.main.asyncAfter(deadline: .now() + 0.1) {
        assert(hostWasReleased(), "Hidden Quick Look must not retain the host or engine")
        window.orderOut(nil)
        try! FileManager.default.removeItem(at: directory)
        print(
          "Native preview live: text input, immediate reopen, update, native close, retention and host release passed"
        )
        app.terminate(nil)
      }
    }
    app.run()
  }
}
