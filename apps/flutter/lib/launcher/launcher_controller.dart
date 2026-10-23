import '../tools/tool_page.dart';
import '../tools/builtin_tool_pages.dart';
import 'launcher_session_coordinator.dart';
import 'launcher_backend.dart';
export 'launcher_backend.dart';
import 'file_workspace_coordinator.dart';
import '../infrastructure/runtime_policy.dart';
import 'activation_flow.dart';
import 'object_action_coordinator.dart';
import 'host_view_navigator.dart';
import 'shortcut_coordinator.dart';
import 'launcher_navigation.dart';
import 'file_search_controller.dart';
import 'dart:async';
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

export '../infrastructure/application_icon_cache.dart' show LoadIcon;
export '../settings/settings_controller.dart' show ApplyExcludedApps;

export 'launcher_navigation.dart' show WakeMode, parseWakeMode;

/// Owns asynchronous presentation lifecycle; matching and execution stay in Rust.
class LauncherController extends ChangeNotifier {
  LauncherController({
    required LauncherBackend backend,
    required DesktopHost window,
    String startupMessage = '',
    bool fileSearchInitializationFailed = false,
    ApplyExcludedApps? applyExcludedApps,
    Future<void> Function(HostSettings)? applyContextSources,
    Future<void> Function(HostSettings)? applyFileSearch,
    FutureOr<List<ExtensionStatusDto>> Function()? readExtensions,
    Future<void> Function()? shutdownTasks,
    int Function()? nowMillis,
    ToolPageRegistry? toolPages,
  })  : _backend = backend,
        _window = window,
        _toolPages =
            toolPages ?? builtinToolPages(showWindow: window.showToolWindow),
        _nowMillis = nowMillis ?? (() => DateTime.now().millisecondsSinceEpoch),
        settings = SettingsController(
            window: window,
            startupMessage: startupMessage,
            fileSearchInitializationFailed: fileSearchInitializationFailed,
            applyExcludedApps: applyExcludedApps,
            applyContextSources: applyContextSources,
            applyFileSearch: applyFileSearch,
            readExtensions: readExtensions,
            readShortcutActions: backend.shortcutActions,
            readSupportedShortcutActions: backend.supportedShortcutActions,
            readShortcutApplications: backend.shortcutApplications),
        icons = ApplicationIconCache(backend.loadIcon) {
    _sessionFlow = LauncherSessionCoordinator(_nowMillis);
    files = FileWorkspaceCoordinator(backend.loadFilePreview);
    files.addListener(_filePresentationChanged);
    hostViews.register(HostViewDto.fileSearch, openFileSearch);
    for (final view in _toolPages.hostViews) {
      hostViews.registerPayload(view, (result) async {
        final showWindow = _toolPages.windowFor(result);
        if (showWindow != null) {
          final generation = _generation;
          await showWindow();
          if (_current(generation)) await dismiss();
          return;
        }
        final page = _toolPages.open(result);
        _toolPage = page;
        _message = result.warning;
        _notify();
        await _syncWindow();
        // Native resize may complete after the new editor acquired logical focus.
        // Reconnect its keyboard only after the Host finished changing the frame.
        if (identical(_toolPage, page) && this.view.visible) {
          ++_windowPresentation;
          _notify();
        }
      });
    }
    settings.addListener(_notify);
    lifecycle = ApplicationLifecycle(
        closeSettings: closeSettings,
        cancelLauncher: () async {
          if (hasRetainedState) {
            await _discardRetained();
          } else if (_phase != LauncherPhase.hidden) {
            await dismiss(retain: false);
          }
        },
        shutdownTasks: shutdownTasks ?? shutdownExecutionSessions);
  }

  late final FileWorkspaceCoordinator files;
  late final LauncherSessionCoordinator _sessionFlow;
  FileFilter _lastFileFilter = FileFilter.all;
  LauncherLocation? get _fileReturn => files.returnLocation;
  set _fileReturn(LauncherLocation? value) => files.returnLocation = value;
  final LauncherBackend _backend;
  final DesktopHost _window;
  final int Function() _nowMillis;
  final ActivationFlow _activation = ActivationFlow();
  final ObjectActionCoordinator _objectActions = ObjectActionCoordinator();
  final HostViewNavigator hostViews = HostViewNavigator();
  InputModeDto _snapshotInputMode = InputModeDto.environment;
  final SettingsController settings;
  late final ApplicationLifecycle lifecycle;
  final ApplicationIconCache icons;
  LauncherPhase _phase = LauncherPhase.hidden;
  LauncherStateDto? _snapshot;
  int? get _session => _sessionFlow.session;
  set _session(int? value) => _sessionFlow.session = value;
  int get _generation => _sessionFlow.generation;
  set _generation(int value) => _sessionFlow.generation = value;
  int get _revision => _sessionFlow.revision;
  set _revision(int value) => _sessionFlow.revision = value;
  int get _requestedRevision => _sessionFlow.requestedRevision;
  set _requestedRevision(int value) => _sessionFlow.requestedRevision = value;
  int _windowPresentation = 0;

