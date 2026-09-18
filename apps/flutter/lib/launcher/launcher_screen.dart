import 'package:flutter/material.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';
import '../src/rust/api.dart';
import '../infrastructure/window_host.dart';
import 'launcher_controller.dart';
import 'launcher_view_state.dart';
import 'widgets/candidate_row.dart';

export 'launcher_controller.dart'
    show
        BeginActivation,
        Activate,
        PromptAccessibility,
        UpdateInput,
        Execute,
        WaitForSnapshot,
        LoadIcon,
        snapshotCursor,
        WakeMode,
        parseWakeMode;

class LauncherApp extends StatelessWidget {
  const LauncherApp({
    super.key,
    required this.beginActivation,
    required this.activate,
    required this.updateInput,
    required this.execute,
    required this.deactivate,
    required this.window,
    this.waitForSnapshot,
    this.loadIcon,
    this.promptAccessibility,
    this.startupMessage = '',
  });
  final BeginActivation beginActivation;
  final Activate activate;
  final UpdateInput updateInput;
  final Execute execute;
  final Future<void> Function(int) deactivate;
  final MethodChannel window;

  /// Awaits late context. Absent in tests that only exercise the first snapshot.
  final WaitForSnapshot? waitForSnapshot;

  /// Loads application icons lazily. Absent in tests that do not need icons.
  final LoadIcon? loadIcon;
  final PromptAccessibility? promptAccessibility;
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
          beginActivation: beginActivation,
          activate: activate,
          updateInput: updateInput,
          execute: execute,
          deactivate: deactivate,
          window: window,
          waitForSnapshot: waitForSnapshot,
          loadIcon: loadIcon,
          promptAccessibility: promptAccessibility,
          startupMessage: startupMessage,
        ),
      );
}

class LauncherScreen extends StatefulWidget {
  const LauncherScreen({
    super.key,
    required this.beginActivation,
    required this.activate,
    required this.updateInput,
    required this.execute,
    required this.deactivate,
    required this.window,
    required this.startupMessage,
    this.waitForSnapshot,
    this.loadIcon,
    this.promptAccessibility,
  });
  final BeginActivation beginActivation;
  final Activate activate;
  final UpdateInput updateInput;
  final Execute execute;
  final Future<void> Function(int) deactivate;
  final MethodChannel window;
  final WaitForSnapshot? waitForSnapshot;
  final LoadIcon? loadIcon;
  final PromptAccessibility? promptAccessibility;
  final String startupMessage;
  @override
  State<LauncherScreen> createState() => _LauncherScreenState();
}

class _LauncherScreenState extends State<LauncherScreen> {
  final _input = TextEditingController();
  final _focus = FocusNode();
  final _scroll = ScrollController();
  late final LauncherController _controller;
  LauncherViewState get _view => _controller.view;
  LauncherStateDto? get _state => _view.snapshot;
  List<CandidateDto> get _visibleCandidates => _view.visibleCandidates;
  List<ScopeDto> get _scopes => _view.scopes;
  ScopeDto? get _scope => _view.scope;
  int get _selectedIndex => _view.selectedIndex;
  String? get _output => _view.output;
  String? get _notice => _view.notice;
  bool get _resolving => _view.resolving;
  bool get _executing => _view.executing;
  bool get _visible => _view.visible;
  double get _height => _view.height;
  static const _rowHeight = LauncherViewState.rowHeight,
      _inputHeight = LauncherViewState.inputHeight,
      _scopeHeight = LauncherViewState.scopeHeight,
      _headerHeight = LauncherViewState.headerHeight,
      _footerHeight = LauncherViewState.footerHeight;
  bool _wasVisible = false;
  int? _displayedRevision;

  @override
  void initState() {
    super.initState();
    _controller = LauncherController(
      beginActivation: widget.beginActivation,
      activate: widget.activate,
      updateInput: widget.updateInput,
      execute: widget.execute,
      deactivate: widget.deactivate,
      window: WindowHost(widget.window),
      waitForSnapshot: widget.waitForSnapshot,
      loadIcon: widget.loadIcon,
      promptAccessibility: widget.promptAccessibility,
      startupMessage: widget.startupMessage,
    );
    _controller.addListener(_refresh);
    _controller.attach();
  }

