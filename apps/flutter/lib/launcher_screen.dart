import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter/foundation.dart';

import 'src/rust/api.dart';

typedef Activate = Future<LauncherStateDto?> Function();
typedef UpdateInput = Future<LauncherStateDto?> Function(
  int session,
  String input,
  int revision,
);
typedef Execute = Future<ActionResultDto> Function(
  int session,
  int revision,
  String candidate,
);
/// Awaits a snapshot beyond `after`, or null when the wait elapsed with nothing new.
///
/// `after` is a session-monotonic cursor (see [snapshotCursor]); the core restarts
/// `snapshot_seq` at 1 for every revision, so the sequence alone cannot order snapshots
/// across an input change.
typedef WaitForSnapshot = Future<LauncherStateDto?> Function(
  int session,
  int after,
);

/// Monotonic position of one snapshot within its session.
int snapshotCursor(LauncherStateDto state) =>
    (state.revision << 32) | state.snapshotSeq;
/// Fetches an application icon on demand; presentation only.
typedef LoadIcon = Future<Uint8List?> Function(String candidateId);

class LauncherApp extends StatelessWidget {
  const LauncherApp({
    super.key,
    required this.activate,
    required this.updateInput,
    required this.execute,
    required this.deactivate,
    required this.window,
    this.waitForSnapshot,
    this.loadIcon,
    this.startupMessage = '',
  });
  final Activate activate;
  final UpdateInput updateInput;
  final Execute execute;
  final Future<void> Function(int) deactivate;
  final MethodChannel window;
  /// Awaits late context. Absent in tests that only exercise the first snapshot.
  final WaitForSnapshot? waitForSnapshot;
  /// Loads application icons lazily. Absent in tests that do not need icons.
  final LoadIcon? loadIcon;
  final String startupMessage;

  @override
  Widget build(BuildContext context) => MaterialApp(
        debugShowCheckedModeBanner: false,
        title: 'Anywhere',
        theme: ThemeData(
          brightness: Brightness.dark,
          useMaterial3: true,
          colorScheme: ColorScheme.fromSeed(
            seedColor: const Color(0xffa7b4cb),
            brightness: Brightness.dark,
          ),
          scaffoldBackgroundColor: Colors.transparent,
          fontFamily: defaultTargetPlatform == TargetPlatform.windows
              ? 'Segoe UI'
              : '.AppleSystemUIFont',
        ),
        home: LauncherScreen(
          activate: activate,
          updateInput: updateInput,
          execute: execute,
          deactivate: deactivate,
          window: window,
          waitForSnapshot: waitForSnapshot,
          loadIcon: loadIcon,
          startupMessage: startupMessage,
        ),
      );
}

class LauncherScreen extends StatefulWidget {
  const LauncherScreen({
    super.key,
    required this.activate,
    required this.updateInput,
    required this.execute,
    required this.deactivate,
    required this.window,
    required this.startupMessage,
    this.waitForSnapshot,
    this.loadIcon,
  });
  final Activate activate;
  final UpdateInput updateInput;
  final Execute execute;
  final Future<void> Function(int) deactivate;
  final MethodChannel window;
  final WaitForSnapshot? waitForSnapshot;
  final LoadIcon? loadIcon;
  final String startupMessage;
  @override
  State<LauncherScreen> createState() => _LauncherScreenState();
}

