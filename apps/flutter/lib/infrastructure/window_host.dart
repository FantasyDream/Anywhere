import 'package:flutter/services.dart';
import '../settings/host_settings.dart';
import '../settings/action_shortcut.dart';

/// Window and preferences port. Native channels, plugins, or test doubles
/// implement the same contract without exposing OS types to controllers.
abstract interface class DesktopHost {
  void listen({
    required Future<void> Function([String? mode]) toggle,
    required Future<void> Function(int) dismiss,
    Future<void> Function()? openSettings,
    Future<void> Function(String, ActionShortcut)? actionShortcut,
    Future<void> Function()? shutdownTasks,
    Future<bool> Function()? closeSettings,
    Future<bool> Function()? prepareToQuit,
  });

  Future<void> ready();
  Future<void> show(int generation, double height,
      {bool settings = false, double width = 648});
  Future<void> resize(int generation, double height, {double width = 648});
  Future<void> hide(int generation);
  Future<void> resumeActionShortcuts();
  Future<void> openPrivacySettings(String pane);
  Future<void> showExecutionSessions(String id);
  Future<HostSettings> readSettings();
  Future<String> readSettingsStatus();
  Future<void> syncActionShortcutCatalog(
      HostSettings settings, List<String> supportedIds);
  Future<String?> writeSettings(HostSettings settings,
      {List<String>? supportedActionIds});
  void dispose();
}

/// MethodChannel transport; method names and payloads are platform neutral.
class WindowHost implements DesktopHost {
  WindowHost(this._channel);
  final MethodChannel _channel;

  @override
  void listen({
    required Future<void> Function([String? mode]) toggle,
    required Future<void> Function(int) dismiss,
    Future<void> Function()? openSettings,
    Future<void> Function(String, ActionShortcut)? actionShortcut,
    Future<void> Function()? shutdownTasks,
    Future<bool> Function()? closeSettings,
    Future<bool> Function()? prepareToQuit,
  }) {
    _channel.setMethodCallHandler((call) async {
      switch (call.method) {
        case 'toggle':
          String? mode;
          final args = call.arguments;
          if (args is Map) {
            final value = args['mode'];
            if (value is String) mode = value;
          }
          await toggle(mode);
        case 'actionShortcut':
          final args = call.arguments as Map?;
          final id = args?['actionId'];
          final binding = ActionShortcut.fromMap(args?['binding']);
          if (id is String && binding != null && binding.actionId == id) {
            await actionShortcut?.call(id, binding);
          }
        case 'dismiss':
          final generation = (call.arguments as Map?)?['generation'];
          if (generation is int) await dismiss(generation);
        case 'openSettings':
          await openSettings?.call();
        case 'closeSettings':
          return await closeSettings?.call() ?? true;
        case 'prepareToQuit':
          return await prepareToQuit?.call() ?? true;
        case 'shutdownExecutionSessions':
          await shutdownTasks?.call();
      }
    });
  }

  @override
  Future<void> ready() => _channel.invokeMethod<void>('ready');
  @override
  Future<void> show(int generation, double height,
          {bool settings = false, double width = 648}) =>
      _channel.invokeMethod<void>('show', {
        'generation': generation,
        'height': height,
        'width': width,
        'settings': settings,
      });
  @override
  Future<void> resize(int generation, double height, {double width = 648}) =>
      _channel.invokeMethod<void>('resize', {
        'generation': generation,
        'height': height,
        'width': width,
      });
  @override
  Future<void> hide(int generation) =>
      _channel.invokeMethod<void>('hide', {'generation': generation});
  @override
  Future<void> resumeActionShortcuts() =>
      _channel.invokeMethod<void>('resumeActionShortcuts');
  @override
  Future<void> openPrivacySettings(String pane) =>
      _channel.invokeMethod<void>('openPrivacySettings', {'pane': pane});
  @override
  Future<void> showExecutionSessions(String id) =>
      _channel.invokeMethod<void>('showExecutionSessions', {'id': id});

  @override
  Future<HostSettings> readSettings() async {
    final raw = await _channel.invokeMapMethod<Object?, Object?>('settings');
    if (raw == null) return HostSettings.defaults;
    return HostSettings.fromMap(raw);
  }

  @override
  Future<String> readSettingsStatus() async {
    final raw =
        await _channel.invokeMapMethod<Object?, Object?>('settingsStatus');
    if (raw == null) throw StateError('Settings status is unavailable.');
    return [raw['hotkeyError'], raw['loginError'], raw['fileAccessError']]
        .whereType<String>()
        .where((message) => message.isNotEmpty)
        .join('\n');
  }

  @override
  Future<void> syncActionShortcutCatalog(
          HostSettings settings, List<String> supportedIds) =>
      _channel.invokeMethod<void>('syncActionShortcutCatalog', {
        'settings': settings.toMap(),
        'supportedActionIds': supportedIds,
      });

  @override
  Future<String?> writeSettings(HostSettings settings,
      {List<String>? supportedActionIds}) async {
    try {
      await _channel.invokeMethod<void>('saveSettings', {
        ...settings.toMap(),
        if (supportedActionIds != null)
          'supportedActionIds': supportedActionIds,
      });
      return null;
    } on PlatformException catch (error) {
      return error.message ?? 'Settings could not be saved.';
    }
  }

  @override
  void dispose() => _channel.setMethodCallHandler(null);
}
