import '../infrastructure/desktop_environment.dart';
import 'package:flutter/services.dart';
import '../src/rust/api.dart';
import '../infrastructure/hotkey_keymap.dart';
import 'host_settings.dart';

/// A persistent business action, never a captured object or Candidate ID.
class ActionShortcut {
  const ActionShortcut(
      {required this.actionId,
      required this.title,
      required this.keyCode,
      required this.modifiers,
      this.global = false,
      this.routes = const []});
  final String actionId, title;
  final int keyCode, modifiers;
  final bool global;
  final List<ShortcutRoute> routes;
  bool get isIntent => routes.isNotEmpty;
  String get label => formatHotkey(keyCode, modifiers);
  Map<String, Object> toMap() => {
        'actionId': actionId,
        'title': title,
        'keyCode': keyCode,
        'modifiers': modifiers,
        'global': global,
        'routes': [for (final route in routes) route.toMap()]
      };
  static ActionShortcut? fromMap(Object? value) {
    if (value is! Map ||
        value['actionId'] is! String ||
        value['title'] is! String ||
        value['keyCode'] is! int ||
        value['modifiers'] is! int ||
        (value.containsKey('global') && value['global'] is! bool)) {
      return null;
    }
    final rawRoutes = value['routes'];
    if (rawRoutes != null && rawRoutes is! List) return null;
    final routes = <ShortcutRoute>[];
    for (final raw in (rawRoutes as List? ?? [])) {
      final route = ShortcutRoute.fromMap(raw);
      if (route == null) return null;
      routes.add(route);
    }
    final binding = ActionShortcut(
        actionId: value['actionId'],
        title: value['title'],
        keyCode: value['keyCode'],
        modifiers: value['modifiers'],
        global: value['global'] == true,
        routes: routes);
    return binding.chordError == null ? binding : null;
  }

  String? get chordError {
    final name = shortcutKeyNames[keyCode];
    if (actionId.isEmpty || actionId.length > 2048 || title.length > 512) {
      return 'Invalid action identity.';
    }
    if (name == null ||
        name.length != 1 ||
        !RegExp(r'^[A-Z]$').hasMatch(name)) {
      return 'Use a letter key with a modifier.';
    }
    const allowed = shortcutMetaKey |
        shortcutShiftKey |
        shortcutAltKey |
        shortcutControlKey;
    if (modifiers & ~allowed != 0 ||
        modifiers & (shortcutMetaKey | shortcutAltKey | shortcutControlKey) ==
            0) {
      return 'Use a modifier with the letter key.';
    }
    final primary = DesktopEnvironment.current.usesMetaCommand
        ? shortcutMetaKey
        : shortcutControlKey;
    final secondary = DesktopEnvironment.current.usesMetaCommand
        ? shortcutControlKey
        : shortcutMetaKey;
    if (modifiers & primary != 0 &&
        modifiers & (shortcutAltKey | secondary) == 0 &&
        reservedActionKeyCodes.contains(keyCode)) {
      return 'This shortcut is reserved for editing or window commands.';
    }
    return null;
  }

  ActionShortcut withGlobal(bool enabled) => ActionShortcut(
      actionId: actionId,
      title: title,
      keyCode: keyCode,
      modifiers: modifiers,
      global: enabled,
      routes: routes);

  bool sameChord(int code, int mask) => keyCode == code && modifiers == mask;
  String? validate(HostSettings settings) =>
      configurationError ?? validateChord(settings);

  String? get configurationError {
    if (isIntent != actionId.startsWith('intent:')) {
      return 'Invalid shortcut intent identity.';
    }
    if (!isIntent) return null;
    if (routes.length > 16 || title.trim().isEmpty) {
      return 'Name the shortcut and use at most 16 rules.';
    }
    final conditions = <String>{};
    for (final route in routes) {
      if (route.error != null) return route.error;
      if (!conditions
          .add('${route.appBundleId ?? ''}/${route.contextType ?? ''}')) {
        return 'Rules must use different app and context conditions.';
      }
    }
    return null;
  }

  String? validateChord(HostSettings settings) {
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
      shortcutEventKeyCode(event) == keyCode && pressedModifiers() == modifiers;
}

int pressedModifiers() {
  final keyboard = HardwareKeyboard.instance;
  return (keyboard.isMetaPressed ? shortcutMetaKey : 0) |
      (keyboard.isAltPressed ? shortcutAltKey : 0) |
      (keyboard.isControlPressed ? shortcutControlKey : 0) |
      (keyboard.isShiftPressed ? shortcutShiftKey : 0);
}

/// Only identities and conditions are persisted, never a captured object.
class ShortcutRoute {
  const ShortcutRoute(
      {required this.actionId,
      required this.title,
      this.appBundleId,
      this.appName,
      this.contextType});
  final String actionId, title;
  final String? appBundleId, appName, contextType;
  String? get error {
    bool valid(String? value, int max) =>
        value == null ||
        (value.isNotEmpty &&
            value.length <= max &&
            RegExp(r'^[A-Za-z0-9._-]+$').hasMatch(value));
    if (actionId.isEmpty ||
        actionId.length > 2048 ||
        title.length > 512 ||
        !valid(appBundleId, 256) ||
        !valid(contextType, 128) ||
        (appName?.length ?? 0) > 512) {
      return 'Invalid shortcut rule.';
    }
    return null;
  }

  String get conditionLabel =>
      '${appName ?? (appBundleId == null ? 'Any app' : 'Saved app')} · ${contextType == null ? 'Any compatible context' : contextTypeLabel(contextType!)}';
  Map<String, Object> toMap() => {
        'actionId': actionId,
        'title': title,
        if (appBundleId != null) 'appBundleId': appBundleId!,
        if (appName != null) 'appName': appName!,
        if (contextType != null) 'contextType': contextType!
      };
  ShortcutRouteDto toDto() => ShortcutRouteDto(
      actionId: actionId, appBundleId: appBundleId, contextType: contextType);
  static ShortcutRoute? fromMap(Object? raw) {
    if (raw is! Map ||
        raw['actionId'] is! String ||
        raw['title'] is! String ||
        (raw['appBundleId'] != null && raw['appBundleId'] is! String) ||
        (raw['appName'] != null && raw['appName'] is! String) ||
        (raw['contextType'] != null && raw['contextType'] is! String)) {
      return null;
    }
    final route = ShortcutRoute(
        actionId: raw['actionId'],
        title: raw['title'],
        appBundleId: raw['appBundleId'],
        appName: raw['appName'],
        contextType: raw['contextType']);
    return route.error == null ? route : null;
  }
}

String contextTypeLabel(String id) =>
    const {
      'core.directory': 'Folder',
      'core.file': 'File',
      'core.file_selection': 'Selected files',
      'core.text': 'Text',
      'core.url': 'Web link',
      'core.json': 'JSON',
      'json.document': 'JSON',
      'git.repository': 'Git repository',
      'project.node': 'Node project',
      'project.maven': 'Maven project',
      'project.gradle': 'Gradle project',
      'project.rust': 'Rust project',
      'java.spring_boot': 'Spring Boot project',
      'core.application': 'Current app',
    }[id] ??
    id;
