import 'activation_prefill.dart';
import 'dart:async';
import 'dart:convert';
import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';
import '../src/rust/api.dart';
import '../infrastructure/application_icon_cache.dart';
import '../infrastructure/window_host.dart';
import '../settings/host_settings.dart';
import '../settings/settings_controller.dart';
import '../settings/action_shortcut.dart';
import '../infrastructure/application_lifecycle.dart';
import 'launcher_view_state.dart';
import 'context_source_status.dart';

export '../infrastructure/application_icon_cache.dart' show LoadIcon;
export '../settings/settings_controller.dart' show ApplyExcludedApps;

enum WakeMode { currentApp, universal }

WakeMode parseWakeMode(String? value) =>
    value == 'universal' ? WakeMode.universal : WakeMode.currentApp;

typedef BeginActivation = int Function();
typedef Activate = Future<LauncherStateDto?> Function(
  int session, {
  bool promptAccessibility,
  bool capturedEnvironment,
});
typedef PromptAccessibility = bool Function();
typedef UpdateInput = Future<LauncherStateDto?> Function(
    int session, String input, int revision,
    {String? scopeId, InputModeDto inputMode});
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
    this.shortcutActions,
    this.supportedShortcutActions,
    this.shortcutApplications,
    this.resolveIntent,
    this.waitIntent,
    this.resolveBinding,
    this.waitBinding,
  });

  final BeginActivation beginActivation;
  final Activate activate;
  final UpdateInput updateInput;
  final Execute execute;
  final Future<void> Function(int) deactivate;
  final WaitForSnapshot? waitForSnapshot;
  final LoadIcon? loadIcon;
  final PromptAccessibility? promptAccessibility;
  final Future<List<ShortcutActionDto>> Function()? shortcutActions;
  final Future<List<String>> Function(List<String>)? supportedShortcutActions;
  final Future<List<ShortcutApplicationDto>> Function()? shortcutApplications;
  final Future<BindingResolutionDto> Function(
      int session, int revision, List<ShortcutRouteDto> routes)? resolveIntent;
  final Future<BindingResolutionDto> Function(
      int session, int revision, List<ShortcutRouteDto> routes)? waitIntent;
  final Future<List<CandidateDto>> Function(
      int session, int revision, String action)? resolveBinding;
  final Future<BindingResolutionDto> Function(
      int session, int revision, String action)? waitBinding;
}

/// Owns asynchronous presentation lifecycle; matching and execution stay in Rust.
class LauncherController extends ChangeNotifier {
  LauncherController({
    required LauncherBackend backend,
    required DesktopHost window,
    String startupMessage = '',
    ApplyExcludedApps? applyExcludedApps,
    Future<void> Function(HostSettings)? applyContextSources,
    List<PluginStatusDto> Function()? readPlugins,
    Future<void> Function()? shutdownTasks,
    int Function()? nowMillis,
  })  : _backend = backend,
        _window = window,
        _nowMillis = nowMillis ?? (() => DateTime.now().millisecondsSinceEpoch),
        settings = SettingsController(
            window: window,
            startupMessage: startupMessage,
            applyExcludedApps: applyExcludedApps,
            applyContextSources: applyContextSources,
            readPlugins: readPlugins,
            readShortcutActions: backend.shortcutActions,
            readSupportedShortcutActions: backend.supportedShortcutActions,
            readShortcutApplications: backend.shortcutApplications),
        icons = ApplicationIconCache(backend.loadIcon) {
    settings.addListener(_notify);
    lifecycle = ApplicationLifecycle(
        closeSettings: closeSettings,
        cancelLauncher: () async {
          if (_phase != LauncherPhase.hidden) await dismiss();
        },
        shutdownTasks: shutdownTasks ?? shutdownExecutionSessions);
  }

  final LauncherBackend _backend;
  final DesktopHost _window;
  final int Function() _nowMillis;
  InputModeDto _inputMode = InputModeDto.environment;
  InputModeDto _snapshotInputMode = InputModeDto.environment;
  ContextSourceDto? _contentSource;
  bool _prefillAllowed = false,
      _prefillGracePassed = false,
      _pendingAppHome = false;
  Timer? _prefillTimer;
  Future<void>? _prefillUpdate;
  final SettingsController settings;
  late final ApplicationLifecycle lifecycle;
  final ApplicationIconCache icons;
  LauncherPhase _phase = LauncherPhase.hidden;
  LauncherStateDto? _snapshot;
  int? _session;
  int _generation = 0, _revision = 0, _requestedRevision = 0;
  String _input = '';
  String? _scopeId;
  List<String>? _actionPath;
  List<CandidateDto>? _bindingChoices;
  int _bindingRequest = 0;
  bool _globalWaiting = false;
  WakeMode _wakeMode = WakeMode.currentApp;
  bool _scopeTouched = false;
  String? _selectedId, _message, _output, _resultSource;
  ActionStatusDto? _resultStatus;
  bool _userMoved = false, _disposed = false;
  Future<bool>? _settingsClosing;
  int _iconFirst = 0;
  int _iconLast = LauncherViewState.maxRows;

