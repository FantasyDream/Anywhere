import 'dart:convert';
import 'dart:ui' show Locale;

import 'package:flutter/services.dart';

import '../infrastructure/desktop_environment.dart';
import '../infrastructure/hotkey_keymap.dart';
import '../infrastructure/runtime_policy.dart';
import '../l10n/context_labels.dart';
import '../l10n/generated/app_localizations.dart';
import '../src/rust/api.dart';
import 'host_settings.dart';

export '../l10n/context_labels.dart';

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

  String? get chordError =>
      chordErrorFor(lookupAppLocalizations(const Locale('en')));
  String? chordErrorFor(AppLocalizations l10n) {
    final name = shortcutKeyNames[keyCode];
    if (actionId.isEmpty ||
        utf8.encode(actionId).length > RuntimePolicy.maxActionIdBytes ||
        utf8.encode(title).length > RuntimePolicy.maxActionTitleBytes) {
      return l10n.invalidActionIdentity;
    }
    if (name == null ||
        name.length != 1 ||
        !RegExp(r'^[A-Z]$').hasMatch(name)) {
      return l10n.letterWithModifier;
    }
    const allowed = shortcutMetaKey |
        shortcutShiftKey |
        shortcutAltKey |
        shortcutControlKey;
    if (modifiers & ~allowed != 0 ||
        modifiers & (shortcutMetaKey | shortcutAltKey | shortcutControlKey) ==
            0) {
      return l10n.modifierWithLetter;
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
      return l10n.reservedEditingShortcut;
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
  String? validate(HostSettings settings, [AppLocalizations? labels]) {
    final l10n = labels ?? lookupAppLocalizations(const Locale('en'));
    return configurationErrorFor(l10n) ?? validateChord(settings, l10n);
  }

  String? get configurationError =>
      configurationErrorFor(lookupAppLocalizations(const Locale('en')));
  String? configurationErrorFor(AppLocalizations l10n) {
    if (isIntent != actionId.startsWith('intent:')) {
      return l10n.invalidShortcutIntent;
    }
    if (!isIntent) return null;
    if (routes.length > RuntimePolicy.maxShortcutRoutes ||
        title.trim().isEmpty) {
      return l10n.shortcutRuleLimit;
    }
    final conditions = <String>{};
    for (final route in routes) {
      if (route.errorFor(l10n) case final error?) return error;
      if (!conditions
          .add('${route.appBundleId ?? ''}/${route.contextType ?? ''}')) {
        return l10n.differentShortcutConditions;
      }
    }
    return null;
  }

  String? validateChord(HostSettings settings, [AppLocalizations? labels]) {
    final l10n = labels ?? lookupAppLocalizations(const Locale('en'));
    if (chordErrorFor(l10n) case final error?) return error;
    if (sameChord(settings.currentAppKeyCode, settings.currentAppModifiers) ||
        sameChord(settings.universalKeyCode, settings.universalModifiers)) {
      return l10n.shortcutAlreadyOpens;
    }
    for (final other in settings.actionShortcuts) {
      if (other.actionId != actionId && other.sameChord(keyCode, modifiers)) {
        return l10n.shortcutAssignedTo(other.title);
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
  String? get error => errorFor(lookupAppLocalizations(const Locale('en')));
  String? errorFor(AppLocalizations l10n) {
    bool valid(String? value, int max) =>
        value == null ||
        (value.isNotEmpty &&
            value.length <= max &&
            RegExp(r'^[A-Za-z0-9._-]+$').hasMatch(value));
    if (actionId.isEmpty ||
        utf8.encode(actionId).length > RuntimePolicy.maxActionIdBytes ||
        utf8.encode(title).length > RuntimePolicy.maxActionTitleBytes ||
        !valid(appBundleId, 256) ||
        !valid(contextType, 128) ||
        utf8.encode(appName ?? '').length > RuntimePolicy.maxActionTitleBytes) {
      return l10n.invalidShortcutRule;
    }
    return null;
  }

  String get conditionLabel =>
      conditionLabelFor(lookupAppLocalizations(const Locale('en')));
  String conditionLabelFor(AppLocalizations l10n) =>
      '${appName ?? (appBundleId == null ? l10n.anyApp : l10n.savedApp)} · ${contextType == null ? l10n.anyCompatibleContext : contextTypeLabel(contextType!, l10n)}';
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
