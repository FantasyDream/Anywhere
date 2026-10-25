import 'package:flutter/foundation.dart';

import '../infrastructure/application_history_backend.dart';
import '../src/rust/api.dart';

/// Source preferences live beyond a mounted settings tab; closing waits for writes.
class AppHistorySettingsController extends ChangeNotifier {
  AppHistorySettingsController(this.backend);
  final AppHistoryPreferencesBackend backend;
  List<AppHistorySourceDto> sources = const [];
  bool loaded = false, busy = false, failed = false, _disposed = false;
  String? failedSource;
  String customDirectory = '';
  String filter = '';
  bool enabledOnly = false;
  final Map<String, String> _drafts = {};
  bool get hasDrafts => _drafts.isNotEmpty;
  bool hasDraft(String id) => _drafts.containsKey(id);
  String draftPath(AppHistorySourceDto source) =>
      _drafts[source.id] ?? source.path;
  Iterable<AppHistorySourceDto> get visibleSources {
    final needle = filter.trim().toLowerCase();
    return sources.where((s) =>
        (!enabledOnly || s.actionEnabled) &&
        '${s.title} ${s.id}'.toLowerCase().contains(needle));
  }

  void setFilter(String value, {bool? onlyEnabled}) {
    filter = value;
    enabledOnly = onlyEnabled ?? enabledOnly;
    notifyListeners();
  }

  void editPath(AppHistorySourceDto source, String path) {
    if (path == source.path) {
      _drafts.remove(source.id);
    } else {
      _drafts[source.id] = path;
    }
    notifyListeners();
  }

  void discardDrafts({String? source}) {
    if (source == null) {
      _drafts.clear();
    } else {
      _drafts.remove(source);
    }
    if (failedSource != null && (source == null || failedSource == source)) {
      failed = false;
      failedSource = null;
    }
    notifyListeners();
  }

  Future<bool> saveDrafts() async {
    final submitted = Map.of(_drafts);
    for (final entry in submitted.entries) {
      if (!await configure(entry.key, entry.value)) return false;
    }
    return true;
  }

  Future<void> Function()? onCatalogChanged;
  Future<bool>? _pending;
  Future<bool> load() => _pending ?? _run();
  Future<bool> configure(String id, String path,
      {AppHistorySourceModeDto mode = AppHistorySourceModeDto.custom}) {
    final replacedDraft =
        mode == AppHistorySourceModeDto.custom ? path : _drafts[id];
    return _run(
        change: () => backend.configure(id, path, mode: mode),
        source: id,
        onSuccess: () {
          if (_drafts[id] == replacedDraft) _drafts.remove(id);
        });
  }

  Future<bool> setActionEnabled(String id, bool enabled) =>
      _run(change: () => backend.setActionEnabled(id, enabled), source: id);
  Future<bool> openCustomDirectory() =>
      _run(change: backend.openCustomDirectory, changesCatalog: false);
  Future<bool> waitForPending() async => await (_pending ?? Future.value(true));
  Future<bool> _run(
      {Future<void> Function()? change,
      String? source,
      VoidCallback? onSuccess,
      bool changesCatalog = true}) {
    final previous = _pending ?? Future.value(true);
    late final Future<bool> next;
    next = previous.then((_) async {
      if (_disposed) return false;
      try {
        if (change != null) {
          try {
            await change();
          } finally {
            if (!_disposed && changesCatalog) await onCatalogChanged?.call();
          }
        }
        customDirectory = await backend.customDirectory();
        final result = await backend.read();
        if (_disposed) return false;
        sources = result;
        _drafts.removeWhere((id, path) => result.any((s) =>
            s.id == id &&
            s.mode == AppHistorySourceModeDto.custom &&
            s.path == path));
        onSuccess?.call();
        loaded = true;
        // A successful metadata read does not mean an invalid path draft was
        // saved. Reopening a tab must not erase that actionable failure.
        if (failedSource == null || !hasDraft(failedSource!)) {
          failed = false;
          failedSource = null;
        }
        return true;
      } catch (_) {
        if (!_disposed) {
          failed = true;
          failedSource = source;
        }
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
