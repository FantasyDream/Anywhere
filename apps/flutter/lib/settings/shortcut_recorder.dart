import '../infrastructure/desktop_environment.dart';
import 'dart:async';
import 'package:flutter/services.dart';

/// Host recording captures registered chords; Flutter is the portable fallback.
/// A cancelled or disposed dialog cannot accept a late native response.
class ShortcutRecorder {
  ShortcutRecorder({MethodChannel? channel})
      : _channel = channel ?? const MethodChannel('anywhere/shortcut-recorder'),
        _useNative = channel != null ||
            DesktopEnvironment.current.nativeShortcutRecording;
  final MethodChannel _channel;
  final bool _useNative;
  static int _nextId = 0;
  int _generation = 0;
  int? _requestId;
  bool get nativePending => _requestId != null;

  void start({
    required bool Function(int code, int modifiers) onChord,
    required void Function() onCancel,
    required void Function() onError,
  }) {
    cancel();
    if (!_useNative) return;
    final generation = _generation;
    unawaited(_listen(generation, onChord, onCancel, onError));
  }

  Future<void> _listen(int generation, bool Function(int, int) onChord,
      void Function() onCancel, void Function() onError) async {
    while (generation == _generation) {
      final id = ++_nextId;
      _requestId = id;
      try {
        final result = await _channel
            .invokeMapMethod<String, Object?>('record', {'id': id});
        if (generation != _generation) return;
        _requestId = null;
        if (result == null) {
          onCancel();
          return;
        }
        final code = result['keyCode'], modifiers = result['modifiers'];
        if (code is! int || modifiers is! int) {
          onError();
          return;
        }
        if (onChord(code, modifiers)) return;
      } on MissingPluginException {
        if (generation == _generation) _requestId = null;
        return;
      } catch (_) {
        if (generation == _generation) {
          _requestId = null;
          onError();
        }
        return;
      }
    }
  }

  void cancel() {
    _generation++;
    final id = _requestId;
    _requestId = null;
    if (id != null) unawaited(_cancel(id));
  }

  Future<void> _cancel(int id) async {
    try {
      await _channel.invokeMethod<void>('cancel', {'id': id});
    } catch (_) {
      // The window lifecycle also cancels native recording.
    }
  }
}
