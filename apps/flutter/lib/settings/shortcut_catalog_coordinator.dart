import 'package:flutter/services.dart';

import '../infrastructure/window_host.dart';
import '../src/rust/api.dart';
import 'host_settings.dart';

enum ShortcutCatalogFailure {
  refresh,
  registration;

  static ShortcutCatalogFailure from(Object error) =>
      error is PlatformException && error.code == 'shortcuts'
          ? registration
          : refresh;
}

/// Owns shortcut catalogs and serializes their native registration lifecycle.
class ShortcutCatalogCoordinator {
  ShortcutCatalogCoordinator({
    required DesktopHost window,
    required HostSettings Function() readSettings,
    required Future<void> Function(HostSettings, ShortcutCatalogFailure?)
        onRegistered,
    Future<List<ShortcutActionDto>> Function()? readActions,
    Future<List<String>> Function(List<String>)? readSupportedActions,
    Future<List<ShortcutApplicationDto>> Function()? readApplications,
  })  : _window = window,
        _readSettings = readSettings,
        _onRegistered = onRegistered,
        _readActions = readActions,
        _readSupportedActions = readSupportedActions,
        _readApplications = readApplications;

  final DesktopHost _window;
  final HostSettings Function() _readSettings;
  final Future<void> Function(HostSettings, ShortcutCatalogFailure?)
      _onRegistered;
  final Future<List<ShortcutActionDto>> Function()? _readActions;
  final Future<List<String>> Function(List<String>)? _readSupportedActions;
  final Future<List<ShortcutApplicationDto>> Function()? _readApplications;
  List<ShortcutActionDto> _actions = const [];
  List<ShortcutActionDto> get actions => _actions;
  List<ShortcutApplicationDto> _applications = const [];
  List<ShortcutApplicationDto> get applications => _applications;
  Set<String> _supported = const {};
  bool supports(String id) =>
      _readSupportedActions == null || _supported.contains(id);
  ShortcutCatalogFailure? _failure;
  ShortcutCatalogFailure? get failure => _failure;
  Future<void> _pending = Future.value();
  bool _disposed = false;

  Future<List<String>?> supportedActionsFor(HostSettings settings) async =>
      _readSupportedActions?.call(<String>{
        for (final binding in settings.actionShortcuts)
          if (binding.isIntent)
            ...binding.routes.map((r) => r.actionId)
          else
            binding.actionId,
      }.toList());

  Future<void> loadPickers(bool Function() isCurrent) async {
    await _loadActions(isCurrent);
    if (_disposed || !isCurrent() || _readApplications == null) return;
    final applications = await _readApplications();
    if (!_disposed && isCurrent()) _applications = applications;
  }

  Future<void> _loadActions(bool Function() isCurrent) async {
    if (_disposed || !isCurrent() || _readActions == null) return;
    final actions = await _readActions();
    if (!_disposed && isCurrent()) _actions = actions;
  }

  /// A changed feature catalog must not leave obsolete picker actions visible.
  Future<void> refreshActions() async {
    try {
      await _loadActions(() => true);
    } catch (_) {
      if (!_disposed) _actions = const [];
      rethrow;
    }
  }

  /// Read settings when the queued operation starts, then revalidate before
  /// native registration and status publication. Host also checks the snapshot.
  Future<void> synchronize() {
    if (_readSupportedActions == null || _disposed) return Future.value();
    final next = _pending.then((_) async {
      final snapshot = _readSettings();
      bool isCurrent() => !_disposed && identical(snapshot, _readSettings());
      if (!isCurrent() || snapshot.actionShortcutVersion != 4) return;
      final previousFailure = _failure;
      _failure = null;
      try {
        final supported = (await supportedActionsFor(snapshot))!;
        if (!isCurrent()) return;
        _supported = Set.unmodifiable(supported);
        await _window.syncActionShortcutCatalog(snapshot, supported);
        if (!isCurrent()) return;
        // Host warning refresh remains within this registration's queue slot.
        await _onRegistered(snapshot, previousFailure);
      } catch (error) {
        if (!isCurrent()) return;
        _failure = ShortcutCatalogFailure.from(error);
        _supported = const {};
        // Uncertain eligibility must not retain a revoked action's system chord.
        try {
          await _window.syncActionShortcutCatalog(snapshot, const []);
        } catch (_) {/* Keep the original failure for explicit recovery. */}
      }
    });
    _pending = next;
    return next;
  }

  void dispose() {
    _disposed = true;
  }
}
