import 'package:flutter/services.dart';
import '../settings/host_settings.dart';

/// Typed lifecycle messages from the macOS host.
class WindowHost {
  WindowHost(this._channel);
  final MethodChannel _channel;

  void listen({
    required Future<void> Function([String? mode]) toggle,
    required Future<void> Function(int) dismiss,
    Future<void> Function()? openSettings,
    Future<void> Function()? shutdownTasks,
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
        case 'dismiss':
          final generation = (call.arguments as Map?)?['generation'];
          if (generation is int) await dismiss(generation);
        case 'openSettings':
          await openSettings?.call();
        case 'shutdownExecutionSessions':
          await shutdownTasks?.call();
      }
    });
  }

  Future<void> ready() => _channel.invokeMethod<void>('ready');
  Future<void> show(int generation, double height, {bool settings = false}) =>
      _channel.invokeMethod<void>('show', {
        'generation': generation,
        'height': height,
        'settings': settings,
      });
  Future<void> resize(int generation, double height) =>
      _channel.invokeMethod<void>('resize', {
        'generation': generation,
        'height': height,
      });
  Future<void> hide(int generation) =>
      _channel.invokeMethod<void>('hide', {'generation': generation});
  Future<void> openPrivacySettings(String pane) =>
      _channel.invokeMethod<void>('openPrivacySettings', {'pane': pane});
  Future<void> showExecutionSessions(String id) =>
      _channel.invokeMethod<void>('showExecutionSessions', {'id': id});

  Future<HostSettings> readSettings() async {
    final raw = await _channel.invokeMapMethod<Object?, Object?>('settings');
    if (raw == null) return HostSettings.defaults;
    return HostSettings.fromMap(raw);
  }

  Future<String> readSettingsStatus() async {
    final raw =
        await _channel.invokeMapMethod<Object?, Object?>('settingsStatus');
    if (raw == null) throw StateError('Settings status is unavailable.');
    return [raw['hotkeyError'], raw['loginError']]
        .whereType<String>()
        .where((message) => message.isNotEmpty)
        .join('\n');
  }

  Future<String?> writeSettings(HostSettings settings) async {
    try {
      await _channel.invokeMethod<void>('saveSettings', settings.toMap());
      return null;
    } on PlatformException catch (error) {
      return error.message ?? 'Settings could not be saved.';
    }
  }

  void dispose() => _channel.setMethodCallHandler(null);
}
