import 'dart:async';

import 'package:flutter/foundation.dart';

import '../infrastructure/bridge_api.dart';
import '../infrastructure/clipboard_preferences_backend.dart';
import '../l10n/app_language.dart';
import '../l10n/clipboard_labels.dart';

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
  bool _visible = false;

  void setVisible(bool visible) {
    if (_disposed || _visible == visible) return;
    _visible = visible;
    _updateRefresh();
    if (visible) unawaited(load());
  }

  void _updateRefresh() {
    _pauseRefresh?.cancel();
    _pauseRefresh = null;
    if (_visible && pausedUntilMs != null && loaded) {
      final remaining = pausedUntilMs! - DateTime.now().millisecondsSinceEpoch;
      // The UI displays a deadline, not a ticking counter. Rust owns resuming.
      // Re-read near that deadline; failed native recovery retries only while visible.
      _pauseRefresh = Timer(
          Duration(milliseconds: remaining <= 0 ? 2000 : remaining + 250), () {
        _pauseRefresh = null;
        if (!_disposed && _visible) unawaited(load());
      });
    }
  }

  Future<bool> waitForPending() => _pending ?? Future.value(!_operationFailed);
  Future<bool> load() => _run();
  Future<bool> setRecording(bool enabled) =>
      _run(() => backend.setRecording(enabled));
  Future<bool> clear() => _run(backend.clear);
  Future<bool> pause(int minutes) => _run(() => backend.pause(minutes));
  Future<bool> setRetention(int days) => _run(() => backend.setRetention(days));

  Future<bool> _run([Future<void> Function()? change]) {
    if (_disposed) return Future.value(false);
    if (change == null && _pending != null) return _pending!;
    final previous = _pending ?? Future.value(true);
    late final Future<bool> next;
    next = previous.then((_) async {
      final result = _disposed ? false : await _apply(change);
      if (identical(_pending, next)) {
        _pending = null;
        busy = false;
      }
      if (!_disposed) notifyListeners();
      return result;
    });
    // Close callbacks triggered by busy must also wait for this write's readback.
    _pending = next;
    busy = true;
    notifyListeners();
    return next;
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
      available = state.available;
      _operationFailed = false;
      notice = null;
      _backendNotice = state.notice;
      loaded = true;
      _updateRefresh();
      return true;
    } catch (_) {
      if (!_disposed) {
        // A write may have succeeded before readback failed. Do not show the old
        // value as an acknowledged preference or allow a second blind write.
        loaded = false;
        _updateRefresh();
        _operationFailed = true;
        notice =
            'Clipboard settings could not be loaded or updated. Try again.';
      }
      return false;
    }
  }

  @override
  void dispose() {
    _disposed = true;
    _pauseRefresh?.cancel();
    super.dispose();
  }
}
