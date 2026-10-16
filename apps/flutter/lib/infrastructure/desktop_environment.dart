import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'hotkey_keymap.dart';

/// Presentation capabilities supplied by the window host. Unknown hosts render
/// an opaque surface; transparency is enabled only by an explicit handshake.
class DesktopEnvironment {
  const DesktopEnvironment(
      {required this.platform,
      this.hasBackdrop = false,
      this.nativeShortcutRecording = false,
      this.fileSearchAccess = false});
  final TargetPlatform platform;
  final bool hasBackdrop;
  final bool nativeShortcutRecording;
  final bool fileSearchAccess;
  static DesktopEnvironment? _current;
  static DesktopEnvironment get current =>
      _current ?? DesktopEnvironment(platform: defaultTargetPlatform);
  static void configure(DesktopEnvironment environment) =>
      _current = environment;
  static void reset() => _current = null;

  static Future<DesktopEnvironment> read(MethodChannel channel) async {
    try {
      final capabilities =
          await channel.invokeMapMethod<String, Object?>('capabilities');
      return DesktopEnvironment(
          platform: defaultTargetPlatform,
          hasBackdrop: capabilities?['backdrop'] == true,
          nativeShortcutRecording: capabilities?['shortcutRecording'] == true,
          fileSearchAccess: capabilities?['fileSearchAccess'] == true);
    } on MissingPluginException {
      return DesktopEnvironment(platform: defaultTargetPlatform);
    }
  }

  bool get usesMetaCommand => platform == TargetPlatform.macOS;
  String? get fontFamily => switch (platform) {
        TargetPlatform.macOS => '.AppleSystemUIFont',
        TargetPlatform.windows => 'Segoe UI',
        _ => null,
      };
  String get fileSelectionLabel =>
      usesMetaCommand ? 'Finder selection' : 'File selection';
  String get applicationIdHint =>
      usesMetaCommand ? 'com.apple.Safari' : 'application.id';
  String get browserDescription => usesMetaCommand
      ? 'Active URL in Safari, Chrome or Edge'
      : 'Active URL in a supported browser';
  double get titleBarInset => usesMetaCommand ? 18 : 0;
  Map<int, String> get modifierLabels => usesMetaCommand
      ? const {
          shortcutControlKey: '⌃',
          shortcutAltKey: '⌥',
          shortcutShiftKey: '⇧',
          shortcutMetaKey: '⌘'
        }
      : const {
          shortcutControlKey: 'Ctrl+',
          shortcutAltKey: 'Alt+',
          shortcutShiftKey: 'Shift+',
          shortcutMetaKey: 'Win+'
        };
  bool get commandPressed => usesMetaCommand
      ? HardwareKeyboard.instance.isMetaPressed
      : HardwareKeyboard.instance.isControlPressed;
  SingleActivator command(LogicalKeyboardKey key, {bool alt = false}) =>
      SingleActivator(key,
          meta: usesMetaCommand, control: !usesMetaCommand, alt: alt);
  String commandShiftLabel(String key) =>
      commandLabel('${modifierLabels[shortcutShiftKey]}$key');
  String commandLabel(String key) => '${usesMetaCommand ? '⌘' : 'Ctrl+'}$key';
}