  Record? _viewKey;
  LauncherViewState? _cachedView;

  LauncherViewState get view {
    // Include every presentation input. Changes before notifyListeners must also
    // invalidate the view (reconciliation and icon prefetch read it internally).
    final key = (
      _phase,
      _generation,
      _snapshot,
      _input,
      _inputMode,
      _snapshotInputMode,
      _contentSource,
      _scopeId,
      _actionPath,
      _selectedId,
      _message,
      _output,
      _resultSource,
      _resultStatus,
      settings.current.universalLabel,
      _bindingChoices
    );
    if (key == _viewKey && _cachedView != null) return _cachedView!;
    _viewKey = key;
    return _cachedView = LauncherViewState(
      phase: _phase,
      generation: _generation,
      snapshot: _snapshot,
      input: _input,
      inputMode: _inputMode,
      inputModeReady: _snapshotInputMode == _inputMode,
      contentSource: _contentSource,
      scopeId: _scopeId,
      actionPath: _actionPath,
      selectedId: _selectedId,
      message: _message,
      output: _output,
      resultSource: _resultSource,
      resultStatus: _resultStatus,
      universalShortcut: settings.current.universalLabel,
      bindingChoices: _bindingChoices,
    );
  }

  HostSettings get hostSettings => settings.current;
  String? get statusMessage =>
      _phase == LauncherPhase.settings ? settings.statusMessage : _message;
  List<PluginStatusDto> get plugins => settings.plugins;
  List<ContextSourceStatus> get contextSourceStatuses =>
      settings.contextSourceStatuses;

  void _notify() {
    if (!_disposed) notifyListeners();
  }

  bool _current(int generation) => !_disposed && generation == _generation;

  void attach() {
    _window.listen(
      toggle: toggle,
      actionShortcut: (id, binding) =>
          triggerGlobalShortcut(id, requestedBinding: binding),
      dismiss: (generation) async {
        if (generation == _generation) await dismiss(hostAlreadyHidden: true);
      },
      openSettings: openSettings,
      shutdownTasks: lifecycle.shutdownTasks,
      closeSettings: closeSettings,
      prepareToQuit: lifecycle.prepareToQuit,
    );
    Future<void>.microtask(() async {
      if (_disposed) return;
      try {
        await settings.load();
        _notify();
      } catch (_) {}
      if (!_disposed) await _window.ready();
    });
  }