  /// Advances only after Host show completes for the current visible Launcher.
  int get windowPresentation => _windowPresentation;
  String _input = '';
  final LauncherNavigation _navigation = LauncherNavigation();
  LauncherOverlay get _overlay => _navigation.overlay;
  set _overlay(LauncherOverlay value) => _navigation.overlay = value;
  InputModeDto? get _navigationMode => _navigation.chooserMode;
  set _navigationMode(InputModeDto? mode) => _navigation.chooserMode = mode;

  List<CandidateDto>? get _bindingChoices => switch (_overlay) {
        BindingChoices(:final items) || ObjectActions(:final items) => items,
        _ => null,
      };
  String? get _objectActionTarget =>
      switch (_overlay) { ObjectActions(:final target) => target, _ => null };
  String? get _objectActionTitle =>
      switch (_overlay) { ObjectActions(:final title) => title, _ => null };
  String? get _objectActionParent =>
      switch (_overlay) { ObjectActions(:final parent) => parent, _ => null };
  static const retention =
      Duration(milliseconds: RuntimePolicy.launcherRetentionMs);
  int? get _hiddenAt => _sessionFlow.hiddenAt;
  LauncherPhase? _retainedPhase;

  Object get _capturePolicy => (
        hostSettings.captureFinderSelection,
        hostSettings.captureSelectedText,
        hostSettings.captureBrowserCurrentPage,
        hostSettings.captureClipboard
      );

  bool get hasRetainedState => _hiddenAt != null;

  // Restore ongoing work, while an empty root is an entry point for the app
  // that is foreground now. Resuming its old frozen environment would ignore
  // the requested wake mode and could keep showing a blank Global Search.
  bool get _hasRetainedWorkspace =>
      _input.isNotEmpty ||
      _activation.mode == InputModeDto.fileSearch ||
      _actionPath != null ||
      _overlay is! LauncherHome ||
      _output != null ||
      _toolPage != null ||
      (_scopeId != null && view.selectedScope?.isCurrentApp == false);

  String? get _scopeId => _navigation.scope;
  set _scopeId(String? value) => _navigation.scope = value;
  List<String>? get _actionPath => _navigation.menu;
  set _actionPath(List<String>? value) => _navigation.menu = value;
  late final ShortcutCoordinator _shortcuts = ShortcutCoordinator(_backend);
  WakeMode get _wakeMode => _navigation.wakeMode;
  set _wakeMode(WakeMode value) => _navigation.wakeMode = value;
  bool _scopeTouched = false;
  String? _selectedId, _message, _output;
  ActionStatusDto? _resultStatus;
  final ToolPageRegistry _toolPages;
  ToolPageSession? _page;
  ToolPageSession? get _toolPage => _page;
  set _toolPage(ToolPageSession? value) {
    if (identical(_page, value)) return;
    _page?.dispose();
    _page = value;
    final session = _session, revision = _revision;
    value?.bindInteraction(() =>
        !_disposed &&
        identical(_page, value) &&
        _phase == LauncherPhase.ready &&
        _session == session &&
        _revision == revision);
  }

