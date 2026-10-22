import AppKit

final class KeyboardReceiver: NSView {
    override var acceptsFirstResponder: Bool { true }
}

@main
struct WindowInputFocusTests {
    @MainActor static func main() {
        _ = NSApplication.shared
        let frame = NSRect(x: 0, y: 0, width: 400, height: 200)
        let launcher = NSWindow(contentRect: frame, styleMask: [.borderless],
                                backing: .buffered, defer: false)
        let settings = NSWindow(contentRect: frame, styleMask: [.titled],
                                backing: .buffered, defer: false)
        launcher.isReleasedWhenClosed = false
        settings.isReleasedWhenClosed = false
        let wrapper = NSView(frame: frame)
        let surface = KeyboardReceiver(frame: frame)
        let editor = KeyboardReceiver(frame: frame)
        wrapper.addSubview(surface)
        wrapper.addSubview(editor)
        launcher.contentView = wrapper

        // Reproduce the old call: AppKit accepts the wrapper even though its
        // acceptsFirstResponder is false, replacing a working text receiver.
        assert(launcher.makeFirstResponder(editor))
        assert(!wrapper.acceptsFirstResponder)
        assert(launcher.makeFirstResponder(wrapper))
        assert(launcher.firstResponder === wrapper)
        WindowInputFocus.restore(in: launcher, content: wrapper)
        assert(launcher.firstResponder === surface)

        // Showing/awakening must not displace an editor attached before show.
        assert(launcher.makeFirstResponder(editor))
        for _ in 0..<3 {
            launcher.orderOut(nil)
            WindowInputFocus.restore(in: launcher, content: wrapper)
            assert(launcher.firstResponder === editor)
        }

        // Switching pages removes the old text input view while Flutter's new
        // client is attached. AppKit falls back to the window; restore routing.
        editor.removeFromSuperview()
        assert(launcher.firstResponder === launcher)
        WindowInputFocus.restore(in: launcher, content: wrapper)
        assert(launcher.firstResponder === surface)
        wrapper.addSubview(editor)
        assert(launcher.makeFirstResponder(editor))

        // A shared-engine move chooses a receiver belonging to the new window.
        launcher.contentView = nil
        settings.contentView = wrapper
        WindowInputFocus.restore(in: launcher, content: wrapper)
        assert(launcher.firstResponder !== editor)
        WindowInputFocus.restore(in: settings, content: wrapper)
        assert(settings.firstResponder === surface)
        assert(settings.makeFirstResponder(editor))
        WindowInputFocus.restore(in: settings, content: wrapper)
        assert(settings.firstResponder === editor)

        // Content with no receiver must not replace an unrelated responder.
        let empty = NSView(frame: frame)
        settings.contentView = empty
        let before = settings.firstResponder
        WindowInputFocus.restore(in: settings, content: empty)
        assert(settings.firstResponder === before)
        print("Window input focus: wrapper regression, editor removal, active editor, repeated wake and window move passed")
    }
}
