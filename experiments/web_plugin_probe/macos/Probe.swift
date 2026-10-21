import AppKit
import FlutterMacOS
import flutter_inappwebview_macos

/// Isolated developer host. No launcher, clipboard, AX, directory or Rust access.
final class ProbeDelegate: NSObject, NSApplicationDelegate, NSWindowDelegate {
    var window: NSWindow!
    var view: FlutterViewController!
    var channel: FlutterMethodChannel!
    var monitor: Any?

    func applicationDidFinishLaunching(_ notification: Notification) {
        NSApp.setActivationPolicy(.regular)
        view = FlutterViewController(project: FlutterDartProject())
        InAppWebViewFlutterPlugin.register(with: view.registrar(forPlugin: "InAppWebViewFlutterPlugin"))
        view.backgroundColor = .clear
        channel = FlutterMethodChannel(name: "anywhere/web-plugin-probe", binaryMessenger: view.engine.binaryMessenger)
        channel.setMethodCallHandler { call, reply in
            if call.method == "report", let value = call.arguments as? String, value.utf8.count <= 4096 {
                print(value)
                fflush(stdout)
                reply(nil)
                if ProcessInfo.processInfo.arguments.contains("--self-test") {
                    DispatchQueue.main.asyncAfter(deadline: .now() + 0.2) { NSApp.terminate(nil) }
                }
            } else { reply(FlutterMethodNotImplemented) }
        }
        window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 760, height: 480),
            styleMask: [.titled, .closable, .resizable], backing: .buffered, defer: false)
        window.title = "Anywhere · Web Plugin P0 Probe"
        window.isReleasedWhenClosed = false
        window.delegate = self
        window.contentViewController = view
        window.appearance = NSAppearance(named: .darkAqua)
        window.center()
        let menu = NSMenu()
        let app = NSMenuItem()
        let appMenu = NSMenu()
        appMenu.addItem(withTitle: "Quit Probe", action: #selector(NSApplication.terminate(_:)), keyEquivalent: "q")
        app.submenu = appMenu
        menu.addItem(app)
        let edit = NSMenuItem()
        let editMenu = NSMenu(title: "Edit")
        for (title, selector, key) in [("Cut", "cut:", "x"), ("Copy", "copy:", "c"),
                                      ("Paste", "paste:", "v"), ("Select All", "selectAll:", "a")] {
            editMenu.addItem(withTitle: title, action: Selector(selector), keyEquivalent: key)
        }
        edit.submenu = editMenu
        menu.addItem(edit)
        NSApp.mainMenu = menu
        monitor = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { [weak self] event in
            guard let self, NSApp.keyWindow === self.window else { return event }
            let flags = event.modifierFlags.intersection(.deviceIndependentFlagsMask)
            if flags == .command, event.charactersIgnoringModifiers?.lowercased() == "k" {
                self.channel.invokeMethod("actions", arguments: nil)
                return nil
            }
            // Let WebKit finish a marked IME composition before treating Esc as Back.
            let composing = (self.window.firstResponder as? NSTextInputClient)?.hasMarkedText() ?? false
            if event.keyCode == 53, !composing {
                self.channel.invokeMethod("back", arguments: nil)
                return nil
            }
            return event
        }
        window.makeKeyAndOrderFront(nil)
        NSApp.activate(ignoringOtherApps: true)
    }
    func windowDidBecomeKey(_ notification: Notification) { channel?.invokeMethod("visibility", arguments: true) }
    func windowDidResignKey(_ notification: Notification) { channel?.invokeMethod("visibility", arguments: false) }
    func windowWillClose(_ notification: Notification) { NSApp.terminate(nil) }
    func applicationWillTerminate(_ notification: Notification) {
        if let monitor { NSEvent.removeMonitor(monitor) }
        channel?.setMethodCallHandler(nil)
    }
}

let app = NSApplication.shared
let delegate = ProbeDelegate()
app.delegate = delegate
app.run()
