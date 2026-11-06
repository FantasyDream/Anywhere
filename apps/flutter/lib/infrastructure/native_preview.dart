import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';

import '../src/rust/api.dart' as api;
import 'desktop_environment.dart';
import 'runtime_policy.dart';

/// Shared window port. Platform code renders; Rust supplies qualified resources.
/// A future Windows host implements this channel without changing page logic.
class NativePreviewController extends ChangeNotifier {
  NativePreviewController(
      {required this.onMove,
      required this.onActivate,
      required this.onSearch,
      required this.onFailure,
      Future<void> Function(String)? release,
      Future<void> Function(String)? retain})
      : _release = release ?? _releaseResource,
        _retain = retain ?? _retainResource;
  static const channel = MethodChannel('anywhere/preview');
  static NativePreviewController? _owner;
  static int _sequence = 0;
  final void Function(int) onMove;
  final VoidCallback onActivate, onSearch, onFailure;
  final Future<void> Function(String) _release;
  final Future<void> Function(String) _retain;
  static Future<void> _releaseResource(String id) => api.releasePreview(id: id);
  static Future<void> _retainResource(String id) => api.retainPreview(id: id);
  String? _selection;
  Future<api.NativePreviewDto> Function()? _prepare;
  api.NativePreviewDto? _resource;
  Timer? _timer;
  int _epoch = 0;
  int? _resourceEpoch;
  bool _preparing = false, _queued = false;
  bool _open = false, _disposed = false;
  bool get isOpen => _open;
  // Keep request stages available in the Host's stderr log in release builds.
  // Never record queries, paths, document content or raw exception messages.
  void _trace(String stage, [Map<String, Object?> fields = const {}]) {
    try {
      stderr.writeln(jsonEncode({
        'event': 'preview_$stage',
        'timestamp': DateTime.now().toUtc().toIso8601String(),
        'epoch': _epoch,
        'open': _open,
        'preparing': _preparing,
        ...fields
      }));
    } catch (_) {/* Diagnostics must not change the preview lifecycle. */}
  }

  // Search focus remains intact. Only an explicit list navigation arms Space.
  bool _browsing = false;
  bool get browsing => _browsing;
  set browsing(bool value) {
    if (_disposed || _browsing == value) return;
    _browsing = value;
    notifyListeners();
  }

  void sync(
      String? selection, Future<api.NativePreviewDto> Function()? prepare) {
    _prepare = prepare;
    if (_selection == selection) return;
    _selection = selection;
    _queued = false;
    ++_epoch;
    _timer?.cancel();
    if (selection == null) {
      close();
      return;
    }
    if (_open) {
      _timer = Timer(
          const Duration(
              milliseconds: RuntimePolicy.previewSelectionDebounceMs),
          _requestShow);
    }
  }

  void edited() {
    browsing = false;
    close();
  }

  bool handleKey(KeyEvent event,
      {required String query, bool composing = false}) {
    if (composing ||
        _disposed ||
        (event is! KeyDownEvent && event is! KeyRepeatEvent)) {
      return false;
    }
    final keys = HardwareKeyboard.instance;
    final plain = !keys.isMetaPressed &&
        !keys.isControlPressed &&
        !keys.isAltPressed &&
        !keys.isShiftPressed;
    final command = DesktopEnvironment.current.commandPressed &&
        !keys.isAltPressed &&
        !keys.isShiftPressed;
    final key = event.logicalKey;
    final previewShortcut = DesktopEnvironment.current.usesMetaCommand
        ? command && !keys.isControlPressed && key == LogicalKeyboardKey.keyY
        : keys.isAltPressed &&
            !keys.isMetaPressed &&
            !keys.isControlPressed &&
            !keys.isShiftPressed &&
            key == LogicalKeyboardKey.keyP;
    if (previewShortcut ||
        (plain &&
            key == LogicalKeyboardKey.space &&
            (_open || browsing || query.isEmpty))) {
      if (_prepare == null) return false;
      if (event is KeyDownEvent) toggle();
      return true;
    }
    if (_open &&
        ((plain && key == LogicalKeyboardKey.escape) ||
            (command && key == LogicalKeyboardKey.keyL))) {
      close();
      if (command) {
        browsing = false;
        onSearch();
      }
      return true;
    }
    if ((plain &&
            [
              LogicalKeyboardKey.arrowLeft,
              LogicalKeyboardKey.arrowRight,
              LogicalKeyboardKey.home,
              LogicalKeyboardKey.end,
              LogicalKeyboardKey.backspace,
              LogicalKeyboardKey.delete
            ].contains(key)) ||
        (command && key == LogicalKeyboardKey.keyA)) {
      browsing = false;
      close();
    }
    return false;
  }

