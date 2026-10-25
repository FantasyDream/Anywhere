import 'package:flutter/foundation.dart';

import '../infrastructure/application_history_backend.dart';
import '../src/rust/api.dart';

/// Source preferences live beyond a mounted settings tab; closing waits for writes.
class AppHistorySettingsController extends ChangeNotifier {
  AppHistorySettingsController(this.backend);
  final AppHistoryPreferencesBackend backend;
  List<AppHistorySourceDto> sources = const [];
  bool loaded = false, busy = false, failed = false, _disposed = false;
  String customDirectory = '';
  Future<void> Function()? onCatalogChanged;
  Future<bool>? _pending;
  Future<bool> load() => _pending ?? _run();
  Future<bool> configure(String id, String path,
          {AppHistorySourceModeDto mode = AppHistorySourceModeDto.custom}) =>
      _run(() => backend.configure(id, path, mode: mode));
  Future<bool> setActionEnabled(String id, bool enabled) =>
      _run(() => backend.setActionEnabled(id, enabled));
  Future<bool> openCustomDirectory() => _run(backend.openCustomDirectory);
  Future<bool> waitForPending() async => await (_pending ?? Future.value(true));
  Future<bool> _run([Future<void> Function()? change]) {
    final previous = _pending ?? Future.value(true);
    late final Future<bool> next;
    next = previous.then((_) async {
      if (_disposed) return false;
      try {
        if (change != null) {
          try {
            await change();
          } finally {
            if (!_disposed) await onCatalogChanged?.call();
          }
        }
        customDirectory = await backend.customDirectory();
        final result = await backend.read();
        if (_disposed) return false;
        sources = result;
        loaded = true;
        failed = false;
        return true;
      } catch (_) {
        if (!_disposed) failed = true;
        return false;
      } finally {
        if (!_disposed && identical(_pending, next)) {
          busy = false;
          _pending = null;
          notifyListeners();
        }
      }
    });
    _pending = next;
    busy = true;
    notifyListeners();
    return next;
  }

  @override
  void dispose() {
    _disposed = true;
    super.dispose();
  }
}
