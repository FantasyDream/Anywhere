import 'dart:async';
import 'package:flutter/foundation.dart';
import '../src/rust/api.dart';
import '../infrastructure/application_icon_cache.dart';
import '../infrastructure/window_host.dart';
import 'launcher_view_state.dart';

export '../infrastructure/application_icon_cache.dart' show LoadIcon;

enum WakeMode { currentApp, universal }

WakeMode parseWakeMode(String? value) =>
    value == 'universal' ? WakeMode.universal : WakeMode.currentApp;

typedef BeginActivation = int Function();
typedef Activate = Future<LauncherStateDto?> Function(
  int session, {
  bool promptAccessibility,
});
typedef PromptAccessibility = bool Function();
typedef UpdateInput = Future<LauncherStateDto?> Function(
    int session, String input, int revision);
typedef Execute = Future<ActionResultDto> Function(
  int session,
  int revision,
  String candidate,
);
typedef WaitForSnapshot = Future<LauncherStateDto?> Function(
    int session, int after);
int snapshotCursor(LauncherStateDto state) =>
    (state.revision << 32) | state.snapshotSeq;

/// Owns asynchronous presentation lifecycle; matching and execution stay in Rust.
class LauncherController extends ChangeNotifier {
  LauncherController({
    required BeginActivation beginActivation,
    required Activate activate,
    required UpdateInput updateInput,
    required Execute execute,
    required Future<void> Function(int) deactivate,
    required WindowHost window,
    WaitForSnapshot? waitForSnapshot,
    LoadIcon? loadIcon,
    PromptAccessibility? promptAccessibility,
    String startupMessage = '',
  })  : _begin = beginActivation,
        _activate = activate,
        _update = updateInput,
        _execute = execute,
        _deactivate = deactivate,
        _window = window,
        _wait = waitForSnapshot,
        _promptAccessibility = promptAccessibility,
        _startupMessage = startupMessage,
        icons = ApplicationIconCache(loadIcon) {
    icons.addListener(_notify);
  }

  final BeginActivation _begin;
  final Activate _activate;
  final UpdateInput _update;
  final Execute _execute;
  final Future<void> Function(int) _deactivate;
  final WindowHost _window;
  final WaitForSnapshot? _wait;
  final PromptAccessibility? _promptAccessibility;
  final String _startupMessage;
  final ApplicationIconCache icons;
  LauncherPhase _phase = LauncherPhase.hidden;
  LauncherStateDto? _snapshot;
  int? _session;
  int _generation = 0, _revision = 0, _requestedRevision = 0;
  String _input = '';
  String? _scopeId;
  WakeMode _wakeMode = WakeMode.currentApp;
  bool _scopeTouched = false;
  String? _selectedId, _message, _output;
  bool _userMoved = false, _disposed = false;

  LauncherViewState get view => LauncherViewState(
        phase: _phase,
        generation: _generation,
        snapshot: _snapshot,
        input: _input,
        scopeId: _scopeId,
        selectedId: _selectedId,
        message: _message,
        output: _output,
      );

  void _notify() {
    if (!_disposed) notifyListeners();
  }

  bool _current(int generation) => !_disposed && generation == _generation;

  void attach() {
    _window.listen(
      toggle: toggle,
      dismiss: (generation) async {
        if (generation == _generation) await dismiss(hostAlreadyHidden: true);
      },
    );
    Future<void>.microtask(() async {
      if (!_disposed) await _window.ready();
    });
  }

  Future<void> toggle([String? mode]) {
    final parsed = parseWakeMode(mode);
    if (_phase == LauncherPhase.hidden) return activate(mode: parsed);
    if (_wakeMode == parsed) return dismiss();
    _applyWakeMode(parsed);
    return Future.value();
  }