  Future<void> toggle([String? mode]) async {
    if (lifecycle.isQuitting) return;
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

  /// Native global events carry only business identity, never cached object data.
  bool _bindingIsCurrent(ActionShortcut binding) =>
      settings.current.actionShortcuts.any((b) =>
          b.actionId == binding.actionId &&
          jsonEncode(b.toMap()) == jsonEncode(binding.toMap()));

  Future<void> triggerGlobalShortcut(String actionId,
      {ActionShortcut? requestedBinding}) async {
    if (_disposed || lifecycle.isQuitting || _phase == LauncherPhase.settings) {
      return;
    }
    final binding = settings.current.actionShortcuts
        .where((b) =>
            b.actionId == actionId &&
            b.global &&
            b.validate(settings.current) == null)
        .firstOrNull;
    if (binding == null ||
        (requestedBinding != null && !_bindingIsCurrent(requestedBinding))) {
      return; // Removed/edited queued event.
    }
    if (_phase == LauncherPhase.hidden) {
      if (binding.isIntent
          ? _backend.waitIntent == null
          : _backend.waitBinding == null) {
        return;
      }
      await activate(mode: WakeMode.universal, globalBinding: binding);
    } else if (_phase == LauncherPhase.ready) {
      await executeShortcut(binding);
    }
  }

  Future<void> activate(
      {WakeMode mode = WakeMode.currentApp,
      ActionShortcut? globalBinding}) async {
    if (_disposed || lifecycle.isQuitting) return;
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
      final state = await _backend.activate(session,
          promptAccessibility: false,
          capturedEnvironment: globalBinding != null);
      if (!_current(generation)) return;
      if (state == null || state.sessionId != session) {
        await dismiss();
        return;
      }
      _input = '';
      _inputMode = InputModeDto.environment;
      _contentSource = null;
      _prefillUpdate = null;
      _pendingAppHome = false;
      _prefillAllowed = mode == WakeMode.currentApp && globalBinding == null;
      _prefillGracePassed = false;
      _prefillTimer?.cancel();
      if (_prefillAllowed) {
        _prefillTimer = Timer(const Duration(milliseconds: 500), () {
          if (_current(generation)) {
            _prefillGracePassed = true;
            _maybePrefill();
          }
        });
      }
      _actionPath = null;
      _snapshot = null;
      settings.recordCaptures(const []);
      _selectedId = null;
      _userMoved = false;
      _scopeTouched = false;
      _scopeId = null;
      _revision = _requestedRevision = state.revision;
      _message = settings.warning.isEmpty ? null : settings.warning;
      _output = null;
      _resultSource = null;
      _resultStatus = null;
      _phase =
          globalBinding == null ? LauncherPhase.ready : LauncherPhase.resolving;
      _globalWaiting = globalBinding != null;
      if (_globalWaiting) {
        _message = 'Capturing context for ${globalBinding!.title}…';
      }
      _apply(state);
      if (_prefillUpdate != null) {
        await _prefillUpdate;
      }
      if (!_current(generation)) return;
      if (mode == WakeMode.universal && globalBinding == null) {
        _inputMode = InputModeDto.search;
        await update('', automatic: true);
      }
      _notify();
      await _syncWindow(show: true);
      if (_current(generation) && view.visible) {
        if (globalBinding == null) _promptCurrentAppAccess(mode);
        _notify();
        unawaited(_follow(generation, session));
        if (globalBinding != null) {
          await _completeGlobalBinding(
              globalBinding, generation, state.revision);
        }
      }
    } catch (error) {
      if (!_current(generation)) return;
      _message = '$error';
      _phase = LauncherPhase.ready;
      _notify();
      await _syncWindow(show: true);
    }
  }

  Future<void> _completeGlobalBinding(
      ActionShortcut binding, int generation, int revision) async {
    bool current() =>
        _current(generation) &&
        _globalWaiting &&
        _requestedRevision == revision &&
        _phase == LauncherPhase.resolving;
    try {
      final resolved = binding.isIntent
          ? await _backend.waitIntent!(_session!, revision,
              binding.routes.map((r) => r.toDto()).toList())
          : await _backend.waitBinding!(_session!, revision, binding.actionId);
      if (!current() ||
          resolved.state.sessionId != _session ||
          resolved.state.revision != revision) {
        return;
      }
      if (!_bindingIsCurrent(binding)) {
        _globalWaiting = false;
        _phase = LauncherPhase.ready;
        _message = 'Shortcut settings changed. Try again.';
        _notify();
        await _syncWindow();
        return;
      }
      _globalWaiting = false;
      _apply(resolved.state);
      _phase = LauncherPhase.ready;
      _message = null;
      await _presentBinding(binding, resolved.candidates,
          requiresChoice: resolved.requiresChoice);
    } catch (error) {
      if (!current()) return;
      _globalWaiting = false;
      _phase = LauncherPhase.ready;
      _message = '$error';
      _notify();
      await _syncWindow();
    }
  }

