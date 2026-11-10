import 'dart:async';

import '../infrastructure/bridge_api.dart';

/// Owns feature status and mutation admission, including source reconciliation.
class FeatureCatalogCoordinator {
  FeatureCatalogCoordinator({
    required FutureOr<List<ExtensionStatusDto>> Function() read,
    required Future<List<ExtensionStatusDto>> Function() refresh,
    required Future<List<ExtensionStatusDto>> Function(String, bool) enable,
    required Future<List<ExtensionStatusDto>> Function(String, String) approve,
    required Future<List<ExtensionStatusDto>> Function(String) revoke,
    required void Function() invalidateReads,
    required void Function() onStarting,
    required void Function(Object) onFailure,
    required Future<void> Function() reconcileShortcuts,
    required void Function() onChanged,
  })  : _read = read,
        _refresh = refresh,
        _enable = enable,
        _approve = approve,
        _revoke = revoke,
        _invalidateReads = invalidateReads,
        _onStarting = onStarting,
        _onFailure = onFailure,
        _reconcileShortcuts = reconcileShortcuts,
        _onChanged = onChanged;

  final FutureOr<List<ExtensionStatusDto>> Function() _read;
  final Future<List<ExtensionStatusDto>> Function() _refresh;
  final Future<List<ExtensionStatusDto>> Function(String, bool) _enable;
  final Future<List<ExtensionStatusDto>> Function(String, String) _approve;
  final Future<List<ExtensionStatusDto>> Function(String) _revoke;
  final void Function() _invalidateReads, _onStarting, _onChanged;
  final void Function(Object) _onFailure;
  final Future<void> Function() _reconcileShortcuts;
  List<ExtensionStatusDto> _statuses = const [];
  List<ExtensionStatusDto> get statuses => _statuses;
  Future<void>? _pending;
  bool get busy => _pending != null;
  bool _disposed = false;

  Future<void> load(bool Function() isCurrent) async {
    if (_disposed || !isCurrent()) return;
    final statuses = await _read();
    if (!_disposed && isCurrent()) _statuses = statuses;
  }

  Future<void> refresh() => _change(_refresh);
  Future<void> enable(ExtensionStatusDto extension, bool enabled) =>
      _change(() => _enable(extension.key, enabled));
  Future<void> approve(ExtensionStatusDto extension) =>
      _change(() => _approve(extension.key, extension.approvalToken));
  Future<void> revoke(ExtensionStatusDto extension) =>
      _change(() => _revoke(extension.key));

  Future<void> _change(FutureOr<List<ExtensionStatusDto>> Function() change) =>
      busy || _disposed ? Future.value() : _queue(change);

  /// Already committed source writes must reconcile even during another change
  /// or while close is draining the queue. They do not open new UI mutations.
  Future<void> reconcileSource() => _queue(_read);

  Future<void> _queue(FutureOr<List<ExtensionStatusDto>> Function() operation) {
    if (_disposed) return Future.value();
    _invalidateReads();
    final previous = _pending ?? Future<void>.value();
    late final Future<void> pending;
    pending = previous.then((_) async {
      if (!_disposed) await _update(operation);
    }).whenComplete(() {
      if (identical(_pending, pending)) {
        _pending = null;
        if (!_disposed) _onChanged();
      }
    });
    _pending = pending;
    _onChanged();
    return pending;
  }

  Future<void> _update(
      FutureOr<List<ExtensionStatusDto>> Function() operation) async {
    _invalidateReads();
    _onStarting();
    if (_disposed) return;
    try {
      final statuses = await operation();
      if (_disposed) return;
      _statuses = statuses;
      _invalidateReads();
    } catch (error) {
      if (!_disposed) _onFailure(error);
    } finally {
      // A backend may commit before its response fails. Always reconcile its
      // shortcut eligibility, including unregistering uncertain system chords.
      if (!_disposed) await _reconcileShortcuts();
      if (!_disposed) _onChanged();
    }
  }

  Future<void> waitForPending() async {
    while (!_disposed) {
      final pending = _pending;
      if (pending == null) return;
      await pending;
      // A source callback may have appended work while this tail was awaited.
    }
  }

  void dispose() {
    _disposed = true;
  }
}