  void toggle() {
    _trace('toggle', {'available': _prepare != null, 'browsing': browsing});
    if (_open) {
      close();
      return;
    }
    if (_prepare == null || _disposed) return;
    _owner?.close();
    _owner = this;
    _browsing = true;
    _open = true;
    channel.setMethodCallHandler((call) async {
      final owner = _owner;
      final args = call.arguments;
      if (owner == null || args is! Map || args['id'] != owner._resource?.id) {
        return;
      }
      switch (call.method) {
        case 'shown':
          try {
            owner._trace('retain');
            await owner._retain(owner._resource!.id);
            owner._trace('retained');
          } catch (error) {
            owner._trace('retain_failed', {'error': '${error.runtimeType}'});
            if (identical(_owner, owner) && owner._resource?.id == args['id']) {
              owner.close();
              owner.onFailure();
            }
          }
        case 'closed':
          owner._trace('native_closed');
          final failed = args['reason'] == 'show_failed';
          // A rejected old show must not cancel a newer queued selection.
          if (failed && owner._resourceEpoch != owner._epoch) return;
          owner.close();
          if (failed) owner.onFailure();
          if (args['restoreFocus'] == true) owner.onSearch();
        case 'next':
          owner.onMove(1);
        case 'previous':
          owner.onMove(-1);
        case 'activate':
          // A panel can still show the previous selection during preparation.
          if (owner._resourceEpoch != owner._epoch) return;
          owner.close();
          owner.onActivate();
        case 'search':
          owner.edited();
          owner.onSearch();
      }
    });
    notifyListeners();
    _requestShow();
  }

  void _requestShow() {
    if (!_open || _disposed) return;
    if (_preparing) {
      _queued = true;
      return;
    }
    unawaited(_show());
  }

  Future<void> _show() async {
    final prepare = _prepare;
    if (!_open || prepare == null || _disposed) return;
    _preparing = true;
    _queued = false;
    final epoch = _epoch;
    api.NativePreviewDto? next;
    api.NativePreviewDto? previous;
    var adopted = false;
    var stage = 'capabilities';
    try {
      _trace('request');
      final capabilities =
          await channel.invokeMapMethod<String, Object?>('capabilities');
      if (_disposed || !_open || epoch != _epoch) return;
      if (capabilities?['supported'] != true) throw UnsupportedError('preview');
      stage = 'prepare';
      next = await prepare();
      _trace('prepared', {'current': !_disposed && _open && epoch == _epoch});
      if (_disposed || !_open || epoch != _epoch) return;
      previous = _resource;
      _resource = next;
      _resourceEpoch = epoch;
      adopted = true;
      stage = 'show';
      await channel.invokeMethod<void>('show', {
        'id': next.id,
        'paths': next.paths,
        'title': next.title,
        'generation': ++_sequence,
      });
      _trace(
          'show_returned', {'current': !_disposed && _open && epoch == _epoch});
      if (_disposed || !_open || epoch != _epoch) return;
      next = null; // The active controller now owns the lease.
    } catch (error) {
      _trace('failed', {
        'stage': stage,
        'error':
            error is PlatformException ? error.code : '${error.runtimeType}'
      });
      if (!_disposed && _open && epoch == _epoch) {
        close();
        onFailure();
      }
    } finally {
      if (previous != null && _resource?.id != previous.id) {
        await _retire(previous.id);
      }
      if (next != null && !adopted) await _retire(next.id);
      _preparing = false;
      if (_queued) _requestShow();
    }
  }

  Future<void> _retire(String id) async {
    try {
      await _release(id);
    } catch (_) {/* Never make closing depend on storage. */}
  }

  void close() {
    if (_open) _trace('close');
    ++_epoch;
    _timer?.cancel();
    _queued = false;
    _resourceEpoch = null;
    final resource = _resource;
    _resource = null;
    if (identical(_owner, this)) {
      _owner = null;
      unawaited(channel.invokeMethod<void>(
          'close', {'generation': ++_sequence}).catchError((Object _) {}));
    }
    final wasOpen = _open;
    _open = false;
    if (resource != null) unawaited(_retire(resource.id));
    if (!_disposed && wasOpen) notifyListeners();
  }

  @override
  void dispose() {
    _disposed = true;
    close();
    super.dispose();
  }
}
