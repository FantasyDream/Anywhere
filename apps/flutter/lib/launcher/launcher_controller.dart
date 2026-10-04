import 'dart:async';
import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';
import '../src/rust/api.dart';
import '../infrastructure/application_icon_cache.dart';
import '../infrastructure/window_host.dart';
import '../settings/host_settings.dart';
import 'launcher_view_state.dart';
import 'context_source_status.dart';

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
typedef UpdateInput = Future<LauncherStateDto?>
    Function(int session, String input, int revision, {String? scopeId});
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
    Future<void> Function(HostSettings)? applyContextSources,
    List<PluginStatusDto> Function()? readPlugins,
  })  : _backend = backend,
        _window = window,
        _hostWarning = startupMessage,
        _readPlugins = readPlugins ?? pluginStatus,
        _applyExcludedApps = applyExcludedApps ??
            ((bundleIds) => setExcludedApps(bundleIds: bundleIds)),
        _applyContextSources = applyContextSources ??
            ((settings) => setContextSources(
                  finderSelection: settings.captureFinderSelection,
                  selectedText: settings.captureSelectedText,
                  browserCurrentPage: settings.captureBrowserCurrentPage,
                  clipboard: settings.captureClipboard,
                )),
        icons = ApplicationIconCache(backend.loadIcon) {
    icons.addListener(_notify);
  }

  final LauncherBackend _backend;
  final WindowHost _window;
  final ApplyExcludedApps _applyExcludedApps;
  final List<PluginStatusDto> Function() _readPlugins;
  final Future<void> Function(HostSettings) _applyContextSources;
  String _hostWarning;
  final ApplicationIconCache icons;
  LauncherPhase _phase = LauncherPhase.hidden;
  LauncherStateDto? _snapshot;
  List<ContextCaptureDto> _lastCaptures = const [];
  int? _session;
  int _generation = 0, _revision = 0, _requestedRevision = 0;
  String _input = '';
  String? _scopeId;
  List<String>? _actionPath;
  WakeMode _wakeMode = WakeMode.currentApp;
  bool _scopeTouched = false;
  String? _selectedId, _message, _output, _resultSource;
  ActionStatusDto? _resultStatus;
  HostSettings _settings = HostSettings.defaults;
  List<PluginStatusDto> _plugins = const [];
  bool _userMoved = false, _disposed = false;
  Future<bool> Function()? beforeSettingsClose;
  Future<bool>? _settingsClosing;
  int _iconFirst = 0;
  int _iconLast = LauncherViewState.maxRows;

  LauncherViewState get view => LauncherViewState(
        phase: _phase,
        generation: _generation,
        snapshot: _snapshot,
        input: _input,
        scopeId: _scopeId,
        actionPath: _actionPath,
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
  List<ContextSourceStatus> get contextSourceStatuses => [
        for (final source in ContextSourceStatus.sources)
          ContextSourceStatus(
              source,
              _lastCaptures
                  .where((capture) => capture.source == source)
                  .firstOrNull,
              enabled: switch (source) {
                ContextSourceDto.finderSelection =>
                  _settings.captureFinderSelection,
                ContextSourceDto.selectedText => _settings.captureSelectedText,
                ContextSourceDto.browserCurrentPage =>
                  _settings.captureBrowserCurrentPage,
                ContextSourceDto.clipboard => _settings.captureClipboard,
                _ => false,
              }),
      ];

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
      shutdownTasks: shutdownExecutionSessions,
      closeSettings: closeSettings,
      prepareToQuit: _prepareToQuit,
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
      if (!await closeSettings()) return;
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
    if (_phase == LauncherPhase.settings && !await closeSettings()) return;
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
      _actionPath = null;
      _snapshot = null;
      _lastCaptures = const [];
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
    _actionPath = null;
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
    _lastCaptures = List.unmodifiable(state.contextCaptures);
    _revision = state.revision;
    if (_scopeId != null && !view.scopes.any((scope) => scope.id == _scopeId)) {
      final removed = _scopeId;
      _scopeId = null;
      _actionPath = null;
      _scopeTouched = true;
      if (state.selectedScope == removed) {
        final generation = _generation;
        final revision = _requestedRevision;
        Future<void>.microtask(() async {
          if (_current(generation) &&
              _session == state.sessionId &&
              _requestedRevision == revision &&
              _scopeId == null) {
            await update(_input);
          }
        });
      }
    }
    // Menu permission can disappear while the aggregate Current App scope stays
    // available through Folder/Git actions. Return to its home in that case.
    if (_actionPath != null && !view.canBrowseMenus) _actionPath = null;
    _maybeSelectScope();
    _reconcile();
    _requestIcons();
  }

  void _reconcile() {
    final entries = view.visibleEntries;
    if (_userMoved && entries.any((entry) => entry.key == _selectedId)) {
      return;
    }
    _selectedId = entries.isEmpty ? null : entries.first.key;
    _userMoved = false;
  }

  void noteIconViewport(int first, int last) {
    if (first == _iconFirst && last == _iconLast) return;
    _iconFirst = first;
    _iconLast = last;
    _requestIcons();
  }

  void _requestIcons() {
    final entries = view.visibleEntries;
    if (entries.isEmpty) return;
    const prefetch = 4;
    final start = (_iconFirst - prefetch).clamp(0, entries.length);
    final end = (_iconLast + prefetch).clamp(start, entries.length);
    final candidates = <CandidateDto>[];
    for (final entry in entries.sublist(start, end)) {
      if (entry.candidate != null) candidates.add(entry.candidate!);
    }
    icons.request(candidates);
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
    _scopeTouched = true;
    _phase = LauncherPhase.resolving;
    _output = _message = null;
    _userMoved = false;
    icons.retryFailures();
    _notify();
    try {
      final state = await _backend.updateInput(session, input, revision,
          scopeId: _scopeId);
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
    final entries = view.visibleEntries;
    if (_phase != LauncherPhase.ready ||
        state == null ||
        state.revision != _revision) {
      return;
    }
    if (entries.isEmpty) return;
    final entry = entries[view.selectedIndex];
    if (entry.navigationPath != null) {
      _actionPath = entry.navigationPath;
      _selectedId = null;
      _refreshPresentation();
      await _syncWindow();
      return;
    }
    await _executeCandidate(entry.candidate!);
  }

  Future<void> _executeCandidate(CandidateDto candidate) async {
    final state = _snapshot;
    if (_phase != LauncherPhase.ready ||
        state == null ||
        state.revision != _revision) {
      return;
    }
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
      if (result.status == ActionStatusDto.succeeded &&
          result.executionSessionId != null &&
          result.warning == null) {
        try {
          await _window.showExecutionSessions(result.executionSessionId!);
          if (_current(generation) && revision == _revision) {
            await dismiss();
          }
        } catch (_) {
          if (!_current(generation) || revision != _revision) {
            return;
          }
          _message =
              'Task started. Open Running Tasks from the menu bar to view it.';
          _notify();
          await _syncWindow();
        }
      } else if (result.status == ActionStatusDto.succeeded &&
          result.closeWindow) {
        await dismiss();
      } else {
        _message = result.executionSessionId != null && result.warning != null
            ? 'Task started. ${result.warning} Open Running Tasks from the menu bar to view it.'
            : result.warning ?? result.error;
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
    final entries = view.visibleEntries;
    if (view.resolving || entries.isEmpty) return;
    _selectedId = entries[
            (view.selectedIndex + direction + entries.length) % entries.length]
        .key;
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
    unawaited(selectScope(scopes[next].id));
  }

  void _refreshPresentation({bool preserveSelection = false}) {
    _output = null;
    _resultSource = null;
    _resultStatus = null;
    _userMoved = preserveSelection;
    icons.retryFailures();
    _reconcile();
    _requestIcons();
    _notify();
  }

  Future<void> selectScope(String? id) async {
    if (id != null && !view.scopes.any((scope) => scope.id == id)) return;
    _scopeId = id;
    _scopeTouched = true;
    _actionPath = null;
    _selectedId = null;
    await update(_input);
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
    if (_phase == LauncherPhase.settings) {
      // Restore the existing window without rebuilding the draft or recording state.
      await _window.show(_generation, view.height, settings: true);
      return;
    }
    if (_phase != LauncherPhase.hidden) await dismiss();
    final generation = ++_generation;
    _phase = LauncherPhase.settings;
    try {
      _settings = await _window.readSettings();
      _plugins = _readPlugins();
    } catch (error) {
      _message = '$error';
    }
    if (!_current(generation) || _phase != LauncherPhase.settings) return;
    _notify();
    await _window.show(generation, view.height, settings: true);
  }

  /// Native close, Done, Escape and a Launcher hotkey share the same draft guard.
  Future<bool> closeSettings() {
    return _settingsClosing ??= _closeSettings().whenComplete(() {
      _settingsClosing = null;
    });
  }

  Future<bool> _closeSettings() async {
    if (_phase != LauncherPhase.settings) return true;
    final generation = _generation;
    if (!await (beforeSettingsClose?.call() ?? Future.value(true))) {
      return false;
    }
    if (!_current(generation) || _phase != LauncherPhase.settings) return false;
    await dismiss();
    return true;
  }

  Future<bool> _prepareToQuit() async {
    if (!await closeSettings()) return false;
    await shutdownExecutionSessions();
    return true;
  }

  void reportSettingsError(String message) {
    if (_disposed) return;
    _message = message;
    _notify();
  }

  Future<void> openPrivacySettings(String pane) async {
    if (_disposed) return;
    final generation = _generation;
    try {
      await _window.openPrivacySettings(pane);
      // The persistent settings window remains open while System Settings is shown.
      if (_current(generation) && _phase != LauncherPhase.settings) {
        await dismiss();
      }
    } catch (_) {
      if (!_current(generation)) return;
      _message =
          'System Settings could not be opened. Open it from the Apple menu.';
      _notify();
      await _syncWindow();
    }
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
    try {
      await _applyContextSources(_settings);
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
      await closeSettings();
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
    if (_actionPath != null) {
      backMenu();
      return;
    }
    if (_scopeId != null) {
      await selectScope(null);
      return;
    }
    await dismiss();
  }

  void _applyWakeMode(WakeMode mode) {
    _actionPath = null;
    _wakeMode = mode;
    _scopeTouched = true;
    if (mode == WakeMode.currentApp) _promptCurrentAppAccess(mode);
    final app = view.scopes.where((scope) => scope.isCurrentApp).firstOrNull;
    unawaited(selectScope(mode == WakeMode.currentApp ? app?.id : null));
  }

  void _maybeSelectScope() {
    if (_wakeMode != WakeMode.currentApp ||
        _scopeTouched ||
        _userMoved ||
        _actionPath != null ||
        _input.isNotEmpty) {
      return;
    }
    final app = view.scopes.where((scope) => scope.isCurrentApp).firstOrNull;
    if (app != null) _scopeId = app.id;
  }

  Future<void> selectAndExecute(String id) async {
    if (view.resolving ||
        !view.visibleCandidates.any((candidate) => candidate.id == id)) {
      return;
    }
    _selectedId = 'candidate:$id';
    _userMoved = true;
    _notify();
    await executeSelected();
  }

  Future<void> selectEntry(int index) async {
    final entries = view.visibleEntries;
    if (view.resolving || index < 0 || index >= entries.length) return;
    _selectedId = entries[index].key;
    _userMoved = true;
    _notify();
    await executeSelected();
  }

  Future<void> browseMenus() async {
    if (_phase != LauncherPhase.ready ||
        !view.canBrowseMenus ||
        _input.isNotEmpty) {
      return;
    }
    _actionPath = const [];
    _selectedId = null;
    _refreshPresentation();
    await _syncWindow();
  }

  void backMenu() {
    final path = _actionPath;
    if (path == null || _input.isNotEmpty) return;
    _actionPath = path.length <= 1 ? null : path.sublist(0, path.length - 1);
    _selectedId = null;
    _refreshPresentation();
    unawaited(_syncWindow());
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