class _LauncherScreenState extends State<LauncherScreen> {
  static const _rowHeight = 52.0;
  static const _maxRows = 7;
  static const _inputHeight = 64.0;
  static const _scopeHeight = 32.0;
  static const _headerHeight = 28.0;
  static const _footerHeight = 32.0;
  static const _emptyHeight = 54.0;
  /// Context Home shows this many rows per scope; a query scrolls instead.
  static const _homeLimit = 7;
  final _input = TextEditingController();
  final _focus = FocusNode();
  final _scroll = ScrollController();
  LauncherStateDto? _state;
  String _scopeId = 'all';
  /// Stable candidate identity, not a list index, so late reranking cannot move it.
  String? _selectedId;
  /// True once the user moved the selection; the service then keeps their row.
  bool _userMovedSelection = false;
  int _revision = 0;
  /// Highest revision this window has asked for.
  ///
  /// A late snapshot carries the revision it was resolved for, so one from a previous
  /// revision must never be displayed. The displayed state's own revision already
  /// rejects that case; this states the intent directly and stays correct if the
  /// displayed state is ever cleared while a request is outstanding. No test reaches
  /// it as a distinct branch today, and it is not claimed as covered.
  int _requestedRevision = 0;
  int _generation = 0;
  bool _visible = false;
  bool _activating = false;
  bool _resolving = false;
  bool _executing = false;
  String? _message;
  String? _output;
  /// Icon bytes for candidates that have one. Bounded so a long search cannot grow
  /// unbounded.
  static const _iconCacheLimit = 128;
  final Map<String, Uint8List> _icons = {};
  /// Candidate ids with an icon request in flight, so a frame does not ask twice.
  final Set<String> _iconRequests = {};
  /// Candidate ids that came back without an icon during the current query.
  ///
  /// Kept separate from `_icons` so an absence is never cached forever, and cleared
  /// whenever the user changes the query or the scope: each of those gets one fresh
  /// attempt, which covers a bridge that was still starting, while repeated rebuilds
  /// of the same query cannot turn into a retry storm.
  final Set<String> _iconFailures = {};

  List<ScopeDto> get _scopes => _state?.scopes ?? const [];
  ScopeDto? get _scope {
    final scopes = _scopes;
    for (final scope in scopes) {
      if (scope.id == _scopeId) return scope;
    }
    return scopes.isEmpty ? null : scopes.first;
  }

  /// Candidates of the active scope, in the order the service ranked them.
  /// Candidates of the active scope, ranked, as this view will show them.
  ///
  /// The service keeps every candidate so its scope id lists always resolve; Context
  /// Home is where the top-N limit belongs, and it is applied per scope rather than to
  /// the whole set, which is what used to leave a populated group rendering empty.
  List<CandidateDto> get _visibleCandidates {
    final state = _state;
    if (state == null) return const [];
    final scope = _scope;
    final byId = <String, CandidateDto>{
      for (final candidate in state.candidates) candidate.id: candidate,
    };
    final list = <CandidateDto>[];
    final ids = scope?.candidateIds;
    if (ids != null) {
      for (final id in ids) {
        final candidate = byId[id];
        if (candidate != null && candidate.availability != 'hidden') {
          list.add(candidate);
        }
      }
    } else {
      for (final candidate in state.candidates) {
        if (candidate.availability != 'hidden') list.add(candidate);
      }
    }
    // An empty input is Context Home, which shows a short list; a query scrolls.
    final searching = (_state?.input ?? '').trim().isNotEmpty;
    if (!searching && list.length > _homeLimit) {
      final shown = list.sublist(0, _homeLimit);
      // The limit is a *view* limit, so it must never hide the row the user is on: a late
      // update can push their selection past it while the candidate is still perfectly
      // present and executable. Keeping it as an extra row preserves the identity they
      // chose instead of silently jumping them back to the first entry.
      final selected = _selectedId;
      if (selected != null && !shown.any((candidate) => candidate.id == selected)) {
        final held = list.where((candidate) => candidate.id == selected);
        if (held.isNotEmpty) return [...shown, held.first];
      }
      return shown;
    }
    return list;
  }

  int get _selectedIndex {
    final candidates = _visibleCandidates;
    final id = _selectedId;
    if (id != null) {
      final index = candidates.indexWhere((candidate) => candidate.id == id);
      if (index >= 0) return index;
    }
    return 0;
  }

  /// Why there is nothing to show, phrased so the user can act on it.
  String get _emptyReason {
    if (_resolving) return 'Searching…';
    final state = _state;
    if (state != null && !state.contextComplete) {
      return 'Looking for actions in this context…';
    }
    final query = _input.text.trim();
    if (query.isNotEmpty) return 'No actions or apps match “$query”';
    return 'No actions for this context';
  }

  String? get _notice =>
      _message ??
      ((_state?.diagnostics.isNotEmpty ?? false)
          ? _state!.diagnostics.join('\n')
          : null);
  double get _previewHeight =>
      ((_output?.split('\n').length ?? 0).clamp(3, 14) * 20.0) + 24;
  /// True when a session is open but there is nothing to list.
  bool get _isEmpty =>
      _output == null && _visibleCandidates.isEmpty && _state != null;

