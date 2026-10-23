import 'dart:async';

import 'package:flutter/foundation.dart';

import '../infrastructure/clipboard_preferences_backend.dart';
import '../l10n/app_language.dart';
import '../l10n/clipboard_labels.dart';
import '../src/rust/api.dart';

/// Owns asynchronous preferences operations, independently of the mounted tab.
class ClipboardSettingsController extends ChangeNotifier {
  ClipboardSettingsController(this.backend);
  final ClipboardPreferencesBackend backend;
  bool recording = false, available = false, loaded = false, busy = false;
  int? pausedUntilMs;
  int retentionDays = 0, storedEntries = 0, storedBytes = 0;
  Timer? _pauseRefresh;
  String? notice;
  ClipboardNoticeDto? _backendNotice;
  bool _operationFailed = false;
  String? noticeFor(AppLocalizations l10n) => _operationFailed
      ? l10n.clipboardSettingsFailed
      : _backendNotice?.labelFor(l10n) ?? notice;
  Future<bool>? _pending;
  bool _disposed = false;

  Future<bool> waitForPending() async => await (_pending ?? Future.value(true));
  Future<bool> load() => _run();
  Future<bool> setRecording(bool enabled) =>
      _run(() => backend.setRecording(enabled));
  Future<bool> clear() => _run(backend.clear);
  Future<bool> pause(int minutes) => _run(() => backend.pause(minutes));
  Future<bool> setRetention(int days) => _run(() => backend.setRetention(days));

  Future<bool> _run([Future<void> Function()? change]) {
    if (_disposed) return Future.value(false);
    if (_pending != null) return _pending!;
    // Publish the pending operation before notifying listeners: a close callback
    // or another observer must wait for this write and its readback too.
    final completion = Completer<bool>();
    final future = completion.future;
    _pending = future;
    busy = true;
    notifyListeners();
    unawaited(_apply(change).then((result) {
      if (identical(_pending, future)) _pending = null;
      completion.complete(result);
    }));
    return future;
  }

  Future<bool> _apply(Future<void> Function()? change) async {
    try {
      await change?.call();
      final state = await backend.read();
      if (_disposed) return false;
      recording = state.recordingEnabled;
      pausedUntilMs = state.pausedUntilMs;
      retentionDays = state.retentionDays;
      storedEntries = state.storedEntries;
      storedBytes = state.storedBytes;
      if (pausedUntilMs != null) {
        _pauseRefresh ??= Timer.periodic(const Duration(seconds: 2), (_) {
          if (!_disposed && !busy) unawaited(load());
        });
      } else {
        _pauseRefresh?.cancel();
        _pauseRefresh = null;
      }
      available = state.available;
      _operationFailed = false;
      notice = null;
      _backendNotice = state.notice;
      loaded = true;
      return true;
    } catch (_) {
      if (!_disposed) {
        // A write may have succeeded before readback failed. Do not show the old
        // value as an acknowledged preference or allow a second blind write.
        loaded = false;
        _operationFailed = true;
        notice =
            'Clipboard settings could not be loaded or updated. Try again.';
      }
      return false;
    } finally {
      if (!_disposed) {
        busy = false;
        notifyListeners();
      }
    }
  }

  @override
  void dispose() {
    _disposed = true;
    _pauseRefresh?.cancel();
    super.dispose();
  }
}
