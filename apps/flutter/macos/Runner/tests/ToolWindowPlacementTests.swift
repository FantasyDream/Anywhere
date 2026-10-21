import AppKit

@main
struct ToolWindowPlacementTests {
    static func main() {
        // visibleFrame excludes a bottom Dock and a top menu bar.
        let main = NSRect(x: 0, y: 80, width: 1440, height: 797)
        let first = ToolWindowPlacement.bottomTrailing(size: ToolWindowPlacement.preferredSize, in: main)
        assert(first.minX == 1024 && first.minY == 96)
        assert(first.width == 400 && first.height == 520)
        assert(main.contains(first))

        // Secondary screens need not start at (0,0).
        for visible in [NSRect(x: -1920, y: -200, width: 1920, height: 1040),
                        NSRect(x: 1440, y: 1080, width: 1280, height: 720)] {
            let frame = ToolWindowPlacement.bottomTrailing(size: first.size, in: visible)
            assert(visible.maxX - frame.maxX == 16)
            assert(frame.minY - visible.minY == 16)
            assert(visible.contains(frame))
        }
        // Reopening after resizing preserves that size; smaller work areas fit.
        let resized = ToolWindowPlacement.bottomTrailing(size: NSSize(width: 460, height: 620), in: main)
        assert(resized.width == 460 && resized.height == 620)
        let small = NSRect(x: -500, y: 20, width: 340, height: 400)
        let fitted = ToolWindowPlacement.bottomTrailing(size: resized.size, in: small)
        assert(fitted.width == 308 && fitted.height == 368 && small.contains(fitted))
        print("Tool window placement: Dock-safe corner, external origins, resizing and work-area fitting passed")
    }
}