  double get _height {
    final candidates = _visibleCandidates;
    final body = _output != null
        ? _previewHeight
        : candidates.length.clamp(0, _maxRows) * _rowHeight + 8;
    final scope = _scopes.length > 1 ? _scopeHeight : 0.0;
    // A panel with zero rows used to collapse the window to a bare search bar, which
    // said nothing about why it was empty. It now keeps a short explanation row.
    final panel = _output != null
        ? _headerHeight + body + _footerHeight + 2
        : candidates.isNotEmpty
            ? _headerHeight + body + _footerHeight + 2
            : _isEmpty
                ? _emptyHeight
                : 0.0;
    return (_inputHeight + scope + (_notice == null ? 0 : 44) + panel)
        .clamp(_inputHeight, 520.0);
  }

  @override
  void initState() {
    super.initState();
    widget.window.setMethodCallHandler((call) async {
      if (call.method == 'toggle') {
        if (_visible || _activating) {
          await _hide();
        } else {
          await _activate();
        }
      } else if (call.method == 'dismiss' &&
          (call.arguments as Map?)?['generation'] == _generation) {
        await _hide(hostAlreadyHidden: true);
      }
    });
    Future<void>.microtask(() async {
      if (mounted) await widget.window.invokeMethod<void>('ready');
    });
  }

  Future<void> _syncWindow({bool show = false}) async {
    if (!mounted || !_visible) return;
    await widget.window.invokeMethod<void>(show ? 'show' : 'resize', {
      'generation': _generation,
      'height': _height,
    });
  }

  /// Let the next snapshot try again for icons that failed.
  void _resetIconFailures() {
    if (_iconFailures.isNotEmpty) _iconFailures.clear();
    _iconRequests.clear();
  }

  /// Fetch icons for the application rows that are about to be drawn.
  ///
  /// Failures leave the semantic glyph in place and never reorder or block rows.
  void _requestIcons(List<CandidateDto> candidates) {
    final load = widget.loadIcon;
    if (load == null) return;
    for (final candidate in candidates) {
      if (candidate.kind != 'application') continue;
      if (_icons.containsKey(candidate.id) ||
          _iconFailures.contains(candidate.id) ||
          _iconRequests.contains(candidate.id)) {
        continue;
      }
      _iconRequests.add(candidate.id);
      unawaited(() async {
        Uint8List? bytes;
        try {
          bytes = await load(candidate.id);
        } catch (_) {
          bytes = null;
        }
        if (!mounted) return;
        setState(() {
          _iconRequests.remove(candidate.id);
          if (bytes == null || bytes.isEmpty) {
            _iconFailures.add(candidate.id);
          } else {
            _icons[candidate.id] = bytes;
            _trimIcons();
          }
        });
      }());
    }
  }

  /// Evict the oldest entries once the cache is over its limit.
  ///
  /// Order is insertion order, so this is FIFO rather than LRU; that is enough
  /// because rows do not change identity and every visible row is requested again
  /// on the next snapshot.
  void _trimIcons() {
    while (_icons.length > _iconCacheLimit) {
      _icons.remove(_icons.keys.first);
    }
  }

  /// Choose the row to highlight after a snapshot is committed.
  ///
  /// Untouched selection follows the new first row; a user's choice survives late
  /// enrichment and only falls back when their row really disappeared.
  void _reconcileSelection(List<CandidateDto> candidates) {
    final current = _selectedId;
    if (!_userMovedSelection) {
      _selectedId = candidates.isEmpty ? null : candidates.first.id;
      return;
    }
    if (current != null &&
        candidates.any((candidate) => candidate.id == current)) {
      return;
    }
    _selectedId = candidates.isEmpty ? null : candidates.first.id;
    _userMovedSelection = false;
  }

