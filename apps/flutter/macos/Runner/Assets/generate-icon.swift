import AppKit

// Anywhere icon: a magnifier whose lens holds the current object and whose handle is
// a code mark. Drawn from geometry only, so it stays legible at 16pt.
let S: CGFloat = 1024
let image = NSImage(size: NSSize(width: S, height: S))
image.lockFocus()
let ctx = NSGraphicsContext.current!.cgContext

func color(_ r: CGFloat, _ g: CGFloat, _ b: CGFloat, _ a: CGFloat = 1) -> CGColor {
    CGColor(srgbRed: r, green: g, blue: b, alpha: a)
}

// --- squircle background -------------------------------------------------------
let inset: CGFloat = 26
let rect = CGRect(x: inset, y: inset, width: S - inset * 2, height: S - inset * 2)
ctx.saveGState()
let clip = CGPath(roundedRect: rect, cornerWidth: 224, cornerHeight: 224, transform: nil)
ctx.addPath(clip)
ctx.clip()
let space = CGColorSpaceCreateDeviceRGB()
let bg = CGGradient(colorsSpace: space,
                    colors: [color(0.169, 0.204, 0.278), color(0.067, 0.086, 0.133)] as CFArray,
                    locations: [0, 1])!
ctx.drawLinearGradient(bg,
                       start: CGPoint(x: rect.minX, y: rect.maxY),
                       end: CGPoint(x: rect.maxX, y: rect.minY),
                       options: [])
// A soft top highlight gives the plate a little depth.
let glow = CGGradient(colorsSpace: space,
                      colors: [color(1, 1, 1, 0.13), color(1, 1, 1, 0)] as CFArray,
                      locations: [0, 1])!
ctx.drawRadialGradient(glow,
                       startCenter: CGPoint(x: S * 0.5, y: S * 0.86), startRadius: 0,
                       endCenter: CGPoint(x: S * 0.5, y: S * 0.86), endRadius: S * 0.62,
                       options: [])
ctx.restoreGState()

// --- magnifier -----------------------------------------------------------------
// Axis runs from the lens centre to the handle tip; 45 degrees, the macOS default.
let lensCenter = CGPoint(x: S * 0.435, y: S * 0.565)
let ringRadius: CGFloat = S * 0.225
let ringWidth: CGFloat = S * 0.076

let handleStart = CGPoint(x: lensCenter.x + ringRadius * 0.72,
                          y: lensCenter.y - ringRadius * 0.72)
let handleEnd = CGPoint(x: S * 0.775, y: S * 0.225)
let handleWidth: CGFloat = S * 0.105

ctx.saveGState()
ctx.setShadow(offset: CGSize(width: 0, height: -S * 0.012),
              blur: S * 0.045,
              color: color(0, 0, 0, 0.45))
ctx.setLineCap(.round)
ctx.setStrokeColor(color(0.949, 0.961, 0.988))
ctx.setLineWidth(handleWidth)
ctx.move(to: handleStart)
ctx.addLine(to: handleEnd)
ctx.strokePath()

ctx.setLineWidth(ringWidth)
ctx.addArc(center: lensCenter, radius: ringRadius, startAngle: 0, endAngle: .pi * 2, clockwise: false)
ctx.strokePath()
ctx.restoreGState()

// --- the current object, inside the lens ---------------------------------------
let dotRadius: CGFloat = S * 0.056
ctx.setFillColor(color(0.435, 0.647, 1.0))
ctx.addArc(center: lensCenter, radius: dotRadius, startAngle: 0, endAngle: .pi * 2, clockwise: false)
ctx.fillPath()

image.unlockFocus()

guard let tiff = image.tiffRepresentation,
      let full = NSBitmapImageRep(data: tiff),
      let png = full.representation(using: .png, properties: [:]) else {
    FileHandle.standardError.write("icon render failed\n".data(using: .utf8)!)
    exit(1)
}
let out = CommandLine.arguments.count > 1 ? CommandLine.arguments[1] : "icon_1024.png"
try! png.write(to: URL(fileURLWithPath: out))
print("wrote \(out)")
