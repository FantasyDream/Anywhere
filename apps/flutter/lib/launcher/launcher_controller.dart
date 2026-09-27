import 'dart:async';
import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';
import '../src/rust/api.dart';
import '../infrastructure/application_icon_cache.dart';
import '../infrastructure/window_host.dart';
import '../settings/host_settings.dart';
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
typedef ApplyExcludedApps = Future<void> Function(List<String> bundleIds);
typedef UpdateInput = Future<LauncherStateDto?> Function(
    int session, String input, int revision);
typedef Execute = Future<ActionResultDto> Function(
  int session,
  int revision,
  String candidate,
);
typedef WaitForSnapshot = Future<SnapshotWaitDto> Function(
    int session, int after);
int snapshotCursor(LauncherStateDto state) =>
    (state.revision << 32) | state.snapshotSeq;

class LauncherBackend {
  const LauncherBackend({
    required this.beginActivation,
    required this.activate,
    required this.updateInput,
    required this.execute,
    required this.deactivate,
    this.waitForSnapshot,
    this.loadIcon,
    this.promptAccessibility,
  });

  final BeginActivation beginActivation;
  final Activate activate;
  final UpdateInput updateInput;
  final Execute execute;
  final Future<void> Function(int) deactivate;
  final WaitForSnapshot? waitForSnapshot;
  final LoadIcon? loadIcon;
  final PromptAccessibility? promptAccessibility;
}

/// Owns asynchronous presentation lifecycle; matching and execution stay in Rust.
class LauncherController extends ChangeNotifier {
  LauncherController({
    required LauncherBackend backend,
    required WindowHost window,
    String startupMessage = '',
    ApplyExcludedApps? applyExcludedApps,
  })  : _backend = backend,
        _window = window,
        _hostWarning = startupMessage,
        _applyExcludedApps = applyExcludedApps ??
            ((bundleIds) => setExcludedApps(bundleIds: bundleIds)),
        icons = ApplicationIconCache(backend.loadIcon) {
    icons.addListener(_notify);
  }

  final LauncherBackend _backend;
  final WindowHost _window;
  final ApplyExcludedApps _applyExcludedApps;
  String _hostWarning;
  final ApplicationIconCache icons;
  LauncherPhase _phase = LauncherPhase.hidden;
  LauncherStateDto? _snapshot;
  int? _session;
  int _generation = 0, _revision = 0, _requestedRevision = 0;
  String _input = '';
  String? _scopeId;
  WakeMode _wakeMode = WakeMode.currentApp;
  bool _scopeTouched = false;
  String? _selectedId, _message, _output, _resultSource;
  ActionStatusDto? _resultStatus;
  HostSettings _settings = HostSettings.defaults;
  List<PluginStatusDto> _plugins = const [];
  bool _userMoved = false, _disposed = false;
  int _iconFirst = 0;
  int _iconLast = LauncherViewState.maxRows;

  LauncherViewState get view => LauncherViewState(
        phase: _phase,
        generation: _generation,
        snapshot: _snapshot,
        input: _input,
        scopeId: _scopeId,
        selectedId: _selectedId,
        message: _message,
        output: _output,
        resultSource: _resultSource,
        resultStatus: _resultStatus,
        universalShortcut: _settings.universalLabel,
      );