  /// Accept a committed snapshot, rejecting anything older than what we hold.
  void _applyState(LauncherStateDto state, {required bool fromLateUpdate}) {
    // Nothing older than what this window asked for may be displayed, and this window
    // asked for at least the revision currently on screen.
    if (state.revision < _requestedRevision) return;
    final held = _state;
    if (held != null && held.sessionId == state.sessionId) {
      if (state.revision < held.revision) return;
      if (state.revision == held.revision &&
          state.snapshotSeq <= held.snapshotSeq) {
        return;
      }
    }
    _state = state;
    _revision = state.revision;
    if (_scopeId != 'all' &&
        !state.scopes.any((scope) => scope.id == _scopeId)) {
      _scopeId = 'all';
    }
    if (_scopeId == 'all' && state.scopes.isEmpty) {
      _scopeId = 'all';
    }
    _reconcileSelection(_visibleCandidates);
    _requestIcons(_visibleCandidates);
    if (fromLateUpdate && _scroll.hasClients) {
      // Keep the user's view stable; only ensure their row is visible.
      _ensureSelectedVisible();
    }
  }

  Future<void> _activate() async {
    _activating = true;
    final generation = ++_generation;
    try {
      final state = await widget.activate();
      if (!mounted || generation != _generation || state == null) {
        if (state != null) await widget.deactivate(state.sessionId);
        return;
      }
      _input.clear();
      setState(() {
        _state = null;
        _selectedId = null;
        _userMovedSelection = false;
        _scopeId = 'all';
        _requestedRevision = state.revision;
        _message = widget.startupMessage.isEmpty ? null : widget.startupMessage;
        _output = null;
        _applyState(state, fromLateUpdate: false);
        _visible = true;
        _resolving = false;
        _executing = false;
      });
      // Capture/ranking finish before showing; a hidden window need not render a frame.
      await _syncWindow(show: true);
      if (mounted && _visible && generation == _generation) {
        _focus.requestFocus();
        // The session's single listener starts here.
        _armFollow(state.sessionId);
      }
    } catch (error) {
      if (mounted && generation == _generation) {
        setState(() {
          _message = '$error';
          _visible = true;
        });
        await _syncWindow(show: true);
        if (mounted && _visible && generation == _generation) {
          _focus.requestFocus();
        }
      }
    } finally {
      if (generation == _generation) _activating = false;
    }
  }

  /// Follow one session for as long as its window is up.
  ///
  /// Exactly one of these runs per visible session. It is started once at activation and
  /// re-armed by [updateInput] when the input changes; starting a second listener per
  /// keystroke left one bridge wait per input behind, each of them cloning the whole state
  /// every time anything was published.
  ///
  /// An elapsed wait is not the end of the session: the core may still publish the
  /// application index or a slow late result, so the loop re-arms from the same cursor. It
  /// ends when the window is hidden, when the session changes, or when the core is gone.
  Future<void> _followLateContext(int generation, int session) async {
    final wait = widget.waitForSnapshot;
    if (wait == null) return;
    var cursor = _state == null ? 0 : snapshotCursor(_state!);
    while (mounted && _visible && generation == _generation) {
      final next = await wait(session, cursor);
      if (!mounted || !_visible || generation != _generation) return;
      // Nothing newer: wait again rather than treat this as the end.
      if (next == null) continue;
      if (next.sessionId != session) return;
      final nextCursor = snapshotCursor(next);
      if (nextCursor <= cursor) continue;
      cursor = nextCursor;
      setState(() => _applyState(next, fromLateUpdate: true));
      await _syncWindow();
    }
  }

  /// Point the session's single listener at the current snapshot.
  void _armFollow(int session) {
    final generation = _generation;
    unawaited(_followLateContext(generation, session));
  }

  Future<void> _hide({bool hostAlreadyHidden = false}) async {
    final generation = _generation;
    final session = _state?.sessionId;
    ++_generation; // Invalidate input, execution and activation responses before any await.
    _focus.unfocus();
    _input.clear();
    setState(() {
      _visible = false;
      _activating = false;
      _resolving = false;
      _executing = false;
      _state = null;
      _selectedId = null;
      _userMovedSelection = false;
      _scopeId = 'all';
      _output = null;
      _message = null;
      _icons.clear();
      _iconRequests.clear();
      _iconFailures.clear();
    });
    final invalidation =
        session == null ? Future<void>.value() : widget.deactivate(session);
    if (!hostAlreadyHidden) {
      await widget.window
          .invokeMethod<void>('hide', {'generation': generation});
    }
    await invalidation;
  }

