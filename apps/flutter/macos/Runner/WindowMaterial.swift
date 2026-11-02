import AppKit

/// Hosts content inside the native effect rather than overlaying sibling views.
/// One material per window keeps Flutter's engine and responder chain intact.
final class WindowMaterial: NativeAccessibilityContainer {
  private let effect: NSView
  let usesLiquidGlass: Bool
  var maskImage: NSImage? {
    didSet { (effect as? NSVisualEffectView)?.maskImage = maskImage }
  }
  var cornerRadius: CGFloat {
    didSet { updateCorners() }
  }

  init(
    frame: NSRect,
    content: NSView,
    cornerRadius: CGFloat,
    preferLiquidGlass: Bool = true
  ) {
    self.cornerRadius = cornerRadius
    // Older SDKs can still build the supported macOS 12 fallback.
    #if compiler(>=6.2)
      if #available(macOS 26.0, *), preferLiquidGlass {
        let glass = NSGlassEffectView(frame: frame)
        glass.style = .regular
        glass.contentView = content
        effect = glass
        usesLiquidGlass = true
      } else {
        effect = Self.fallback(frame: frame, content: content)
        usesLiquidGlass = false
      }
    #else
      effect = Self.fallback(frame: frame, content: content)
      usesLiquidGlass = false
    #endif
    super.init(frame: frame)
    appearance = NSAppearance(named: .darkAqua)
    autoresizingMask = [.width, .height]
    effect.frame = bounds
    effect.autoresizingMask = [.width, .height]
    content.frame = bounds
    content.autoresizingMask = [.width, .height]
    addSubview(effect)
    updateCorners()
  }

  required init?(coder: NSCoder) { fatalError("Use init(frame:content:cornerRadius:)") }

  func detachContent(_ content: NSView) {
    setEmbeddedAccessibilityChild(nil)
    #if compiler(>=6.2)
      if #available(macOS 26.0, *), let glass = effect as? NSGlassEffectView,
        glass.contentView === content
      {
        glass.contentView = nil
      }
    #endif
    content.removeFromSuperview()
  }

  private static func fallback(frame: NSRect, content: NSView) -> NSVisualEffectView {
    let material = NSVisualEffectView(frame: frame)
    material.material = .hudWindow
    material.blendingMode = .behindWindow
    material.state = .active
    material.addSubview(content)
    return material
  }

  private func updateCorners() {
    wantsLayer = true
    layer?.cornerRadius = cornerRadius
    layer?.masksToBounds = cornerRadius > 0
    #if compiler(>=6.2)
      if #available(macOS 26.0, *) {
        (effect as? NSGlassEffectView)?.cornerRadius = cornerRadius
      }
    #endif
  }
}

/// The same host is moved between Launcher and Settings, retaining one engine.
final class WindowMaterialController: NSViewController {
  let material: WindowMaterial
  init(contentController: NSViewController, frame: NSRect, cornerRadius: CGFloat) {
    material = WindowMaterial(
      frame: frame,
      content: contentController.view,
      cornerRadius: cornerRadius
    )
    super.init(nibName: nil, bundle: nil)
    addChild(contentController)
    view = material
  }
  required init?(coder: NSCoder) { fatalError("Use init(contentController:frame:cornerRadius:)") }

  /// Dedicated tool windows detach native input and containment before stopping
  /// their engine. Moving Launcher / Settings keeps the content attached instead.
  func detachContent() {
    for child in children {
      let content = child.view
      if let window = content.window, let responder = window.firstResponder as? NSView,
        responder === content || responder.isDescendant(of: content)
      {
        window.makeFirstResponder(nil)
      }
      material.detachContent(content)
      child.removeFromParent()
    }
  }
}