  Future<void> activate({WakeMode mode = WakeMode.currentApp}) async {
    if (_disposed || _phase != LauncherPhase.hidden) return;
    final generation = ++_generation;
    _phase = LauncherPhase.activating;
    _notify();
    try {
      // Synchronous reservation closes the gap where a hidden host had no session
      // identity to cancel while foreground capture was still running.
      final session = _begin();
      _session = session;
      final state = await _activate(session, promptAccessibility: false);
      if (!_current(generation)) return;
      if (state == null || state.sessionId != session) {
        await dismiss();
        return;
      }
      _input = '';
      _snapshot = null;
      _selectedId = null;
      _userMoved = false;
      _wakeMode = mode;
      _scopeTouched = false;
      _scopeId = null;
      _revision = _requestedRevision = state.revision;
      _message = _startupMessage.isEmpty ? null : _startupMessage;
      _output = null;
      _phase = LauncherPhase.ready;
      _apply(state);
      _notify();
      await _syncWindow(show: true);
      if (_current(generation) && view.visible) {
        _promptCurrentAppAccess(mode);
        _notify();
        unawaited(_follow(generation, session));
      }
    } catch (error) {
      if (!_current(generation)) return;
      _message = '$error';
      _phase = LauncherPhase.ready;
      _notify();
      await _syncWindow(show: true);
    }
  }

  Future<void> dismiss({bool hostAlreadyHidden = false}) async {
    if (_disposed) return;
    final generation = _generation;
    final session = _session;
    ++_generation;
    _session = null;
    _snapshot = null;
    _phase = LauncherPhase.hidden;
    _input = '';
    _scopeId = null;
    _scopeTouched = false;
    _selectedId = _message = _output = null;
    _userMoved = false;
    icons.clear();
    _notify();
    // Begin cancellation before waiting on a platform message.
    final invalidation =
        session == null ? Future<void>.value() : _deactivate(session);
    try {
      if (!hostAlreadyHidden) await _window.hide(generation);
    } finally {
      await invalidation;
    }
  }

  void _apply(LauncherStateDto state) {
    if (state.sessionId != _session || state.revision < _requestedRevision) {
      return;
    }
    final held = _snapshot;
    if (held != null &&
        (state.revision < held.revision ||
            (state.revision == held.revision &&
                state.snapshotSeq <= held.snapshotSeq))) {
      return;
    }
    _snapshot = state;
    _revision = state.revision;
    _maybeSelectCurrentApp();
    _reconcile();
    icons.request(view.visibleCandidates);
  }

  void _reconcile() {
    final candidates = view.visibleCandidates;
    if (_userMoved &&
        candidates.any((candidate) => candidate.id == _selectedId)) {
      return;
    }
    _selectedId = candidates.isEmpty ? null : candidates.first.id;
    _userMoved = false;
  }

  Future<void> _follow(int generation, int session) async {
    final wait = _wait;
    if (wait == null) return;
    var cursor = _snapshot == null ? 0 : snapshotCursor(_snapshot!);
    try {
      while (_current(generation) && view.visible) {
        final next = await wait(session, cursor);
        if (!_current(generation) || !view.visible) return;
        if (next == null) continue;
        if (next.sessionId != session) return;
        final nextCursor = snapshotCursor(next);
        if (nextCursor <= cursor) continue;
        cursor = nextCursor;
        _apply(next);
        _notify();
        await _syncWindow();
      }
    } catch (error) {
      if (!_current(generation)) return;
      _message = 'Live updates stopped: $error';
      _notify();
      await _syncWindow();
    }
  }

  Future<void> update(String input) async {
    final session = _session;
    if (session == null || !view.visible) return;
    final revision = ++_revision;
    _requestedRevision = revision;
    final generation = _generation;
    _input = input;
    _phase = LauncherPhase.resolving;
    _output = _message = null;
    _userMoved = false;
    icons.retryFailures();
    _notify();
    try {
      final state = await _update(session, input, revision);
      if (!_current(generation) ||
          !view.visible ||
          revision != _revision ||
          state == null ||
          state.sessionId != session ||
          state.revision != revision) {
        return;
      }
      _selectedId = null;
      _apply(state);
      _phase = LauncherPhase.ready;
      _notify();
      await _syncWindow();
    } catch (error) {
      if (!_current(generation) || revision != _revision) return;
      _message = '$error';
      _phase = LauncherPhase.ready;
      _notify();
      await _syncWindow();
    }
  }