  HostSettings get hostSettings => _settings;
  String? get statusMessage => _message;
  List<PluginStatusDto> get plugins => _plugins;

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
      openSettings: openSettings,
    );
    Future<void>.microtask(() async {
      if (_disposed) return;
      try {
        _settings = await _window.readSettings();
        _notify();
      } catch (_) {}
      if (!_disposed) await _window.ready();
    });
  }

  Future<void> toggle([String? mode]) async {
    final parsed = parseWakeMode(mode);
    if (_phase == LauncherPhase.settings) {
      await dismiss();
      return activate(mode: parsed);
    }
    if (_phase == LauncherPhase.hidden) return activate(mode: parsed);
    if (_wakeMode == parsed) return dismiss();
    if (_phase == LauncherPhase.activating) {
      await dismiss();
      return activate(mode: parsed);
    }
    _applyWakeMode(parsed);
  }

  Future<void> activate({WakeMode mode = WakeMode.currentApp}) async {
    if (_disposed) return;
    if (_phase == LauncherPhase.settings) {
      await dismiss();
    }
    if (_phase != LauncherPhase.hidden) return;
    final generation = ++_generation;
    _phase = LauncherPhase.activating;
    _wakeMode = mode;
    _notify();
    try {
      // Synchronous reservation closes the gap where a hidden host had no session
      // identity to cancel while foreground capture was still running.
      final session = _backend.beginActivation();
      _session = session;
      final state =
          await _backend.activate(session, promptAccessibility: false);
      if (!_current(generation)) return;
      if (state == null || state.sessionId != session) {
        await dismiss();
        return;
      }
      _input = '';
      _snapshot = null;
      _selectedId = null;
      _userMoved = false;
      _scopeTouched = false;
      _scopeId = null;
      _revision = _requestedRevision = state.revision;
      _message = _hostWarning.isEmpty ? null : _hostWarning;
      _output = null;
      _resultSource = null;
      _resultStatus = null;
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
    _selectedId = _message = _output = _resultSource = null;
    _resultStatus = null;
    _userMoved = false;
    icons.clear();
    _notify();
    // Begin cancellation before waiting on a platform message.
    final invalidation =
        session == null ? Future<void>.value() : _backend.deactivate(session);
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
    // A late menu or permission update can remove the selected scope entirely.
    // Fall back to the full result list so unrelated actions remain reachable.
    if (_scopeId != null &&
        !state.scopes.any((scope) => scope.id == _scopeId)) {
      _scopeId = null;
    }
    _maybeSelectCurrentApp();
    _reconcile();
    _requestIcons();
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

  void noteIconViewport(int first, int last) {
    if (first == _iconFirst && last == _iconLast) return;
    _iconFirst = first;
    _iconLast = last;
    _requestIcons();
  }

  void _requestIcons() {
    final candidates = view.visibleCandidates;
    if (candidates.isEmpty) return;
    const prefetch = 4;
    final start = (_iconFirst - prefetch).clamp(0, candidates.length);
    final end = (_iconLast + prefetch).clamp(start, candidates.length);
    icons.request(candidates.sublist(start, end));
  }

  Future<void> _follow(int generation, int session) async {
    final wait = _backend.waitForSnapshot;
    if (wait == null) return;
    var cursor = _snapshot == null ? 0 : snapshotCursor(_snapshot!);
    try {
      while (_current(generation) && view.visible) {
        final notice = await wait(session, cursor);
        if (!_current(generation) || !view.visible) return;
        if (notice.kind == SnapshotWaitKindDto.timedOut) continue;
        if (notice.kind != SnapshotWaitKindDto.updated) return;
        final next = notice.state;
        if (next == null || next.sessionId != session) return;
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
      final state = await _backend.updateInput(session, input, revision);
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
    if (candidate.availability != AvailabilityDto.enabled) {
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
      final result =
          await _backend.execute(state.sessionId, revision, candidate.id);
      if (!_current(generation) ||
          revision != _revision ||
          result.sessionId != state.sessionId ||
          result.revision != revision) {
        return;
      }
      if (result.status == ActionStatusDto.succeeded && result.closeWindow) {
        await dismiss();
      } else {
        _message = result.warning ?? result.error;
        _output = result.outputText;
        _resultStatus = result.status;
        _resultSource = _sourceFor(candidate);
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
    _resultSource = null;
    _resultStatus = null;
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

  void _refreshPresentation() {
    _output = null;
    _resultSource = null;
    _resultStatus = null;
    _userMoved = false;
    icons.retryFailures();
    _reconcile();
    _requestIcons();
    _notify();
  }

  void selectScope(String id) {
    if (!view.scopes.any((scope) => scope.id == id)) return;
    _scopeId = id;
    _scopeTouched = true;
    _refreshPresentation();
    unawaited(_syncWindow());
  }

  void back() {
    if (_output == null) return;
    _output = null;
    _resultSource = null;
    _resultStatus = null;
    _notify();
    unawaited(_syncWindow());
  }

  String _sourceFor(CandidateDto candidate) {
    final subtitle = candidate.subtitle;
    if (subtitle != null && subtitle.isNotEmpty) return subtitle;
    return candidate.title;
  }

  Future<void> copyResult() async {
    final text = _output;
    if (text == null || text.isEmpty) return;
    await Clipboard.setData(ClipboardData(text: text));
    _message = 'Copied';
    _notify();
  }

  Future<void> openSettings() async {
    if (_disposed) return;
    if (_phase != LauncherPhase.hidden && _phase != LauncherPhase.settings) {
      await dismiss();
    }
    final generation = ++_generation;
    _phase = LauncherPhase.settings;
    try {
      _settings = await _window.readSettings();
      _plugins = pluginStatus();
    } catch (error) {
      _message = '$error';
    }
    _notify();
    await _window.show(generation, view.height, settings: true);
  }

  Future<bool> saveSettings(HostSettings next) async {
    final error = await _window.writeSettings(next);
    try {
      _settings = await _window.readSettings();
    } catch (_) {
      _settings = next;
    }
    try {
      _hostWarning = await _window.readSettingsStatus();
    } catch (_) {
      // Keep the last known warning when the host cannot report its status.
    }
    final failures = <String>[
      if (error != null && error.isNotEmpty) error,
    ];
    try {
      await _applyExcludedApps(_settings.excludedApps);
    } catch (failure) {
      failures.add('$failure');
    }
    if (failures.isEmpty) {
      _message = _hostWarning.isEmpty ? null : _hostWarning;
    } else {
      _message = failures.join('\n');
    }
    _notify();
    return failures.isEmpty;
  }

  Future<void> refreshPlugins() async {
    try {
      _plugins = await reloadPlugins();
    } catch (error) {
      _message = '$error';
    }
    _notify();
  }

  Future<void> escape() async {
    if (_phase == LauncherPhase.settings) {
      await dismiss();
      return;
    }
    if (_output != null) {
      back();
      return;
    }
    if (_input.isNotEmpty) {
      await update('');
      return;
    }
    if (_scopeId != null) {
      _scopeId = null;
      _scopeTouched = true;
      _refreshPresentation();
      await _syncWindow();
      return;
    }
    await dismiss();
  }

  void _applyWakeMode(WakeMode mode) {
    _wakeMode = mode;
    _scopeTouched = false;
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
    _refreshPresentation();
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
      _backend.promptAccessibility?.call();
    } catch (_) {}
  }

  Future<void> _syncWindow({bool show = false}) async {
    if (_disposed || !view.visible) return;
    if (show) {
      await _window.show(_generation, view.height,
          settings: _phase == LauncherPhase.settings);
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
      unawaited(_backend.deactivate(session).catchError((Object _) {}));
    }
    icons.removeListener(_notify);
    icons.dispose();
    _window.dispose();
    super.dispose();
  }
}
