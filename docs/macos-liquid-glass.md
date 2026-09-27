# macOS 27 glass integration review

The launcher already uses an AppKit `NSVisualEffectView` behind its transparent
Flutter surface. The panel now uses a lighter translucent fill, so the system
material can remain visible while text and actions retain a stable dark
background. This is a system blur, not Liquid Glass.

The macOS 27 SDK provides `NSGlassEffectView` (available from macOS 26). Its
`contentView` is the supported way to place controls inside the effect;
`effectIsInteractive` is available from macOS 27. The current host installs a
`FlutterViewController` directly as the window's content controller and puts
the visual-effect view behind it. Swapping in `NSGlassEffectView` as that
background would leave Flutter outside its `contentView`, so the SDK does not
guarantee the intended rendering or interaction. Flutter's macOS platform-view
support also lacks complete gesture handling, making a Flutter platform view a
poor fit for the launcher input and result list.

## Integration path

1. Build an AppKit container view controller, install the Flutter controller as
   its child, and place Flutter's view in `NSGlassEffectView.contentView`.
2. Apply glass to the control/navigation surface only. Keep long result lists
   readable with a solid enough content fill. Leave `effectIsInteractive`
   disabled unless hit testing and keyboard/IME behavior are proven on macOS 27.
3. Keep `NSVisualEffectView` for macOS 12–25. Test Reduce Transparency,
   Light/Dark appearance, resizing, keyboard focus, Chinese IME, repeated
   hotkeys, and VoiceOver on real hardware before shipping the native host
   change.

References: [Apple Liquid Glass overview](https://developer.apple.com/documentation/TechnologyOverviews/adopting-liquid-glass),
[NSGlassEffectView](https://developer.apple.com/documentation/appkit/nsglasseffectview),
[Flutter macOS platform views](https://docs.flutter.dev/platform-integration/macos/platform-views).