  void _refresh() {
    if (!mounted) return;
    if (!_visible || !_wasVisible) {
      _input.clear();
    } else if (_view.input.isEmpty && _input.text.isNotEmpty) {
      _input.clear();
    }
    if (_visible) {
      if (!_wasVisible) _focus.requestFocus();
      if (_state?.revision != _displayedRevision && _scroll.hasClients) {
        _scroll.jumpTo(0);
      }
      _ensureSelectedVisible();
    } else {
      _focus.unfocus();
    }
    _wasVisible = _visible;
    _displayedRevision = _state?.revision;
    setState(() {});
  }

  @override
  void dispose() {
    _controller.removeListener(_refresh);
    _controller.dispose();
    _input.dispose();
    _focus.dispose();
    _scroll.dispose();
    super.dispose();
  }

  void _ensureSelectedVisible() {
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!mounted || !_visible || !_scroll.hasClients) return;
      final index = _selectedIndex;
      final top = index * _rowHeight;
      final bottom = top + _rowHeight;
      if (top < _scroll.offset) _scroll.jumpTo(top);
      if (bottom > _scroll.offset + _scroll.position.viewportDimension) {
        _scroll.jumpTo(
          (bottom - _scroll.position.viewportDimension).clamp(
            0,
            _scroll.position.maxScrollExtent,
          ),
        );
      }
    });
  }

  KeyEventResult _onKey(FocusNode node, KeyEvent event) {
    if (event is! KeyDownEvent && event is! KeyRepeatEvent) {
      return KeyEventResult.ignored;
    }
    // Leave candidate selection to the input method until composition has finished.
    if (!_input.value.composing.isCollapsed) return KeyEventResult.ignored;
    final control = HardwareKeyboard.instance.isControlPressed;
    if (event.logicalKey == LogicalKeyboardKey.escape) {
      _controller.escape();
    } else if (event.logicalKey == LogicalKeyboardKey.arrowDown) {
      _controller.move(1);
    } else if (event.logicalKey == LogicalKeyboardKey.arrowUp) {
      _controller.move(-1);
    } else if (event.logicalKey == LogicalKeyboardKey.tab && control) {
      _controller.cycleScope(
        HardwareKeyboard.instance.isShiftPressed ? -1 : 1,
      );
    } else if (event.logicalKey == LogicalKeyboardKey.enter ||
        event.logicalKey == LogicalKeyboardKey.numpadEnter) {
      _controller.executeSelected();
    } else {
      return KeyEventResult.ignored;
    }
    return KeyEventResult.handled;
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
            child: Column(
              children: [
                SizedBox(
                  height: _inputHeight,
                  child: Padding(
                    padding: const EdgeInsets.symmetric(horizontal: 20),
                    child: Row(
                      children: [
                        const Icon(
                          Icons.search_rounded,
                          size: 24,
                          color: Color(0xffd0d6e1),
                        ),
                        const SizedBox(width: 14),
                        Expanded(
                          child: TextField(
                            controller: _input,
                            focusNode: _focus,
                            autofocus: true,
                            style: const TextStyle(
                              fontSize: 18,
                              color: Color(0xfff7f8fc),
                            ),
                            cursorColor: const Color(0xffd5d7df),
                            decoration: const InputDecoration(
                              hintText: 'Search for an action or app…',
                              hintStyle: TextStyle(
                                color: Color(0xffc2c7d2),
                                fontWeight: FontWeight.w400,
                              ),
                              border: InputBorder.none,
                              isDense: true,
                            ),
                            onChanged: _controller.update,
                          ),
                        ),
                        const SizedBox(width: 12),
                        if (_resolving || _executing)
                          const SizedBox(
                            width: 14,
                            height: 14,
                            child: CircularProgressIndicator(
                              strokeWidth: 1.5,
                              color: Color(0xffd0d6e1),
                            ),
                          )
                        else
                          _keycap('esc'),
                      ],
                    ),
                  ),
                ),
                // A single scope is not a choice: hide the bar when there is nothing
                // to switch between, which is exactly the empty case.
                if (_scopes.isNotEmpty) _scopeBar(),
                if (_notice != null)
                  SizedBox(
                    height: 44,
                    child: Padding(
                      padding: const EdgeInsets.symmetric(
                        horizontal: 20,
                        vertical: 5,
                      ),
                      child: Tooltip(
                        message: _notice!,
                        child: Align(
                          alignment: Alignment.centerLeft,
                          child: Text(
                            _notice!,
                            maxLines: 2,
                            overflow: TextOverflow.ellipsis,
                            style: const TextStyle(
                              fontSize: 11,
                              color: Color(0xffe3bc83),
                            ),
                          ),
                        ),
                      ),
                    ),
                  ),
                if (hasPanel) ...[
                  const Divider(
                    height: 1,
                    thickness: 1,
                    color: Color(0x24ffffff),
                  ),
                  SizedBox(
                    height: _headerHeight,
                    child: Padding(
                      padding: const EdgeInsets.symmetric(horizontal: 18),
                      child: Row(
                        children: [
                          Text(
                            _output == null ? 'Suggested actions' : 'Result',
                            style: const TextStyle(
                              fontSize: 11,
                              fontWeight: FontWeight.w500,
                              color: Color(0xffd0d6e1),
                            ),
                          ),
                          const SizedBox(width: 16),
                          Expanded(
                            child: Text(
                              labels,
                              textAlign: TextAlign.right,
                              maxLines: 1,
                              overflow: TextOverflow.ellipsis,
                              style: const TextStyle(
                                fontSize: 11,
                                color: Color(0xffc2c7d2),
                              ),
                            ),
                          ),
                        ],
                      ),
                    ),
                  ),
                  Expanded(
                    child: _output != null
                        ? SingleChildScrollView(
                            key: const Key('result-preview'),
                            padding: const EdgeInsets.symmetric(
                              horizontal: 20,
                              vertical: 12,
                            ),
                            child: SelectableText(
                              _output!,
                              style: const TextStyle(
                                fontFamily: 'monospace',
                                fontSize: 12,
                                height: 1.65,
                                color: Color(0xffe2e3e8),
                              ),
                            ),
                          )
                        : ListView.builder(
                            key: const Key('action-list'),
                            controller: _scroll,
                            padding: const EdgeInsets.fromLTRB(8, 0, 8, 8),
                            itemExtent: _rowHeight,
                            itemCount: candidates.length,
                            itemBuilder: (context, index) =>
                                _candidateRow(candidates[index], index),
                          ),
                  ),
                  const Divider(
                    height: 1,
                    thickness: 1,
                    color: Color(0x24ffffff),
                  ),
                  SizedBox(
                    height: _footerHeight,
                    child: Padding(
                      padding: const EdgeInsets.symmetric(horizontal: 16),
                      child: Row(
                        children: [
                          const Icon(
                            Icons.bolt_rounded,
                            size: 14,
                            color: Color(0xffd0d6e1),
                          ),
                          const SizedBox(width: 6),
                          Text(
                            _output == null
                                ? '${candidates.length} results'
                                : 'Result',
                            style: const TextStyle(
                              fontSize: 11,
                              color: Color(0xffc2c7d2),
                            ),
                          ),
                          const Spacer(),
                          Text(
                            _output == null ? 'Run' : 'Back to results',
                            style: const TextStyle(
                              fontSize: 11,
                              color: Color(0xffd0d6e1),
                            ),
                          ),
                          const SizedBox(width: 8),
                          _keycap(_output == null ? '↵' : '↑ ↓'),
                        ],
                      ),
                    ),
                  ),
                ],
              ],
            ),
          ),
        ),
      ),
    );
  }

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
          onTap: () => _controller.selectScope(scope.id),
          child: Padding(
            padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 4),
            child: Text(
              scope.label,
              style: TextStyle(
                fontSize: 11,
                fontWeight: selected ? FontWeight.w600 : FontWeight.w400,
                color: const Color(0xffdbe0ea),
              ),
            ),
          ),
        ),
      );

  Widget _candidateRow(CandidateDto candidate, int index) => CandidateRow(
        candidate: candidate,
        selected: index == _selectedIndex,
        iconBytes: _controller.icons.bytesFor(candidate.id),
        onTap: () => _controller.selectAndExecute(candidate.id),
      );

  Widget _keycap(String label) => Container(
        padding: const EdgeInsets.symmetric(horizontal: 5, vertical: 2),
        decoration: BoxDecoration(
          color: const Color(0x4d454955),
          border: Border.all(color: const Color(0xff484a52), width: 0.5),
          borderRadius: BorderRadius.circular(4),
        ),
        child: Text(
          label,
          style: const TextStyle(
            fontSize: 10,
            height: 1.1,
            color: Color(0xffd0d6e1),
          ),
        ),
      );
}
