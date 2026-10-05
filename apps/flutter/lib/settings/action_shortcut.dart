import 'package:flutter/services.dart';
import '../infrastructure/hotkey_keymap.dart';
import 'host_settings.dart';

/// A persistent business action, never a captured object or Candidate ID.
class ActionShortcut {
  const ActionShortcut(
      {required this.actionId,
      required this.title,
      required this.keyCode,
      required this.modifiers});
  final String actionId, title;
  final int keyCode, modifiers;
  String get label => formatHotkey(keyCode, modifiers);
  Map<String, Object> toMap() => {
        'actionId': actionId,
        'title': title,
        'keyCode': keyCode,
        'modifiers': modifiers
      };
  static ActionShortcut? fromMap(Object? value) {
    if (value is! Map ||
        value['actionId'] is! String ||
        value['title'] is! String ||
        value['keyCode'] is! int ||
        value['modifiers'] is! int) {
      return null;
    }
    final binding = ActionShortcut(
        actionId: value['actionId'],
        title: value['title'],
        keyCode: value['keyCode'],
        modifiers: value['modifiers']);
    return binding.chordError == null ? binding : null;
  }

  String? get chordError {
    final name = macOSKeyNames[keyCode];
    if (actionId.isEmpty || actionId.length > 2048 || title.length > 512) {
      return 'Invalid action identity.';
    }
    if (name == null ||
        name.length != 1 ||
        !RegExp(r'^[A-Z]$').hasMatch(name)) {
      return 'Use a letter key with Command, Option or Control.';
    }
    const allowed =
        nativeCmdKey | nativeShiftKey | nativeOptionKey | nativeControlKey;
    if (modifiers & ~allowed != 0 ||
        modifiers & (nativeCmdKey | nativeOptionKey | nativeControlKey) == 0) {
      return 'Use Command, Option or Control with the letter key.';
    }
    if (modifiers & (nativeOptionKey | nativeControlKey) == 0 &&
        reservedActionKeyCodes.contains(keyCode)) {
      return 'This shortcut is reserved for editing or window commands.';
    }
    return null;
  }

  bool sameChord(int code, int mask) => keyCode == code && modifiers == mask;
  String? validate(HostSettings settings) {
    if (chordError != null) return chordError;
    if (sameChord(settings.currentAppKeyCode, settings.currentAppModifiers) ||
        sameChord(settings.universalKeyCode, settings.universalModifiers)) {
      return 'This shortcut already opens Anywhere.';
    }
    for (final other in settings.actionShortcuts) {
      if (other.actionId != actionId && other.sameChord(keyCode, modifiers)) {
        return 'This shortcut is assigned to ${other.title}.';
      }
    }
    return null;
  }

  bool matches(KeyEvent event) =>
      carbonKeyCode(event.logicalKey) == keyCode &&
      pressedModifiers() == modifiers;
}

int pressedModifiers() {
  final keyboard = HardwareKeyboard.instance;
  return (keyboard.isMetaPressed ? nativeCmdKey : 0) |
      (keyboard.isAltPressed ? nativeOptionKey : 0) |
      (keyboard.isControlPressed ? nativeControlKey : 0) |
      (keyboard.isShiftPressed ? nativeShiftKey : 0);
}
