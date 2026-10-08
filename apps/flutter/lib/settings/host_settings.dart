import 'package:flutter/services.dart';
import '../infrastructure/hotkey_keymap.dart';
import '../infrastructure/legacy_macos_shortcuts.dart';
import '../infrastructure/desktop_environment.dart';
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
    this.actionShortcutVersion = 4,
  });

  static const metaKey = shortcutMetaKey;
  static const shiftKey = shortcutShiftKey;
  static const altKey = shortcutAltKey;
  static const controlKey = shortcutControlKey;
  static const spaceKey = 0x2c;

  static const defaults = HostSettings(
    currentAppKeyCode: spaceKey,
    currentAppModifiers: metaKey | altKey,
    universalKeyCode: spaceKey,
    universalModifiers: metaKey | altKey | shiftKey,
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

    // Legacy native codes are converted once, before validation or matching.
    if (number('actionShortcutVersion', 1) < 4) {
      map = migrateLegacyMacSettings(map);
    }
    final excluded = map['excludedApps'];
    final version = number('actionShortcutVersion', 1);
    final supported = version >= 1 && version <= 4;
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
      actionShortcutVersion: supported ? 4 : version,
      actionShortcuts: supported && map['actionShortcuts'] is List
          ? [
              for (final raw in map['actionShortcuts'] as List)
                if (ActionShortcut.fromMap(version < 3 && raw is Map
                        ? {...raw, 'routes': []}
                        : raw)
                    case final binding?)
                  version == 1 ? binding.withGlobal(false) : binding
            ]
          : const [],
    );
  }
}

String formatHotkey(int keyCode, int modifiers) {
  final buffer = StringBuffer();
  for (final entry in DesktopEnvironment.current.modifierLabels.entries) {
    if (modifiers & entry.key != 0) buffer.write(entry.value);
  }
  buffer.write(shortcutKeyNames[keyCode] ?? 'Key');
  return buffer.toString();
}

int? shortcutKeyCode(LogicalKeyboardKey key) => shortcutLogicalKeyCodes[key];

/// USB HID keyboard usages preserve physical positions across OS and layouts.
int? shortcutEventKeyCode(KeyEvent event) =>
    shortcutPhysicalKeyCodes[event.physicalKey];

Map<Object?, Object?> migrateLegacyMacSettings(Map<Object?, Object?> raw) {
  int? key(Object? value) => value is int ? legacyMacKeyCodes[value] : null;
  int? modifiers(Object? value) {
    if (value is! int || value & ~6912 != 0) return null;
    return legacyMacModifiers.entries.fold<int>(
        0, (mask, entry) => mask | (value & entry.key != 0 ? entry.value : 0));
  }

  Object? binding(Object? value) => value is Map
      ? {
          ...value,
          'keyCode': key(value['keyCode']),
          'modifiers': modifiers(value['modifiers'])
        }
      : value;
  return {
    ...raw,
    for (final prefix in ['currentApp', 'universal']) ...{
      '${prefix}KeyCode': key(raw['${prefix}KeyCode']),
      '${prefix}Modifiers': modifiers(raw['${prefix}Modifiers']),
    },
    if (raw['actionShortcuts'] is List)
      'actionShortcuts': (raw['actionShortcuts'] as List).map(binding).toList(),
  };
}