  Future<void> _update(String input) async {
    final session = _state?.sessionId;
    if (session == null || !_visible) return;
    final revision = ++_revision;
    _requestedRevision = revision;
    final generation = _generation;
    setState(() {
      _resolving = true;
      _executing = false;
      _output = null;
      _message = null;
      _userMovedSelection = false;
      _resetIconFailures();
    });
    try {
      final state = await widget.updateInput(session, input, revision);
      if (!mounted ||
          !_visible ||
          generation != _generation ||
          revision != _revision ||
          state == null ||
          state.sessionId != session ||
          state.revision != revision) {
        return;
      }
      setState(() {
        _selectedId = null;
        _scopeId = 'all';
        _applyState(state, fromLateUpdate: false);
        _resolving = false;
      });
      if (_scroll.hasClients) _scroll.jumpTo(0);
      await _syncWindow();
      // The listener started at activation is already following this session and picks the
      // new revision up from the published cursor; no second subscription is started here.
    } catch (error) {
      if (mounted &&
          _visible &&
          generation == _generation &&
          revision == _revision) {
        setState(() {
          _message = '$error';
          _resolving = false;
        });
        await _syncWindow();
      }
    }
  }

  Future<void> _execute() async {
    final state = _state;
    final candidates = _visibleCandidates;
    if (state == null ||
        candidates.isEmpty ||
        _resolving ||
        _executing ||
        !_visible ||
        state.revision != _revision) {
      return;
    }
    final candidate = candidates[_selectedIndex];
    if (candidate.availability != 'enabled') {
      setState(() => _message = candidate.availabilityReason ??
          'This action is not available right now.');
      await _syncWindow();
      return;
    }
    final generation = _generation;
    final revision = _revision;
    setState(() => _executing = true);
    try {
      final result = await widget.execute(state.sessionId, revision, candidate.id);
      if (!mounted ||
          !_visible ||
          generation != _generation ||
          revision != _revision ||
          result.sessionId != state.sessionId ||
          result.revision != revision) {
        return;
      }
      if (result.success && result.closeWindow) {
        await _hide();
      } else {
        setState(() {
          _message = result.message == 'Done' ? null : result.message;
          _output = result.outputText;
        });
        await _syncWindow();
      }
    } catch (error) {
      if (mounted &&
          _visible &&
          generation == _generation &&
          revision == _revision) {
        setState(() => _message = '$error');
        await _syncWindow();
      }
    } finally {
      if (mounted && generation == _generation && revision == _revision) {
        setState(() => _executing = false);
      }
    }
  }

  void _move(int direction) {
    final candidates = _visibleCandidates;
    if (_resolving || candidates.isEmpty) return;
    final next = (_selectedIndex + direction) % candidates.length;
    setState(() {
      _selectedId = candidates[next].id;
      _userMovedSelection = true;
      _output = null;
    });
    _syncWindow();
    _ensureSelectedVisible();
  }

  /// Option/Alt + arrows cycle scopes; plain arrows stay with the text caret.
  void _cycleScope(int direction) {
    final scopes = _scopes;
    if (scopes.length < 2) return;
    final index = scopes.indexWhere((scope) => scope.id == _scopeId);
    final base = index < 0 ? 0 : index;
    final next = (base + direction + scopes.length) % scopes.length;
    setState(() {
      _scopeId = scopes[next].id;
      _output = null;
      _userMovedSelection = false;
      _resetIconFailures();
    });
    _reconcileSelection(_visibleCandidates);
    _requestIcons(_visibleCandidates);
    _ensureSelectedVisible();
    _syncWindow();
  }