  Future<void> dismiss({bool hostAlreadyHidden = false}) async {
    if (_disposed) return;
    final generation = _generation;
    final session = _session;
    ++_generation;
    _globalWaiting = false;
    _session = null;
    _snapshot = null;
    _cachedView = null;
    _viewKey = null;
    _bindingChoices = null;
    ++_bindingRequest;
    _phase = LauncherPhase.hidden;
    _input = '';
    _prefillAllowed = false;
    _prefillTimer?.cancel();
    _contentSource = null;
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
    _snapshotInputMode = _inputMode;
    _bindingChoices = null;
    ++_bindingRequest;
    settings.recordCaptures(state.contextCaptures);
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
    if (_pendingAppHome) {
      final app = view.scopes.where((scope) => scope.isCurrentApp).firstOrNull;
      if (app != null) {
        _scopeId = app.id;
        _pendingAppHome = false;
      }
    }
    _maybeSelectScope();
    _maybePrefill();
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

  Future<void> update(String input, {bool automatic = false}) async {
    final session = _session;
    if (session == null || !view.visible) return;
    final revision = ++_revision;
    _requestedRevision = revision;
    final generation = _generation;
    _input = input;
    if (!automatic) {
      _prefillAllowed = false;
      _prefillTimer?.cancel();
      _pendingAppHome = false;
      _scopeTouched = true;
    }
    _globalWaiting = false;
    _bindingChoices = null;
    ++_bindingRequest;
    _phase = LauncherPhase.resolving;
    _output = _message = null;
    _userMoved = false;
    icons.retryFailures();
    _notify();
    try {
      final state = await _backend.updateInput(session, input, revision,
          scopeId: _scopeId, inputMode: _inputMode);
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
      if (revision != _requestedRevision) return;
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

  void _maybePrefill() {
    if (!_prefillAllowed ||
        _scopeTouched ||
        _userMoved ||
        _input.isNotEmpty ||
        !view.visible ||
        _globalWaiting) {
      return;
    }
    final captures = _snapshot?.contextCaptures ?? const <ContextCaptureDto>[];
    final prefill = ActivationPrefill.choose(captures, _nowMillis(),
        selectedEnabled: settings.current.captureSelectedText,
        clipboardEnabled: settings.current.captureClipboard,
        allowPendingSelection: _prefillGracePassed);
    if (prefill == null) {
      return;
    }
    _prefillAllowed = false;
    _prefillTimer?.cancel();
    _inputMode = InputModeDto.content;
    _contentSource = prefill.source;
    _wakeMode = WakeMode.universal;
    _scopeId = null;
    _actionPath = null;
    _prefillUpdate = update(prefill.text, automatic: true);
    unawaited(_prefillUpdate);
  }

  Future<void> _returnToCurrentApp() async {
    _prefillAllowed = false;
    _prefillTimer?.cancel();
    _inputMode = InputModeDto.environment;
    _contentSource = null;
    _wakeMode = WakeMode.currentApp;
    _scopeId = null;
    _actionPath = null;
    _pendingAppHome = true;
    await update('', automatic: true);
  }

  Future<void> _emptyGlobalSearch() async {
    _prefillAllowed = false;
    _prefillTimer?.cancel();
    _inputMode = InputModeDto.search;
    _contentSource = null;
    _wakeMode = WakeMode.universal;
    _scopeId = null;
    _actionPath = null;
    _pendingAppHome = false;
    await update('', automatic: true);
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

  /// Only called from the visible Launcher. Resolution uses the complete current
  /// action offers, independently of search text or the selected scope.
  Future<void> executeShortcut(ActionShortcut binding) async {
    final resolve = _backend.resolveBinding;
    final state = _snapshot;
    if ((binding.isIntent ? _backend.resolveIntent == null : resolve == null) ||
        state == null ||
        _phase != LauncherPhase.ready ||
        _output != null ||
        state.revision != _requestedRevision) {
      return;
    }
    if (binding.validate(settings.current) != null ||
        !settings.current.actionShortcuts.any((b) =>
            b.actionId == binding.actionId &&
            b.sameChord(binding.keyCode, binding.modifiers))) {
      return;
    }
    final request = ++_bindingRequest;
    final generation = _generation;
    final revision = _requestedRevision;
    try {
      final intent = binding.isIntent
          ? await _backend.resolveIntent!(state.sessionId, revision,
              binding.routes.map((r) => r.toDto()).toList())
          : null;
      final candidates = intent?.candidates ??
          await resolve!(state.sessionId, revision, binding.actionId);
      if (!_current(generation) ||
          request != _bindingRequest ||
          revision != _requestedRevision ||
          _phase != LauncherPhase.ready) {
        return;
      }
      if (!_bindingIsCurrent(binding)) {
        _message = 'Shortcut settings changed. Try again.';
        _notify();
        await _syncWindow();
        return;
      }
      if (intent != null) {
        if (intent.state.sessionId != state.sessionId ||
            intent.state.revision != revision) {
          return;
        }
        _apply(intent.state);
      }
      await _presentBinding(binding, candidates,
          requiresChoice: intent?.requiresChoice ?? false);
    } catch (error) {
      if (!_current(generation) ||
          request != _bindingRequest ||
          revision != _requestedRevision) {
        return;
      }
      _message = '$error';
      _notify();
      await _syncWindow();
    }
  }

  Future<void> _presentBinding(
      ActionShortcut binding, List<CandidateDto> candidates,
      {bool requiresChoice = false}) async {
    if (candidates.length == 1 && !requiresChoice) {
      await _executeCandidate(candidates.single);
    } else if (candidates.isNotEmpty) {
      _bindingChoices = candidates;
      _selectedId = null;
      _message = binding.isIntent
          ? 'Choose an action or object for ${binding.title}'
          : 'Choose an object for ${binding.title}';
      _notify();
      await _syncWindow();
    }
  }

  String? shortcutLabelFor(CandidateDto candidate) {
    if (candidate.kind != CandidateKindDto.action) return null;
    for (final binding in settings.current.actionShortcuts) {
      if (binding.actionId == candidate.commandId &&
          binding.validate(settings.current) == null) {
        return binding.label;
      }
    }
    return null;
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
    _prefillAllowed = false;
    _prefillTimer?.cancel();
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
    if (_disposed || lifecycle.isQuitting) return;
    if (_phase == LauncherPhase.settings) {
      // Restore the existing window without rebuilding the draft or recording state.
      await _window.show(_generation, view.height, settings: true);
      return;
    }
    if (_phase != LauncherPhase.hidden) await dismiss();
    final generation = ++_generation;
    _phase = LauncherPhase.settings;
    try {
      await settings.load(plugins: true);
    } catch (error) {
      settings.reportError('$error');
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
    if (!await settings.prepareClose()) {
      return false;
    }
    if (!_current(generation) || _phase != LauncherPhase.settings) return false;
    // Quitting only needs the draft decision; it does not need to reacquire keys.
    if (!lifecycle.isQuitting) {
      try {
        await _window.resumeActionShortcuts();
      } catch (error) {
        if (_current(generation) && _phase == LauncherPhase.settings) {
          final detail = error is PlatformException ? error.message : '$error';
          settings.reportShortcutRestoreError(detail ?? '');
        }
        return false;
      }
      if (!_current(generation) || _phase != LauncherPhase.settings) {
        return false;
      }
      settings.shortcutsRestored();
    }
    await dismiss();
    return true;
  }

  // Compatibility facade; settings behavior belongs to SettingsController.
  Future<bool> saveSettings(HostSettings next) async {
    final saved = await settings.save(next);
    _message = settings.statusMessage;
    _notify();
    return saved;
  }

  Future<void> openPrivacySettings(String pane) async {
    if (_disposed) return;
    final generation = _generation;
    final opened = await settings.openPrivacySettings(pane);
    if (!_current(generation)) return;
    if (opened && _phase != LauncherPhase.settings) {
      await dismiss();
    } else if (!opened) {
      _message = settings.statusMessage;
      _notify();
      await _syncWindow();
    }
  }

  Future<void> escape() async {
    if (_globalWaiting) {
      await dismiss();
      return;
    }
    if (_bindingChoices != null) {
      _bindingChoices = null;
      ++_bindingRequest;
      _message = null;
      _selectedId = null;
      _notify();
      await _syncWindow();
      return;
    }
    if (_phase == LauncherPhase.settings) {
      await closeSettings();
      return;
    }
    if (_output != null) {
      back();
      return;
    }
    if (_inputMode == InputModeDto.content) {
      await _returnToCurrentApp();
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
    if (_scopeId != null || _wakeMode == WakeMode.currentApp) {
      await _emptyGlobalSearch();
      return;
    }
    await dismiss();
  }

  void _applyWakeMode(WakeMode mode) {
    _actionPath = null;
    _contentSource = null;
    _prefillAllowed = false;
    _prefillTimer?.cancel();
    _inputMode = mode == WakeMode.currentApp
        ? InputModeDto.environment
        : InputModeDto.search;
    _pendingAppHome = mode == WakeMode.currentApp;
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
    _prefillAllowed = false;
    _prefillTimer?.cancel();
    _notify();
    await executeSelected();
  }

  Future<void> selectEntry(int index) async {
    final entries = view.visibleEntries;
    if (view.resolving || index < 0 || index >= entries.length) return;
    _selectedId = entries[index].key;
    _userMoved = true;
    _prefillAllowed = false;
    _prefillTimer?.cancel();
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
    _prefillTimer?.cancel();
    _disposed = true;
    _cachedView = null;
    _viewKey = null;
    ++_generation;
    final session = _session;
    if (session != null) {
      unawaited(_backend.deactivate(session).catchError((Object _) {}));
    }
    settings.removeListener(_notify);
    settings.dispose();
    icons.dispose();
    _window.dispose();
    super.dispose();
  }
}
