import 'package:flutter/services.dart';

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
  });

  static const cmdKey = 1 << 8;
  static const shiftKey = 1 << 9;
  static const optionKey = 1 << 11;
  static const controlKey = 1 << 12;
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
    );
  }
}

String formatHotkey(int keyCode, int modifiers) {
  final buffer = StringBuffer();
  if (modifiers & HostSettings.controlKey != 0) buffer.write('⌃');
  if (modifiers & HostSettings.optionKey != 0) buffer.write('⌥');
  if (modifiers & HostSettings.shiftKey != 0) buffer.write('⇧');
  if (modifiers & HostSettings.cmdKey != 0) buffer.write('⌘');
  buffer.write(_keyName(keyCode));
  return buffer.toString();
}

String _keyName(int keyCode) {
  const named = {
    0x31: 'Space',
    0x24: 'Return',
    0x30: 'Tab',
    0x35: 'Esc',
  };
  if (named.containsKey(keyCode)) return named[keyCode]!;
  const letters = {
    0x00: 'A',
    0x0B: 'B',
    0x08: 'C',
    0x02: 'D',
    0x0E: 'E',
    0x03: 'F',
    0x05: 'G',
    0x04: 'H',
    0x22: 'I',
    0x26: 'J',
    0x28: 'K',
    0x25: 'L',
    0x2E: 'M',
    0x2D: 'N',
    0x1F: 'O',
    0x23: 'P',
    0x0C: 'Q',
    0x0F: 'R',
    0x01: 'S',
    0x11: 'T',
    0x20: 'U',
    0x09: 'V',
    0x0D: 'W',
    0x07: 'X',
    0x10: 'Y',
    0x06: 'Z',
  };
  return letters[keyCode] ?? 'Key';
}

int? carbonKeyCode(LogicalKeyboardKey key) {
  final letters = {
    LogicalKeyboardKey.keyA: 0x00,
    LogicalKeyboardKey.keyB: 0x0B,
    LogicalKeyboardKey.keyC: 0x08,
    LogicalKeyboardKey.keyD: 0x02,
    LogicalKeyboardKey.keyE: 0x0E,
    LogicalKeyboardKey.keyF: 0x03,
    LogicalKeyboardKey.keyG: 0x05,
    LogicalKeyboardKey.keyH: 0x04,
    LogicalKeyboardKey.keyI: 0x22,
    LogicalKeyboardKey.keyJ: 0x26,
    LogicalKeyboardKey.keyK: 0x28,
    LogicalKeyboardKey.keyL: 0x25,
    LogicalKeyboardKey.keyM: 0x2E,
    LogicalKeyboardKey.keyN: 0x2D,
    LogicalKeyboardKey.keyO: 0x1F,
    LogicalKeyboardKey.keyP: 0x23,
    LogicalKeyboardKey.keyQ: 0x0C,
    LogicalKeyboardKey.keyR: 0x0F,
    LogicalKeyboardKey.keyS: 0x01,
    LogicalKeyboardKey.keyT: 0x11,
    LogicalKeyboardKey.keyU: 0x20,
    LogicalKeyboardKey.keyV: 0x09,
    LogicalKeyboardKey.keyW: 0x0D,
    LogicalKeyboardKey.keyX: 0x07,
    LogicalKeyboardKey.keyY: 0x10,
    LogicalKeyboardKey.keyZ: 0x06,
    LogicalKeyboardKey.space: 0x31,
    LogicalKeyboardKey.enter: 0x24,
    LogicalKeyboardKey.tab: 0x30,
    LogicalKeyboardKey.escape: 0x35,
  };
  return letters[key];
}
