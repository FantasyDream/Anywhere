import 'package:flutter/services.dart';
import '../infrastructure/hotkey_keymap.dart';
import 'action_shortcut.dart';

class HostSettings {
  const HostSettings({
    required this.currentAppKeyCode,
    required this.currentAppModifiers,
    required this.universalKeyCode,
    required this.universalModifiers,
    required this.launchAtLogin,
    required this.excludedApps,
    required this.captureFinderSelection,
    required this.captureSelectedText,
    required this.captureBrowserCurrentPage,
    required this.captureClipboard,
    this.actionShortcuts = const [],
    this.actionShortcutVersion = 1,
  });

  static const cmdKey = nativeCmdKey;
  static const shiftKey = nativeShiftKey;
  static const optionKey = nativeOptionKey;
  static const controlKey = nativeControlKey;
  static const spaceKey = 0x31;

  static const defaults = HostSettings(
    currentAppKeyCode: spaceKey,
    currentAppModifiers: cmdKey | optionKey,
    universalKeyCode: spaceKey,
    universalModifiers: cmdKey | optionKey | shiftKey,
    launchAtLogin: false,
    excludedApps: [],
    captureFinderSelection: true,
    captureSelectedText: true,
    captureBrowserCurrentPage: true,
    captureClipboard: true,
  );

  final int currentAppKeyCode;
  final int currentAppModifiers;
  final int universalKeyCode;
  final int universalModifiers;
  final bool launchAtLogin;
  final List<String> excludedApps;
  final bool captureFinderSelection;
  final bool captureSelectedText;
  final bool captureBrowserCurrentPage;
  final bool captureClipboard;
  final List<ActionShortcut> actionShortcuts;
  final int actionShortcutVersion;

  String get currentAppLabel =>
      formatHotkey(currentAppKeyCode, currentAppModifiers);
  String get universalLabel =>
      formatHotkey(universalKeyCode, universalModifiers);

  HostSettings copyWith({
    int? currentAppKeyCode,
    int? currentAppModifiers,
    int? universalKeyCode,
    int? universalModifiers,
    bool? launchAtLogin,
    List<String>? excludedApps,
    bool? captureFinderSelection,
    bool? captureSelectedText,
    bool? captureBrowserCurrentPage,
    bool? captureClipboard,
    List<ActionShortcut>? actionShortcuts,
  }) =>
      HostSettings(
        currentAppKeyCode: currentAppKeyCode ?? this.currentAppKeyCode,
        currentAppModifiers: currentAppModifiers ?? this.currentAppModifiers,
        universalKeyCode: universalKeyCode ?? this.universalKeyCode,
        universalModifiers: universalModifiers ?? this.universalModifiers,
        launchAtLogin: launchAtLogin ?? this.launchAtLogin,
        excludedApps: excludedApps ?? this.excludedApps,
        captureFinderSelection:
            captureFinderSelection ?? this.captureFinderSelection,
        captureSelectedText: captureSelectedText ?? this.captureSelectedText,
        captureBrowserCurrentPage:
            captureBrowserCurrentPage ?? this.captureBrowserCurrentPage,
        captureClipboard: captureClipboard ?? this.captureClipboard,
        actionShortcuts: actionShortcuts ?? this.actionShortcuts,
        actionShortcutVersion: actionShortcutVersion,
      );

  Map<String, Object?> toMap() => {
        'currentAppKeyCode': currentAppKeyCode,
        'currentAppModifiers': currentAppModifiers,
        'universalKeyCode': universalKeyCode,
        'universalModifiers': universalModifiers,
        'launchAtLogin': launchAtLogin,
        'excludedApps': excludedApps,
        'captureFinderSelection': captureFinderSelection,
        'captureSelectedText': captureSelectedText,
        'captureBrowserCurrentPage': captureBrowserCurrentPage,
        'captureClipboard': captureClipboard,
        'actionShortcutVersion': actionShortcutVersion,
        'actionShortcuts': [
          for (final binding in actionShortcuts) binding.toMap()
        ],
      };

  static HostSettings fromMap(Map<Object?, Object?> map) {
    int number(String key, int fallback) {
      final value = map[key];
      if (value is int) return value;
      if (value is num) return value.toInt();
      return fallback;
    }

    final excluded = map['excludedApps'];
    return HostSettings(
      currentAppKeyCode:
          number('currentAppKeyCode', defaults.currentAppKeyCode),
      currentAppModifiers:
          number('currentAppModifiers', defaults.currentAppModifiers),
      universalKeyCode: number('universalKeyCode', defaults.universalKeyCode),
      universalModifiers:
          number('universalModifiers', defaults.universalModifiers),
      launchAtLogin: map['launchAtLogin'] == true,
      excludedApps: excluded is List
          ? [
              for (final item in excluded)
                if (item is String && item.trim().isNotEmpty) item.trim()
            ]
          : const [],
      captureFinderSelection: map['captureFinderSelection'] != false,
      captureSelectedText: map['captureSelectedText'] != false,
      captureBrowserCurrentPage: map['captureBrowserCurrentPage'] != false,
      captureClipboard: map['captureClipboard'] != false,
      actionShortcutVersion: number('actionShortcutVersion', 1),
      actionShortcuts:
          map['actionShortcutVersion'] == 1 && map['actionShortcuts'] is List
              ? [
                  for (final raw in map['actionShortcuts'] as List)
                    if (ActionShortcut.fromMap(raw) case final binding?) binding
                ]
              : const [],
    );
  }
}

String formatHotkey(int keyCode, int modifiers) {
  final buffer = StringBuffer();
  for (final entry in nativeModifierLabels.entries) {
    if (modifiers & entry.key != 0) buffer.write(entry.value);
  }
  buffer.write(macOSKeyNames[keyCode] ?? 'Key');
  return buffer.toString();
}

int? carbonKeyCode(LogicalKeyboardKey key) => macOSLogicalKeyCodes[key];