  void _ensureSelectedVisible() {
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!mounted || !_visible || !_scroll.hasClients) return;
      final index = _selectedIndex;
      final top = index * _rowHeight;
      final bottom = top + _rowHeight;
      if (top < _scroll.offset) _scroll.jumpTo(top);
      if (bottom > _scroll.offset + _scroll.position.viewportDimension) {
        _scroll.jumpTo((bottom - _scroll.position.viewportDimension)
            .clamp(0, _scroll.position.maxScrollExtent));
      }
    });
  }

  KeyEventResult _onKey(FocusNode node, KeyEvent event) {
    if (event is! KeyDownEvent && event is! KeyRepeatEvent) {
      return KeyEventResult.ignored;
    }
    // Leave candidate selection to the input method until composition has finished.
    if (!_input.value.composing.isCollapsed) return KeyEventResult.ignored;
    final scopeSwitch = HardwareKeyboard.instance.isAltPressed ||
        HardwareKeyboard.instance.isMetaPressed;
    if (event.logicalKey == LogicalKeyboardKey.escape) {
      _hide();
    } else if (event.logicalKey == LogicalKeyboardKey.arrowDown) {
      _move(1);
    } else if (event.logicalKey == LogicalKeyboardKey.arrowUp) {
      _move(-1);
    } else if (event.logicalKey == LogicalKeyboardKey.arrowLeft && scopeSwitch) {
      _cycleScope(-1);
    } else if (event.logicalKey == LogicalKeyboardKey.arrowRight && scopeSwitch) {
      _cycleScope(1);
    } else if (event.logicalKey == LogicalKeyboardKey.enter ||
        event.logicalKey == LogicalKeyboardKey.numpadEnter) {
      _execute();
    } else {
      return KeyEventResult.ignored;
    }
    return KeyEventResult.handled;
  }

  @override
  void dispose() {
    ++_generation;
    widget.window.setMethodCallHandler(null);
    _input.dispose();
    _focus.dispose();
    _scroll.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final candidates = _visibleCandidates;
    final hasPanel = _output != null || candidates.isNotEmpty;
    final contexts = (_state?.contexts ?? []);
    final labels = contexts.map((c) => c.label).toSet().join(' · ');
    return Align(
      alignment: Alignment.topCenter,
      child: SizedBox(
        key: const Key('launcher-panel'),
        width: 648,
        height: _height,
        child: Material(
          // AppKit supplies the real behind-window blur; keep this tint translucent.
          color: const Color(0xcc1b1d23),
          shape: RoundedRectangleBorder(
            borderRadius: BorderRadius.circular(12),
            side: const BorderSide(color: Color(0x38ffffff), width: 0.7),
          ),
          clipBehavior: Clip.antiAlias,
          child: Focus(
            onKeyEvent: _onKey,
            child: Column(children: [
              SizedBox(
                height: _inputHeight,
                child: Padding(
                  padding: const EdgeInsets.symmetric(horizontal: 20),
                  child: Row(children: [
                    const Icon(Icons.search_rounded,
                        size: 24, color: Color(0xffd0d6e1)),
                    const SizedBox(width: 14),
                    Expanded(
                        child: TextField(
                      controller: _input,
                      focusNode: _focus,
                      autofocus: true,
                      style: const TextStyle(
                          fontSize: 18, color: Color(0xfff7f8fc)),
                      cursorColor: const Color(0xffd5d7df),
                      decoration: const InputDecoration(
                        hintText: 'Search for an action or app…',
                        hintStyle: TextStyle(
                            color: Color(0xffc2c7d2),
                            fontWeight: FontWeight.w400),
                        border: InputBorder.none,
                        isDense: true,
                      ),
                      onChanged: _update,
                    )),
                    const SizedBox(width: 12),
                    if (_resolving || _executing)
                      const SizedBox(
                          width: 14,
                          height: 14,
                          child: CircularProgressIndicator(
                              strokeWidth: 1.5, color: Color(0xffd0d6e1)))
                    else
                      _keycap('esc'),
                  ]),
                ),
              ),
              // A single scope is not a choice: hide the bar when there is nothing
                // to switch between, which is exactly the empty case.
                if (_scopes.length > 1) _scopeBar(),
              if (_notice != null)
                SizedBox(
                    height: 44,
                    child: Padding(
                      padding: const EdgeInsets.symmetric(
                          horizontal: 20, vertical: 5),
                      child: Tooltip(
                          message: _notice!,
                          child: Align(
                            alignment: Alignment.centerLeft,
                            child: Text(_notice!,
                                maxLines: 2,
                                overflow: TextOverflow.ellipsis,
                                style: const TextStyle(
                                    fontSize: 11, color: Color(0xffe3bc83))),
                          )),
                    )),
              if (_isEmpty && _output == null)
                SizedBox(height: _emptyHeight, child: _emptyState()),
              if (hasPanel) ...[
                const Divider(
                    height: 1, thickness: 1, color: Color(0x24ffffff)),
                SizedBox(
                    height: _headerHeight,
                    child: Padding(
                      padding: const EdgeInsets.symmetric(horizontal: 18),
                      child: Row(children: [
                        Text(_output == null ? 'Suggested actions' : 'Result',
                            style: const TextStyle(
                                fontSize: 11,
                                fontWeight: FontWeight.w500,
                                color: Color(0xffd0d6e1))),
                        const SizedBox(width: 16),
                        Expanded(
                            child: Text(labels,
                                textAlign: TextAlign.right,
                                maxLines: 1,
                                overflow: TextOverflow.ellipsis,
                                style: const TextStyle(
                                    fontSize: 11, color: Color(0xffc2c7d2)))),
                      ]),
                    )),
                Expanded(
                    child: _output != null
                        ? SingleChildScrollView(
                            key: const Key('result-preview'),
                            padding: const EdgeInsets.symmetric(
                                horizontal: 20, vertical: 12),
                            child: SelectableText(_output!,
                                style: const TextStyle(
                                    fontFamily: 'monospace',
                                    fontSize: 12,
                                    height: 1.65,
                                    color: Color(0xffe2e3e8))),
                          )
                        : ListView.builder(
                            key: const Key('action-list'),
                            controller: _scroll,
                            padding: const EdgeInsets.fromLTRB(8, 0, 8, 8),
                            itemExtent: _rowHeight,
                            itemCount: candidates.length,
                            itemBuilder: (context, index) =>
                                _candidateRow(candidates[index], index),
                          )),
                const Divider(
                    height: 1, thickness: 1, color: Color(0x24ffffff)),
                SizedBox(
                    height: _footerHeight,
                    child: Padding(
                      padding: const EdgeInsets.symmetric(horizontal: 16),
                      child: Row(children: [
                        const Icon(Icons.bolt_rounded,
                            size: 14, color: Color(0xffd0d6e1)),
                        const SizedBox(width: 6),
                        Text(
                            _output == null
                                ? '${candidates.length} results'
                                : 'Result',
                            style: const TextStyle(
                                fontSize: 11, color: Color(0xffc2c7d2))),
                        const Spacer(),
                        Text(_output == null ? 'Run' : 'Back to results',
                            style: const TextStyle(
                                fontSize: 11, color: Color(0xffd0d6e1))),
                        const SizedBox(width: 8),
                        _keycap(_output == null ? '↵' : '↑ ↓'),
                      ]),
                    )),
              ],
            ]),
          ),
        ),
      ),
    );
  }

  Widget _emptyState() => Padding(
        key: const Key('empty-state'),
        padding: const EdgeInsets.symmetric(horizontal: 20),
        child: Row(children: [
          Icon(
              _state?.contextComplete ?? true
                  ? Icons.search_off_rounded
                  : Icons.hourglass_empty_rounded,
              size: 16,
              color: const Color(0xff9aa0ad)),
          const SizedBox(width: 10),
          Expanded(
            child: Text(_emptyReason,
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: const TextStyle(fontSize: 12, color: Color(0xffc2c7d2))),
          ),
        ]),
      );

  Widget _scopeBar() {
    final scopes = _scopes;
    final active = _scope?.id;
    return SizedBox(
      height: _scopeHeight,
      child: ListView(
        key: const Key('scope-bar'),
        scrollDirection: Axis.horizontal,
        padding: const EdgeInsets.symmetric(horizontal: 14),
        children: [
          for (final scope in scopes)
            Padding(
              padding: const EdgeInsets.symmetric(horizontal: 3, vertical: 4),
              child: _scopeChip(scope, scope.id == active),
            ),
        ],
      ),
    );
  }

  Widget _scopeChip(ScopeDto scope, bool selected) => Material(
        color: selected ? const Color(0x59a7b4cb) : Colors.transparent,
        borderRadius: BorderRadius.circular(6),
        child: InkWell(
          borderRadius: BorderRadius.circular(6),
          onTap: () {
            setState(() {
              _scopeId = scope.id;
              _output = null;
              _userMovedSelection = false;
              _resetIconFailures();
            });
            _reconcileSelection(_visibleCandidates);
            _requestIcons(_visibleCandidates);
            _syncWindow();
          },
          child: Padding(
            padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 4),
            child: Text(scope.label,
                style: TextStyle(
                    fontSize: 11,
                    fontWeight: selected ? FontWeight.w600 : FontWeight.w400,
                    color: const Color(0xffdbe0ea))),
          ),
        ),
      );

  Widget _candidateRow(CandidateDto candidate, int index) {
    final selected = index == _selectedIndex;
    final disabled = candidate.availability != 'enabled';
    return Semantics(
      selected: selected,
      button: true,
      child: Material(
        color: selected ? const Color(0x665c6070) : Colors.transparent,
        borderRadius: BorderRadius.circular(7),
        child: InkWell(
          borderRadius: BorderRadius.circular(7),
          onTap: () {
            if (_resolving) return;
            setState(() {
              _selectedId = candidate.id;
              _userMovedSelection = true;
            });
            _execute();
          },
          child: Padding(
            padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 6),
            child: Row(children: [
              Container(
                  width: 30,
                  height: 30,
                  decoration: BoxDecoration(
                      // Only the fallback glyph needs a plate: a real icon is opaque.
                      color: _icons.containsKey(candidate.id)
                          ? Colors.transparent
                          : const Color(0x4d454955),
                      borderRadius: BorderRadius.circular(7)),
                  clipBehavior: Clip.antiAlias,
                  child: _iconFor(candidate, disabled)),
              const SizedBox(width: 12),
              Expanded(
                  child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                mainAxisAlignment: MainAxisAlignment.center,
                children: [
                  Text(candidate.title,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: TextStyle(
                          fontSize: 13,
                          fontWeight: FontWeight.w500,
                          color: disabled
                              ? const Color(0xff9aa0ad)
                              : const Color(0xffebebef))),
                  if (candidate.subtitle != null || disabled)
                    Text(
                        disabled
                            ? (candidate.availabilityReason ??
                                'Not available right now')
                            : candidate.subtitle!,
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                        style: TextStyle(
                            fontSize: 11,
                            height: 1.5,
                            color: disabled
                                ? const Color(0xffc9a26b)
                                : const Color(0xffd0d6e1))),
                ],
              )),
              if (selected && !disabled) ...[
                const SizedBox(width: 12),
                const Icon(Icons.keyboard_return_rounded,
                    size: 16, color: Color(0xffa8abb7)),
              ],
            ]),
          ),
        ),
      ),
    );
  }

  /// Real application icon when available, semantic glyph otherwise.
  Widget _iconFor(CandidateDto candidate, bool disabled) {
    final bytes = _icons[candidate.id];
    if (bytes != null) {
      // `cover` fills the slot the way the system's own icon renderer does. `contain`
      // letterboxes, which showed up as a transparent frame around the artwork.
      return Image.memory(bytes,
          width: 30,
          height: 30,
          fit: BoxFit.cover,
          gaplessPlayback: true,
          errorBuilder: (_, __, ___) => _glyph(candidate, disabled));
    }
    return _glyph(candidate, disabled);
  }

  Widget _glyph(CandidateDto candidate, bool disabled) => Icon(
        _icon(candidate),
        size: 17,
        color: disabled ? const Color(0xff8a8f9c) : const Color(0xffcad0de),
      );

  Widget _keycap(String label) => Container(
        padding: const EdgeInsets.symmetric(horizontal: 5, vertical: 2),
        decoration: BoxDecoration(
            color: const Color(0x4d454955),
            border: Border.all(color: const Color(0xff484a52), width: 0.5),
            borderRadius: BorderRadius.circular(4)),
        child: Text(label,
            style: const TextStyle(
                fontSize: 10, height: 1.1, color: Color(0xffd0d6e1))),
      );

  IconData _icon(CandidateDto candidate) {
    if (candidate.kind == 'application') return Icons.apps_rounded;
    return switch (candidate.iconKey) {
      'terminal' => Icons.terminal_rounded,
      'folder' => Icons.folder_open_rounded,
      'file' => Icons.insert_drive_file_outlined,
      'copy' => Icons.content_copy_rounded,
      'globe' => Icons.language_rounded,
      'code' => Icons.data_object_rounded,
      'search' => Icons.search_rounded,
      'git' => Icons.account_tree_outlined,
      _ => Icons.bolt_rounded,
    };
  }
}