  bool _userMoved = false, _disposed = false;
  bool get _fileMoreLoading => files.moreLoading;

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
      _activation.mode,
      _snapshotInputMode,
      _activation.source,
      _scopeId,
      _actionPath,
      _selectedId,
      _message,
      _output,
      _toolPage,
      _resultStatus,
      settings.current.universalLabel,
      _overlay,
      files.filter,
    );
    if (key == _viewKey && _cachedView != null) return _cachedView!;
    _viewKey = key;
    final previous = _cachedView;
    final retainEntries = _activation.mode == InputModeDto.fileSearch &&
        _snapshotInputMode == _activation.mode &&
        _snapshot?.input == _input &&
        previous?.isFileSearch == true &&
        previous?.bindingChoices == null &&
        _bindingChoices == null &&
        previous?.output == null &&
        _output == null &&
        previous?.input == _input &&
        previous?.fileFilter == files.filter &&
        identical(previous?.snapshot?.candidates, _snapshot?.candidates);
    return _cachedView = LauncherViewState(
      retainedFileEntries: retainEntries ? previous!.entryCollection : null,
      phase: _phase,
      generation: _generation,
      snapshot: _snapshot,
      input: _input,
      inputMode: _activation.mode,
      inputModeReady: _snapshotInputMode == _activation.mode,
      contentSource: _activation.source,
      scopeId: _scopeId,
      actionPath: _actionPath,
      selectedId: _selectedId,
      message: _message,
      output: _output,
      toolPage: _toolPage,
      resultStatus: _resultStatus,
      universalShortcut: settings.current.universalLabel,
      bindingChoices: _bindingChoices,
      objectActionTitle: _objectActionTitle,
      fileFilter: files.filter,
      navigationMode: _navigationMode,
    );
  }

  HostSettings get hostSettings => settings.current;

  void _resetFiles() {
    files.reset();
    _lastFileFilter = files.filter;
  }

  void _filePresentationChanged() {
    if (_lastFileFilter != files.filter) {
      _lastFileFilter = files.filter;
      _selectedId = null;
      _userMoved = false;
      _overlay = const LauncherHome();
      if (!_disposed && view.isFileSearch && _backend.updateFiles != null) {
        unawaited(update(_input));
        return;
      }
    }
    if (_disposed) return;
    final previousHeight = _cachedView?.height;
    _notify();
    if (view.isFileSearch && previousHeight != view.height) {
      unawaited(_syncWindow());
    }
  }

  void _notify() {
    if (_disposed) return;
    if (_phase == LauncherPhase.hidden) {
      notifyListeners();
      return;
    }
    final entries = view.visibleEntries;
    final parentKey = _objectActionParent;
    final file = view.isFileSearch && !view.resolving
        ? parentKey != null
            ? _snapshot?.candidates
                .where((c) => 'candidate:${c.id}' == parentKey)
                .firstOrNull
            : _bindingChoices == null && entries.isNotEmpty
                ? entries[view.selectedIndex].candidate
                : null
        : null;
    files.sync(view.isFileSearch ? _session : null, _revision,
        file?.kind == CandidateKindDto.file ? file : null);
    notifyListeners();
  }

  bool _current(int generation) =>
      !_disposed && _sessionFlow.current(generation);

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
    await _applyWakeMode(parsed);
  }

  /// Native global events carry only business identity, never cached object data.
  bool _bindingIsCurrent(ActionShortcut binding) =>
      _shortcuts.bindingCurrent(binding, settings.current);

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
    if (_hiddenAt != null) {
      if (globalBinding == null &&
          _hasRetainedWorkspace &&
          await _resumeRetained()) {
        return;
      }
      await _discardRetained();
      if (_disposed || lifecycle.isQuitting || _phase != LauncherPhase.hidden) {
        return;
      }
    }
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
      _activation.begin(
          prefill: mode == WakeMode.currentApp && globalBinding == null,
          current: () => _current(generation),
          reconsider: _maybePrefill);
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
      _toolPage = null;
      _resultStatus = null;
      _phase =
          globalBinding == null ? LauncherPhase.ready : LauncherPhase.resolving;
      _shortcuts.waiting = globalBinding != null;
      if (_shortcuts.waiting) {
        _message = 'Capturing context for ${globalBinding!.title}…';
      }
      _apply(state);
      if (!_current(generation)) return;
      if (mode == WakeMode.universal && globalBinding == null) {
        _activation.mode = InputModeDto.search;
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
        _shortcuts.waiting &&
        _requestedRevision == revision &&
        _phase == LauncherPhase.resolving;
    try {
      final resolved =
          await _shortcuts.resolve(binding, _session!, revision, wait: true);
      if (!current()) return;
      if (!_bindingIsCurrent(binding)) {
        _shortcuts.waiting = false;
        _phase = LauncherPhase.ready;
        _message = 'Shortcut settings changed. Try again.';
        _notify();
        await _syncWindow();
        return;
      }
      _shortcuts.waiting = false;
      _apply(resolved.state!);
      _phase = LauncherPhase.ready;
      _message = null;
      await _presentBinding(binding, resolved.candidates,
          requiresChoice: resolved.requiresChoice);
    } catch (error) {
      if (!current()) return;
      _shortcuts.waiting = false;
      _phase = LauncherPhase.ready;
      _message = '$error';
      _notify();
      await _syncWindow();
    }
  }

  Future<void> dismiss(
      {bool hostAlreadyHidden = false,
      bool retain = true,
      bool preserveSettings = false}) async {
    if (_disposed) return;
    // Native focus loss and an explicit hide can arrive for the same window.
    // A repeated hide must not turn retained work into a destructive dismissal.
    if (retain && _phase == LauncherPhase.hidden) return;
    if (retain &&
        _phase != LauncherPhase.hidden &&
        _phase != LauncherPhase.settings &&
        _session != null &&
        _snapshot != null &&
        _backend.suspend != null &&
        _backend.resume != null) {
      final generation = _generation, session = _session!;
      _retainedPhase = _phase;
      ++_generation;
      _shortcuts.invalidate();
      _phase = LauncherPhase.hidden;
      _activation.stopPrefill();
      files.invalidateOperations();
      _objectActions.invalidate();
      final suspension = _backend.suspend!(session);
      _sessionFlow.retain(
          _capturePolicy, suspension, () => unawaited(_discardRetained()));
      // Listeners save scroll / editor state when the window becomes hidden.
      // Publish the retention lifetime before notifying them.
      _notify();
      try {
        if (!hostAlreadyHidden) await _window.hide(generation);
      } finally {
        await suspension;
      }
      return;
    }
    final generation = _generation;
    final session = _session;
    ++_generation;
    _shortcuts.waiting = false;
    _navigationMode = null;
    _session = null;
    _snapshot = null;
    _cachedView = null;
    _viewKey = null;
    _overlay = const LauncherHome();
    _fileReturn = null;
    _resetFiles();
    _shortcuts.invalidate();
    _phase = preserveSettings ? LauncherPhase.settings : LauncherPhase.hidden;
    _input = '';
    _activation.stopPrefill();
    _activation.source = null;
    _actionPath = null;
    _scopeId = null;
    _scopeTouched = false;
    _selectedId = _message = _output = null;
    _toolPage = null;
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

  Future<void> _discardRetained() => _discardHiddenState();
  Future<void> _discardHiddenState() async {
    if (_hiddenAt == null) return;
    final generation = _generation, session = _session;
    _retainedPhase = null;
    final pending = _sessionFlow.clearRetention();
    try {
      await pending;
    } catch (_) {}
    if (!_current(generation) || _session != session || _hiddenAt != null) {
      return;
    }
    // Hidden cleanup must never clear a newer settings window or activation.
    if (_phase == LauncherPhase.hidden || _phase == LauncherPhase.settings) {
      await dismiss(
          hostAlreadyHidden: true,
          retain: false,
          preserveSettings: _phase == LauncherPhase.settings);
    }
  }

  Future<bool> _resumeRetained() =>
      _sessionFlow.restoreOnce(_restoreHiddenState);
  Future<bool> _restoreHiddenState() async {
    final hidden = _hiddenAt, session = _session, resume = _backend.resume;
    if (hidden == null ||
        session == null ||
        resume == null ||
        !_sessionFlow.canResume(_capturePolicy)) {
      return false;
    }
    _sessionFlow.cancelDeadline();
    final generation = ++_generation;
    try {
      await _sessionFlow.suspension;
      if (!_current(generation) ||
          _phase != LauncherPhase.hidden ||
          !_sessionFlow.canResume(_capturePolicy)) {
        return false;
      }
      final state = await resume(session);
      if (!_current(generation) ||
          !_sessionFlow.canResume(_capturePolicy) ||
          state == null ||
          state.sessionId != session) {
        return false;
      }
      _sessionFlow.clearRetention();
      _phase = LauncherPhase.ready;
      _apply(state);
      if (_retainedPhase == LauncherPhase.resolving ||
          (view.isFileSearch &&
              state.fileSearch == null &&
              _input.isNotEmpty)) {
        await update(_input, automatic: true);
      }
      _retainedPhase = null;
      _notify();
      await _syncWindow(show: true);
      unawaited(_follow(generation, session));
      return true;
    } catch (_) {
      return false;
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
    final fileTarget = _objectActionTarget,
        fileTitle = _objectActionTitle,
        fileParent = _objectActionParent;
    final overlay = _overlay;
    _snapshot = state;
    _snapshotInputMode = _activation.mode;
    _overlay = overlay is RootChooser ? overlay : const LauncherHome();
    _shortcuts.invalidate();
    if (fileTarget != null && held?.revision == state.revision) {
      final actions = state.resultActions
          .where((c) => c.executionContextId == fileTarget)
          .toList();
      if (actions.isNotEmpty ||
          state.candidates.any((candidate) =>
              candidate.primaryAction &&
              candidate.executionContextId == fileTarget)) {
        _overlay = ObjectActions(
            target: fileTarget,
            title: fileTitle!,
            parent: fileParent,
            items: actions);
      }
    }
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
    if (_activation.pendingAppHome) {
      final app = view.scopes.where((scope) => scope.isCurrentApp).firstOrNull;
      if (app != null) {
        _scopeId = app.id;
        _activation.pendingAppHome = false;
      }
    }
    _maybeSelectScope();
    _maybePrefill();
    final stableFileRows = view.isFileSearch &&
        held?.revision == state.revision &&
        identical(held?.candidates, state.candidates) &&
        _bindingChoices == null;
    if (!stableFileRows) {
      _reconcile();
      _requestIcons();
    }
  }

  void _reconcile() {
    final entries = view.visibleEntries;
    if (_userMoved && view.entryCollection.indices[_selectedId] != null) {
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
    final waitDelta = _backend.waitForSnapshotDelta;
    if (wait == null && waitDelta == null) return;
    var cursor = _snapshot == null ? 0 : snapshotCursor(_snapshot!);
    try {
      while (_current(generation) && view.visible) {
        final baseCount = _snapshot?.candidates.length ?? 0;
        final notice = waitDelta == null
            ? await wait!(session, cursor)
            : await waitDelta(session, cursor,
                _snapshot?.candidateListVersion ?? 0, baseCount);
        if (!_current(generation) || !view.visible) return;
        if (notice.kind == SnapshotWaitKindDto.timedOut) continue;
        if (notice.kind != SnapshotWaitKindDto.updated) return;
        final packet = notice.state;
        if (packet == null || packet.sessionId != session) return;
        final heldCursor =
            _snapshot == null ? cursor : snapshotCursor(_snapshot!);
        final nextCursor = snapshotCursor(packet);
        if (nextCursor <= heldCursor) {
          cursor = heldCursor;
          continue;
        }
        final next = LauncherSessionCoordinator.materialize(
            _snapshot, notice, baseCount);
        if (next == null) {
          cursor = heldCursor;
          continue;
        }
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
    _navigationMode = null;
    final revision = ++_revision;
    _requestedRevision = revision;
    files.invalidateOperations();
    _objectActions.invalidate();

    final generation = _generation;
    _input = input;
    if (!automatic) {
      _activation.stopPrefill();
      _activation.pendingAppHome = false;
      _scopeTouched = true;
    }
    _shortcuts.waiting = false;
    _overlay = const LauncherHome();
    _shortcuts.invalidate();
    _phase = LauncherPhase.resolving;
    _output = _message = null;
    _toolPage = null;
    _userMoved = false;
    icons.retryFailures();
    _notify();
    try {
      final state = _activation.mode == InputModeDto.fileSearch &&
              _backend.updateFiles != null
          ? await _backend.updateFiles!(
              session, input, revision, files.filter.name)
          : await _backend.updateInput(session, input, revision,
              scopeId: _scopeId, inputMode: _activation.mode);
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
    if (!_activation.prefillAllowed) return;
    if (_activation.pendingPrefill != null) {
      _activation.reconsiderAfterCheck = true;
      return;
    }
    final generation = _generation;
    Future<void> check() async {
      final offer = await _activation.take(
          _snapshot?.contextCaptures ?? const [], _nowMillis(),
          choose: _backend.chooseActivationInput,
          interacted: () =>
              !_current(generation) ||
              _scopeTouched ||
              _userMoved ||
              _input.isNotEmpty ||
              _shortcuts.waiting ||
              _activation.reconsiderAfterCheck,
          visible: () => view.visible,
          nowMillis: _nowMillis,
          selectedEnabled: settings.current.captureSelectedText,
          clipboardEnabled: settings.current.captureClipboard);
      if (offer == null) return;
      _wakeMode = WakeMode.universal;
      _scopeId = null;
      _actionPath = null;
      await update(offer.text, automatic: true);
    }

    final pending = check();
    _activation.pendingPrefill = pending;
    unawaited(pending.catchError((Object error) {
      // A failed recognition is not a content page or a Launcher failure.
      // Keep the current app available and leave diagnostics to the backend.
    }).whenComplete(() {
      if (!identical(_activation.pendingPrefill, pending)) return;
      _activation.pendingPrefill = null;
      if (_current(generation) && _activation.reconsiderAfterCheck) {
        _activation.reconsiderAfterCheck = false;
        _maybePrefill();
      }
    }));
  }

  Future<void> _returnToCurrentApp() async {
    _activation.stopPrefill();
    _activation.mode = InputModeDto.environment;
    _activation.source = null;
    _wakeMode = WakeMode.currentApp;
    _scopeId = null;
    _actionPath = null;
    _activation.pendingAppHome = true;
    await update('', automatic: true);
  }

  Future<void> _emptyGlobalSearch() async {
    _activation.stopPrefill();
    _activation.mode = InputModeDto.search;
    _activation.source = null;
    _wakeMode = WakeMode.universal;
    _scopeId = null;
    _actionPath = null;
    _activation.pendingAppHome = false;
    await update('', automatic: true);
  }

  Future<void> openFileSearch() async {
    if (_phase != LauncherPhase.ready || view.isFileSearch) return;
    // A global Action shortcut opens a tool directly, before the user has
    // visited a parent page. Its back destination is empty Global Search.
    final parentMode = _wakeMode == WakeMode.universal &&
            _input.isEmpty &&
            _activation.mode == InputModeDto.environment
        ? InputModeDto.search
        : _activation.mode;
    _fileReturn = LauncherLocation(
        input: _input,
        scope: _scopeId,
        menu: _actionPath,
        mode: parentMode,
        universal: _wakeMode == WakeMode.universal,
        source: _activation.source);
    _activation.stopPrefill();
    _activation.pendingAppHome = false;
    _scopeTouched = true;
    _scopeId = null;
    _actionPath = null;
    _activation.source = null;
    _activation.mode = InputModeDto.fileSearch;
    _resetFiles();
    await update('', automatic: true);
  }

  Future<void> closeFileSearch() async {
    final previous = _fileReturn;
    if (previous == null) {
      return;
    }
    _fileReturn = null;
    _resetFiles();
    _scopeId = previous.scope;
    _actionPath = previous.menu;
    _activation.mode = previous.mode;
    _wakeMode = previous.universal ? WakeMode.universal : WakeMode.currentApp;
    _activation.source = previous.source;
    await update(previous.input, automatic: true);
  }

  /// Selecting a result is safe; activation is a separate, deliberate gesture.
  bool selectResult(int index) {
    final entries = view.visibleEntries;
    if (_phase != LauncherPhase.ready || index < 0 || index >= entries.length) {
      return false;
    }
    _selectedId = entries[index].key;
    _userMoved = true;
    freezeFileResults();
    _activation.stopPrefill();
    _notify();
    return true;
  }

  void freezeFileResults() {
    if (!view.isFileSearch ||
        _session == null ||
        _backend.freezeFiles == null) {
      return;
    }
    unawaited(_backend.freezeFiles!(_session!, _revision).catchError((_) {}));
  }

  bool get fileMoreLoading => _fileMoreLoading;
  bool get canLoadMoreFiles =>
      view.hasFileControls &&
      _phase == LauncherPhase.ready &&
      !_fileMoreLoading &&
      (_backend.fileMore != null || _backend.fileMoreDelta != null) &&
      ((_snapshot?.fileSearch?.nextCursor != null) ||
          (_snapshot?.fileSearch?.needsRefresh ?? false));

  /// The backend returns the accumulated list and keeps all displayed targets
  /// executable. Loading never clears selection or resets the scroll position.
  Future<void> loadMoreFiles({bool advanceSelection = false}) async {
    final session = _session;
    if (session == null || !canLoadMoreFiles) return;
    final generation = _generation, revision = _revision;
    final selection = _selectedId;
    final previousCount = view.visibleEntries.length;
    _notify();
    try {
      final state = await files.loadSnapshot(
          session,
          revision,
          _backend,
          () => _snapshot,
          () => _current(generation) && revision == _revision && view.visible);
      if (!_current(generation) ||
          revision != _revision ||
          state == null ||
          !view.visible ||
          state.sessionId != session ||
          state.revision != revision) {
        return;
      }
      _apply(state);
      if (advanceSelection &&
          _selectedId == selection &&
          view.visibleEntries.length > previousCount &&
          view.hasFileControls) {
        _selectedId = view.visibleEntries[previousCount].key;
        _userMoved = true;
      }
      _message = null;
      _notify();
      await _syncWindow();
    } catch (_) {
      if (_current(generation) && revision == _revision) {
        _message = 'Could not load more files. Scroll or press ↓ to retry.';
        _notify();
      }
    }
  }

  Future<void> retrySearch() async {
    if (_phase != LauncherPhase.ready ||
        !(view.searchUnavailable || view.searchPartial) ||
        _scopeId != null ||
        view.isContent) {
      return;
    }
    await update(_input);
  }

  bool get canShowObjectActions {
    final entries = view.visibleEntries;
    final candidate =
        entries.isEmpty ? null : entries[view.selectedIndex].candidate;
    return _objectActionTarget == null &&
        candidate != null &&
        candidate.primaryAction &&
        candidate.executionContextId != null &&
        (view.isFileSearch ||
            _backend.objectActions != null ||
            (_snapshot?.resultActions.any((c) =>
                    c.executionContextId == candidate.executionContextId) ??
                false));
  }

  Future<void> showObjectActions() async {
    if (_phase != LauncherPhase.ready || !canShowObjectActions) return;
    freezeFileResults();
    final file = view.visibleEntries[view.selectedIndex].candidate!;
    final generation = _generation, revision = _revision;
    final parent = _selectedId;
    List<CandidateDto>? actions;
    try {
      actions = await _objectActions.load(
          () async => _backend.objectActions != null
              ? await _backend.objectActions!(_session!, revision, file.id)
              : _snapshot?.resultActions
                      .where((c) =>
                          c.executionContextId == file.executionContextId)
                      .toList() ??
                  [],
          () =>
              _current(generation) &&
              revision == _revision &&
              view.visible &&
              _selectedId == parent);
    } catch (_) {
      if (_current(generation) && view.visible) {
        _message =
            'Actions could not be loaded. Select the item and try again.';
        _notify();
      }
      return;
    }
    if (!_current(generation) ||
        revision != _revision ||
        !view.visible ||
        actions == null ||
        _selectedId != parent) {
      return;
    }
    if (actions.isEmpty && !view.isFileSearch) return;
    _overlay = ObjectActions(
        target: file.executionContextId!,
        title: file.title,
        parent: _selectedId,
        items: actions);
    _selectedId = null;
    _notify();
    await _syncWindow();
  }

  Future<void> executeSelected() async {
    final state = _snapshot;
    final entries = view.visibleEntries;
    if (_phase != LauncherPhase.ready ||
        _toolPage != null ||
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
        _toolPage != null ||
        state.revision != _requestedRevision) {
      return;
    }
    if (binding.validate(settings.current) != null ||
        !settings.current.actionShortcuts.any((b) =>
            b.actionId == binding.actionId &&
            b.sameChord(binding.keyCode, binding.modifiers))) {
      return;
    }
    final request = _shortcuts.reserve();
    final generation = _generation;
    final revision = _requestedRevision;
    try {
      final offers =
          await _shortcuts.resolve(binding, state.sessionId, revision);
      if (!_current(generation) ||
          !_shortcuts.current(request) ||
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
      if (offers.state != null) _apply(offers.state!);
      await _presentBinding(binding, offers.candidates,
          requiresChoice: offers.requiresChoice);
    } catch (error) {
      if (!_current(generation) ||
          !_shortcuts.current(request) ||
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
      _overlay = BindingChoices(candidates);
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
        _toolPage != null ||
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
    _activation.stopPrefill();
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
          hostViews.accepts(result.view)) {
        _phase = LauncherPhase.ready;
        await hostViews.open(result.view, result: result);
        if (result.warning != null &&
            _current(generation) &&
            revision == _revision) {
          _message = result.warning;
          _notify();
        }
      } else if (result.status == ActionStatusDto.succeeded &&
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
        if (_output != null) _toolPage = _toolPages.openResult(result);
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
    if (view.isFileSearch && view.hasFileControls) {
      if (direction > 0 && view.selectedIndex == entries.length - 1) {
        if (canLoadMoreFiles) unawaited(loadMoreFiles(advanceSelection: true));
        return;
      }
      if (direction < 0 && view.selectedIndex == 0) return;
    }
    _selectedId = entries[
            (view.selectedIndex + direction + entries.length) % entries.length]
        .key;
    _userMoved = true;
    freezeFileResults();
    _output = null;
    _activation.stopPrefill();
    _resultStatus = null;
    _notify();
    unawaited(_syncWindow());
  }

  void cycleScope(int direction) {
    if (view.isFileSearch) {
      if (_phase == LauncherPhase.ready && view.hasFileControls) {
        files.cycleFilter(direction);
      }
      return;
    }
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
    _resultStatus = null;
    _userMoved = preserveSelection;
    icons.retryFailures();
    _reconcile();
    _requestIcons();
    _notify();
  }

  Future<void> selectScope(String? id) async {
    if (id != null && !view.scopes.any((scope) => scope.id == id)) return;
    _navigation.selectScope(id);
    _scopeTouched = true;
    _selectedId = null;
    await update(_input);
  }

  void back() {
    if (_output == null && _toolPage == null) return;
    _toolPage = null;
    _output = null;
    _resultStatus = null;
    _notify();
    unawaited(_syncWindow());
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
      await settings.load(catalog: true);
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
    if (hasRetainedState) {
      _phase = LauncherPhase.hidden;
      ++_generation;
      _notify();
      await _window.hide(generation);
    } else {
      await dismiss();
    }
    return true;
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
    if (_toolPage != null) {
      back();
      return;
    }
    if (_navigationMode != null) {
      await toggleNavigation();
      return;
    }
    if (_shortcuts.waiting) {
      await dismiss();
      return;
    }
    if (_bindingChoices != null) {
      final parent = _objectActionParent;
      _overlay = const LauncherHome();
      _shortcuts.invalidate();
      _message = null;
      _selectedId = parent;
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
    if (view.isFileSearch) {
      if (_input.isNotEmpty) {
        await update('');
      } else {
        await closeFileSearch();
      }
      return;
    }
    if (_activation.mode == InputModeDto.content) {
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

  /// Explicit root navigation never uses the visibility-toggle semantics.
  Future<void> toggleNavigation() async {
    if (_phase != LauncherPhase.ready || view.isContent || view.isFileSearch) {
      return;
    }
    _navigationMode = _navigationMode == null ? _activation.mode : null;
    // Opening the chooser is a deliberate interaction; late captured content
    // must not replace the input while the user is choosing a search mode.
    _activation.stopPrefill();
    _notify();
    await _syncWindow();
  }

  void moveNavigation(int direction) {
    if (_navigationMode == null || direction == 0) return;
    _navigation.cycleRoot();
    _notify();
  }

  Future<void> chooseNavigation() async {
    final mode = _navigationMode;
    if (mode == null) return;
    await navigateRoot(
        mode == InputModeDto.search ? WakeMode.universal : WakeMode.currentApp);
  }

  Future<void> navigateRoot(WakeMode mode) async {
    if (_phase != LauncherPhase.ready) return;
    _navigationMode = null;
    await _applyWakeMode(mode);
  }

  Future<void> _applyWakeMode(WakeMode mode) async {
    _navigationMode = null;
    _fileReturn = null;
    _resetFiles();
    _actionPath = null;
    _activation.source = null;
    _activation.stopPrefill();
    _activation.mode = mode == WakeMode.currentApp
        ? InputModeDto.environment
        : InputModeDto.search;
    _activation.pendingAppHome = mode == WakeMode.currentApp;
    _wakeMode = mode;
    _scopeTouched = true;
    if (mode == WakeMode.currentApp) _promptCurrentAppAccess(mode);
    final app = view.scopes.where((scope) => scope.isCurrentApp).firstOrNull;
    _scopeId = mode == WakeMode.currentApp ? app?.id : null;
    _selectedId = null;
    // Keep pending App selection until the environment snapshot arrives. This
    // transition is navigation, not user typing (which cancels auto-selection).
    await update(_input, automatic: true);
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
        view.entryCollection.indices['candidate:$id'] == null) {
      return;
    }
    _selectedId = 'candidate:$id';
    _userMoved = true;
    _activation.stopPrefill();
    _notify();
    await executeSelected();
  }

  Future<void> selectEntry(int index) async {
    if (!selectResult(index)) return;
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
    _navigation.backMenu();
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
      final generation = _generation;
      await _window.show(generation, view.height,
          width: view.width, settings: _phase == LauncherPhase.settings);
      if (_current(generation) &&
          view.visible &&
          _phase != LauncherPhase.settings) {
        ++_windowPresentation;
        _notify();
      }
    } else {
      await _window.resize(_generation, view.height, width: view.width);
    }
  }

  @override
  void dispose() {
    _activation.dispose();
    _sessionFlow.dispose();
    _toolPage = null;
    _disposed = true;
    _cachedView = null;
    _viewKey = null;
    ++_generation;
    final session = _session;
    if (session != null) {
      unawaited(_backend.deactivate(session).catchError((Object _) {}));
    }
    files.removeListener(_filePresentationChanged);
    files.dispose();
    settings.removeListener(_notify);
    settings.dispose();
    icons.dispose();
    _window.dispose();
    super.dispose();
  }
}
