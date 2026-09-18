import 'package:flutter/services.dart';

/// Typed lifecycle messages shared by the macOS and Windows hosts.
class WindowHost {
  WindowHost(this._channel);
  final MethodChannel _channel;

  void listen({
    required Future<void> Function([String? mode]) toggle,
    required Future<void> Function(int) dismiss,
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
      }
    });
  }

  Future<void> ready() => _channel.invokeMethod<void>('ready');
  Future<void> show(int generation, double height) => _channel
      .invokeMethod<void>('show', {'generation': generation, 'height': height});
  Future<void> resize(int generation, double height) =>
      _channel.invokeMethod<void>('resize', {
        'generation': generation,
        'height': height,
      });
  Future<void> hide(int generation) =>
      _channel.invokeMethod<void>('hide', {'generation': generation});
  void dispose() => _channel.setMethodCallHandler(null);
}
