import AppKit

/// AppKit coordinates start at the bottom. visibleFrame already excludes Dock
/// and menu bar; keep this calculation independent of window/focus side effects.
enum ToolWindowPlacement {
    static let preferredSize = NSSize(width: 400, height: 520)

    static func bottomTrailing(size: NSSize, in visible: NSRect) -> NSRect {
        let inset = min(16, max(0, (min(visible.width, visible.height) - 1) / 2))
        let width = min(max(1, size.width), max(1, visible.width - inset * 2))
        let height = min(max(1, size.height), max(1, visible.height - inset * 2))
        return NSRect(x: visible.maxX - inset - width, y: visible.minY + inset,
                      width: width, height: height)
    }
}