  Future<void> executeSelected() async {
    final state = _snapshot;
    final candidates = view.visibleCandidates;
    if (_phase != LauncherPhase.ready ||
        state == null ||
        candidates.isEmpty ||
        state.revision != _revision) {
      return;
    }
    final candidate = candidates[view.selectedIndex];
    if (candidate.availability != 'enabled') {
      _message = candidate.availabilityReason ??
          'This action is not available right now.';
      _notify();
      await _syncWindow();
      return;
    }
    final generation = _generation, revision = _revision;
    _phase = LauncherPhase.executing;
    _notify();
    try {
      final result = await _execute(state.sessionId, revision, candidate.id);
      if (!_current(generation) ||
          revision != _revision ||
          result.sessionId != state.sessionId ||
          result.revision != revision) {
        return;
      }
      if (result.success && result.closeWindow) {
        await dismiss();
      } else {
        _message = result.message == 'Done' ? null : result.message;
        _output = result.outputText;
        _notify();
        await _syncWindow();
      }
    } catch (error) {
      if (!_current(generation) || revision != _revision) return;
      _message = '$error';
      _notify();
      await _syncWindow();
    } finally {
      if (_current(generation) && revision == _revision) {
        _phase = LauncherPhase.ready;
        _notify();
      }
    }
  }

  void move(int direction) {
    final candidates = view.visibleCandidates;
    if (view.resolving || candidates.isEmpty) return;
    _selectedId =
        candidates[(view.selectedIndex + direction) % candidates.length].id;
    _userMoved = true;
    _output = null;
    _notify();
    unawaited(_syncWindow());
  }

  void cycleScope(int direction) {
    final scopes = view.scopes;
    if (scopes.isEmpty) return;
    final index = scopes.indexWhere((scope) => scope.id == _scopeId);
    final next = index < 0
        ? (direction >= 0 ? 0 : scopes.length - 1)
        : (index + direction + scopes.length) % scopes.length;
    selectScope(scopes[next].id);
  }

  void selectScope(String id) {
    if (!view.scopes.any((scope) => scope.id == id)) return;
    _scopeId = id;
    _scopeTouched = true;
    _output = null;
    _userMoved = false;
    icons.retryFailures();
    _reconcile();
    icons.request(view.visibleCandidates);
    _notify();
    unawaited(_syncWindow());
  }

  Future<void> escape() async {
    if (_input.isNotEmpty) {
      await update('');
      return;
    }
    if (_scopeId != null) {
      _scopeId = null;
      _scopeTouched = true;
      _output = null;
      _userMoved = false;
      icons.retryFailures();
      _reconcile();
      icons.request(view.visibleCandidates);
      _notify();
      await _syncWindow();
      return;
    }
    await dismiss();
  }

  void _applyWakeMode(WakeMode mode) {
    _wakeMode = mode;
    _scopeTouched = false;
    _output = null;
    _userMoved = false;
    if (mode == WakeMode.universal) {
      _scopeId = null;
    } else {
      _promptCurrentAppAccess(mode);
      if (view.scopes.any((scope) => scope.id == 'app')) {
        _scopeId = 'app';
      } else {
        _scopeId = null;
      }
    }
    icons.retryFailures();
    _reconcile();
    icons.request(view.visibleCandidates);
    _notify();
    unawaited(_syncWindow());
  }

  void _maybeSelectCurrentApp() {
    if (_wakeMode != WakeMode.currentApp || _scopeTouched) return;
    if (_input.trim().isNotEmpty) return;
    if (_scopeId != null) return;
    if (_snapshot?.scopes.any((scope) => scope.id == 'app') ?? false) {
      _scopeId = 'app';
    }
  }

  Future<void> selectAndExecute(String id) async {
    if (view.resolving ||
        !view.visibleCandidates.any((candidate) => candidate.id == id)) {
      return;
    }
    _selectedId = id;
    _userMoved = true;
    _notify();
    await executeSelected();
  }

  void _promptCurrentAppAccess(WakeMode mode) {
    if (mode != WakeMode.currentApp) return;
    try {
      _promptAccessibility?.call();
    } catch (_) {}
  }

  Future<void> _syncWindow({bool show = false}) async {
    if (_disposed || !view.visible) return;
    if (show) {
      await _window.show(_generation, view.height);
    } else {
      await _window.resize(_generation, view.height);
    }
  }

  @override
  void dispose() {
    _disposed = true;
    ++_generation;
    final session = _session;
    if (session != null) {
      unawaited(_deactivate(session).catchError((Object _) {}));
    }
    icons.removeListener(_notify);
    icons.dispose();
    _window.dispose();
    super.dispose();
  }
}
