import AppKit
import Darwin
import FlutterMacOS
import QuickLookUI

private func fail(_ message: String, file: StaticString = #fileID, line: UInt = #line) -> Never {
  fputs("\(file):\(line): \(message)\n", stderr)
  exit(EXIT_FAILURE)
}

private func verify(
  _ condition: @autoclosure () -> Bool,
  _ message: String = "Preview check failed",
  file: StaticString = #fileID,
  line: UInt = #line
) {
  if !condition() { fail(message, file: file, line: line) }
}

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
      verify(reply == self.codec.encodeSuccessEnvelope(nil), "Preview show failed: \(id)")
      replied = true
    }
    verify(replied)
    verify(events.last?.method == "shown")
    verify((events.last?.arguments as? [String: Any])?["id"] as? String == id)
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
    let file = directory.appendingPathComponent("preview.txt")
    let xml = directory.appendingPathComponent("settings.xml")
    let xmlRows = (0..<350).map { "<!-- Synthetic XML preview row \($0) -->" }.joined(
      separator: "\n"
    )
    do {
      try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
      try "Synthetic preview regression".write(to: file, atomically: true, encoding: .utf8)
      try "<?xml version=\"1.0\"?>\n<settings>\n\(xmlRows)\n</settings>\n".write(
        to: xml,
        atomically: true,
        encoding: .utf8
      )
    } catch { fail("Unable to prepare preview fixtures: \(error)") }

    DispatchQueue.main.asyncAfter(deadline: .now() + 0.3) {
      app.activate(ignoringOtherApps: true)
      window.makeKeyAndOrderFront(nil)
    }
    let runChecks: @MainActor @Sendable () -> Void = {
      // App activation is asynchronous and may be delayed by system dialogs.
      let activationDeadline = Date().addingTimeInterval(5)
      while (!app.isActive || !window.isKeyWindow) && Date() < activationDeadline {
        app.activate(ignoringOtherApps: true)
        window.makeKeyAndOrderFront(nil)
        RunLoop.current.run(until: Date().addingTimeInterval(0.1))
      }
      verify(app.isActive && window.isKeyWindow, "Live preview test requires an active desktop")
      autoreleasepool {
        guard let panel = QLPreviewPanel.shared() else { fail("Quick Look panel is unavailable") }
        // NSTextView is a competing Quick Look controller, including Flutter's
        // native editor. The host must keep it from claiming our list preview.
        verify(editor.acceptsPreviewPanelControl(panel))
        for index in 1...3 {
          verify(window.firstResponder === editor)
          messenger.show("preview-\(index)", generation: index, path: file.path)
          verify(window.firstResponder === receiver)
          verify(panel.dataSource === host)
          verify(panel.currentPreviewItem?.previewItemURL == file)
          RunLoop.current.run(until: Date().addingTimeInterval(0.35))
          verify(panel.isVisible && host!.ownsPanel)
          NativePreviewHost.dismissUnownedPanel()
          verify(panel.isVisible && host!.ownsPanel, "Wake cleanup must preserve an owned preview")
          if index == 3 {
            panel.makeKeyAndOrderFront(nil)
            verify(host!.returnToOwner())
            verify(window.isKeyWindow)
            verify(!host!.returnToOwner())
          } else {
            host!.close(restoreFocus: index != 2)
          }
          verify(!panel.isVisible)
          verify(window.firstResponder === editor)
          verify(window.nextResponder === originalResponder)
          verify(panel.dataSource == nil && panel.delegate == nil)
          // Deliberately reopen immediately: no delay may hide a late close.
        }
        messenger.show("replacement", generation: 4, path: file.path)
        messenger.show("updated", generation: 5, path: file.path)
        verify(panel.currentPreviewItem?.previewItemTitle == "updated")
        verify(window.firstResponder === receiver)
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
        verify(!host!.previewPanel(panel, handle: wake), "Preview must not consume Command Space")
        verify(panel.isVisible && host!.ownsPanel)
        // The system close button follows Quick Look's own close lifecycle.
        guard let closeButton = panel.standardWindowButton(.closeButton) else {
          fail("System preview close button is unavailable")
        }
        closeButton.performClick(nil)
        verify(!host!.ownsPanel && !panel.isVisible)
        verify(closeButton.target !== host, "Closing must restore the system button target")
        verify(window.firstResponder === editor)
        // A user can reopen before Quick Look finishes its native close animation.
        messenger.show("after-native-close", generation: 6, path: file.path)
        RunLoop.current.run(until: Date().addingTimeInterval(0.35))
        verify(
          panel.isVisible,
          "Reopened preview disappeared: active=\(app.isActive), key=\(window.isKeyWindow), owned=\(host!.ownsPanel)"
        )
        host!.close()
        verify(messenger.events.filter { $0.method == "closed" }.count == 5)
        messenger.show("xml-native-close", generation: 7, path: xml.path)
        RunLoop.current.run(until: Date().addingTimeInterval(1.5))
        panel.makeKeyAndOrderFront(nil)
        verify(panel.isVisible && host!.ownsPanel)
        guard let xmlCloseButton = panel.standardWindowButton(.closeButton) else {
          fail("XML close button is unavailable")
        }
        xmlCloseButton.performClick(nil)
        RunLoop.current.run(until: Date().addingTimeInterval(0.5))
        verify(
          !panel.isVisible && !host!.ownsPanel,
          "XML preview must close through its system action"
        )
        verify(window.firstResponder === editor)
        messenger.show("xml-loading-close", generation: 8, path: xml.path)
        host!.close(restoreFocus: false)
        RunLoop.current.run(until: Date().addingTimeInterval(1.5))
        verify(
          !panel.isVisible && !host!.ownsPanel,
          "A late XML renderer must not reopen a closed preview"
        )
        messenger.show("renderer-hide", generation: 9, path: xml.path)
        RunLoop.current.run(until: Date().addingTimeInterval(0.35))
        let beforeHide = messenger.events.filter { $0.method == "closed" }.count
        panel.order(.out, relativeTo: 0)
        verify(!panel.isVisible)
        RunLoop.current.run(until: Date().addingTimeInterval(0.1))
        verify(!host!.ownsPanel, "A native hide without willClose must end the preview lease")
        verify(messenger.events.last?.method == "closed")
        verify(messenger.events.filter { $0.method == "closed" }.count == beforeHide + 1)
        verify(window.firstResponder === editor)
        panel.close()
        verify(messenger.events.filter { $0.method == "closed" }.count == beforeHide + 1)
        messenger.show("transition-hide", generation: 10, path: xml.path)
        panel.order(.out, relativeTo: 0)
        messenger.show("after-transition-hide", generation: 11, path: xml.path)
        RunLoop.current.run(until: Date().addingTimeInterval(0.35))
        verify(
          host!.ownsPanel && panel.isVisible,
          "An old hide callback must not close the reopened preview"
        )
        host!.close()
        messenger.show("hidden-owner", generation: 12, path: file.path)
        panel.makeKeyAndOrderFront(nil)
        window.orderOut(nil)
        verify(!window.isVisible && panel.isVisible)
        verify(!host!.returnToOwner())
        verify(!panel.isVisible, "Wake must clear preview even when its owner is hidden")
        verify(!window.isVisible, "Clearing an expired preview must not revive its hidden owner")
        window.makeKeyAndOrderFront(nil)
        window.makeFirstResponder(editor)
        messenger.show("lifecycle-hide", generation: 13, path: xml.path)
        panel.makeKeyAndOrderFront(nil)
        app.hide(nil)
        window.orderOut(nil)
        host!.close(restoreFocus: false)
        RunLoop.current.run(until: Date().addingTimeInterval(16))
        verify(
          !panel.isVisible && !window.isVisible,
          "Hidden previews must stay closed past retention"
        )
        app.unhide(nil)
        app.activate(ignoringOtherApps: true)
        window.makeKeyAndOrderFront(nil)
        WindowInputFocus.restore(in: window, content: root)
        NativePreviewHost.dismissUnownedPanel()
        RunLoop.current.run(until: Date().addingTimeInterval(0.35))
        verify(!panel.isVisible && !host!.ownsPanel, "Unhide must not restore an expired preview")
        host = nil
      }
      // Check after AppKit's event autorelease pool has drained as well.
      DispatchQueue.main.asyncAfter(deadline: .now() + 0.1) {
        verify(hostWasReleased(), "Hidden Quick Look must not retain the host or engine")
        guard let orphanedPanel = QLPreviewPanel.shared() else {
          fail("Quick Look panel is unavailable after Host release")
        }
        orphanedPanel.orderFront(nil)
        NativePreviewHost.dismissUnownedPanel()
        verify(!orphanedPanel.isVisible, "Wake must dismiss a panel restored without a live Host")
        window.orderOut(nil)
        do { try FileManager.default.removeItem(at: directory) } catch {
          fail("Unable to remove preview fixtures: \(error)")
        }
        print(
          "Native preview live: text input, immediate reopen, update, native close/hide, XML close/loading, retention and host release passed"
        )
        app.terminate(nil)
      }
    }
    Timer.scheduledTimer(withTimeInterval: 1, repeats: false) { _ in
      MainActor.assumeIsolated { runChecks() }
    }
    app.run()
  }
}
