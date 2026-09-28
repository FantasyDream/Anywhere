import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import '../src/rust/api.dart';
import '../infrastructure/window_host.dart';
import 'launcher_controller.dart';
import 'launcher_style.dart';
import 'launcher_view_state.dart';
import 'widgets/candidate_row.dart';
import '../settings/settings_screen.dart';

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

class LauncherApp extends StatefulWidget {
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
  State<LauncherApp> createState() => _LauncherAppState();
}

class _LauncherAppState extends State<LauncherApp> {
  late final LauncherController _controller;

  @override
  void initState() {
    super.initState();
    _controller = LauncherController(
      backend: LauncherBackend(
        beginActivation: widget.beginActivation,
        activate: widget.activate,
        updateInput: widget.updateInput,
        execute: widget.execute,
        deactivate: widget.deactivate,
        waitForSnapshot: widget.waitForSnapshot,
        loadIcon: widget.loadIcon,
        promptAccessibility: widget.promptAccessibility,
      ),
      window: WindowHost(widget.window),
      startupMessage: widget.startupMessage,
    );
    _controller.attach();
  }

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => MaterialApp(
        debugShowCheckedModeBanner: false,
        title: 'Anywhere',
        theme: ThemeData(
          brightness: Brightness.dark,
          useMaterial3: true,
          colorScheme: ColorScheme.fromSeed(
            seedColor: const Color(0xff8ea2ff),
            brightness: Brightness.dark,
          ),
          scaffoldBackgroundColor: Colors.transparent,
          fontFamily: '.AppleSystemUIFont',
        ),
        home: LauncherScreen(controller: _controller),
      );
}

class LauncherScreen extends StatefulWidget {
  const LauncherScreen({super.key, required this.controller});
  final LauncherController controller;
  @override
  State<LauncherScreen> createState() => _LauncherScreenState();
}

class _LauncherScreenState extends State<LauncherScreen> {
  final _input = TextEditingController();
  final _focus = FocusNode();
  final _scroll = ScrollController();
  LauncherController get _controller => widget.controller;
  LauncherViewState get _view => _controller.view;
  bool _wasVisible = false;
  int? _displayedRevision;
  String? _submitted;

  void _onInput() {
    final value = _input.value;
    if (!value.composing.isCollapsed) return;
    if (value.text == _submitted) return;
    _submitted = value.text;
    _controller.update(value.text);
  }

  void _reportIconViewport() {
    final count = _view.visibleCandidates.length;
    if (count == 0 || _view.output != null) return;
    var first = 0;
    var last = LauncherViewState.maxRows;
    if (_scroll.hasClients) {
      first = (_scroll.offset / LauncherViewState.rowHeight).floor();
      final visible =
          (_scroll.position.viewportDimension / LauncherViewState.rowHeight)
              .ceil();
      last = first + visible;
    }
    _controller.noteIconViewport(first, last);
  }

  @override
  void initState() {
    super.initState();
    _controller.addListener(_refresh);
    _input.addListener(_onInput);
    _scroll.addListener(_reportIconViewport);
  }

  @override
  void didUpdateWidget(LauncherScreen oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.controller != widget.controller) {
      oldWidget.controller.removeListener(_refresh);
      widget.controller.addListener(_refresh);
    }
  }

  void _refresh() {
    if (!mounted) return;
    if (!_view.visible || !_wasVisible) {
      _submitted = '';
      _input.clear();
    } else if (_view.input.isEmpty && _input.text.isNotEmpty) {
      _submitted = '';
      _input.clear();
    }
    if (_view.visible) {
      if (!_wasVisible) _focus.requestFocus();
      if (_view.snapshot?.revision != _displayedRevision &&
          _scroll.hasClients) {
        _scroll.jumpTo(0);
      }
      _ensureSelectedVisible();
    } else {
      _focus.unfocus();
    }
    _wasVisible = _view.visible;
    _displayedRevision = _view.snapshot?.revision;
    _reportIconViewport();
    setState(() {});
  }

  @override
  void dispose() {
    _controller.removeListener(_refresh);
    _input.removeListener(_onInput);
    _scroll.removeListener(_reportIconViewport);
    _input.dispose();
    _focus.dispose();
    _scroll.dispose();
    super.dispose();
  }

  void _ensureSelectedVisible() {
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!mounted || !_view.visible || !_scroll.hasClients) return;
      final index = _view.selectedIndex;
      final top = index * LauncherViewState.rowHeight;
      final bottom = top + LauncherViewState.rowHeight;
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
    if (_view.phase == LauncherPhase.settings) {
      return Align(
        alignment: Alignment.topCenter,
        child: SizedBox(
          key: const Key('launcher-panel'),
          width: 648,
          height: _view.height,
          child: Material(
            color: LauncherStyle.surface,
            shape: LauncherStyle.panelShape,
            clipBehavior: Clip.antiAlias,
            child: SettingsScreen(controller: _controller),
          ),
        ),
      );
    }
    final candidates = _view.visibleCandidates;
    final hasPanel = _view.output != null || candidates.isNotEmpty;
    final contexts = (_view.snapshot?.contexts ?? []);
    final labels = contexts.map(_contextLabel).toSet().join(' · ');
    return Align(
      alignment: Alignment.topCenter,
      child: SizedBox(
        key: const Key('launcher-panel'),
        width: 648,
        height: _view.height,
        child: Material(
          color: LauncherStyle.surface,
          shape: LauncherStyle.panelShape,
          clipBehavior: Clip.antiAlias,
          child: Focus(
            onKeyEvent: _onKey,
            child: Column(
              children: [
                SizedBox(
                  height: LauncherViewState.inputHeight,
                  child: Padding(
                    padding: const EdgeInsets.symmetric(horizontal: 20),
                    child: Row(
                      children: [
                        Container(
                          width: 34,
                          height: 34,
                          decoration: BoxDecoration(
                            borderRadius: BorderRadius.circular(11),
                            gradient: const LinearGradient(
                              begin: Alignment.topLeft,
                              end: Alignment.bottomRight,
                              colors: [Color(0x448ba5ff), Color(0x225769ae)],
                            ),
                            border: Border.all(color: const Color(0x449eb4ff)),
                          ),
                          child: const Icon(
                            Icons.search_rounded,
                            size: 19,
                            color: LauncherStyle.primary,
                          ),
                        ),
                        const SizedBox(width: 12),
                        Expanded(
                          child: TextField(
                            controller: _input,
                            focusNode: _focus,
                            autofocus: true,
                            style: const TextStyle(
                              fontSize: 17,
                              fontWeight: FontWeight.w500,
                              letterSpacing: -0.2,
                              color: LauncherStyle.primary,
                            ),
                            cursorColor: LauncherStyle.accent,
                            cursorWidth: 1.4,
                            decoration: InputDecoration(
                              hintText: _view.searchHint,
                              hintStyle: const TextStyle(
                                color: LauncherStyle.muted,
                                fontWeight: FontWeight.w400,
                              ),
                              border: InputBorder.none,
                              isDense: true,
                            ),
                          ),
                        ),
                        const SizedBox(width: 12),
                        if (_view.resolving || _view.executing)
                          const SizedBox(
                            width: 14,
                            height: 14,
                            child: CircularProgressIndicator(
                              strokeWidth: 1.5,
                              color: LauncherStyle.accent,
                            ),
                          )
                        else
                          const LauncherKeycap('esc'),
                      ],
                    ),
                  ),
                ),
                // A single scope is not a choice: hide the bar when there is nothing
                // to switch between, which is exactly the empty case.
                if (_view.scopes.isNotEmpty) _scopeBar(),
                if (_view.notice != null)
                  SizedBox(
                    height: 44,
                    child: Padding(
                      padding: const EdgeInsets.symmetric(
                        horizontal: 16,
                        vertical: 4,
                      ),
                      child: Tooltip(
                        message: _view.notice!,
                        child: Container(
                          padding: const EdgeInsets.symmetric(horizontal: 10),
                          decoration: BoxDecoration(
                            color: const Color(0x2ce5b779),
                            borderRadius: BorderRadius.circular(9),
                            border: Border.all(color: const Color(0x44e5b779)),
                          ),
                          child: Row(
                            children: [
                              const Icon(Icons.info_outline_rounded,
                                  size: 15, color: LauncherStyle.warning),
                              const SizedBox(width: 8),
                              Expanded(
                                child: Text(
                                  _view.notice!,
                                  maxLines: 2,
                                  overflow: TextOverflow.ellipsis,
                                  style: const TextStyle(
                                      fontSize: 11,
                                      color: LauncherStyle.warning),
                                ),
                              ),
                            ],
                          ),
                        ),
                      ),
                    ),
                  ),
                if (hasPanel) ...[
                  const Divider(
                      height: 1, thickness: 1, color: LauncherStyle.divider),
                  SizedBox(
                    height: LauncherViewState.headerHeight,
                    child: Padding(
                      padding: const EdgeInsets.symmetric(horizontal: 18),
                      child: Row(
                        children: [
                          Text(
                            _view.resultHeading,
                            style: const TextStyle(
                              fontSize: 11,
                              fontWeight: FontWeight.w600,
                              letterSpacing: 0.4,
                              color: LauncherStyle.secondary,
                            ),
                          ),
                          const SizedBox(width: 16),
                          Expanded(
                            child: Text(
                              _view.output == null
                                  ? labels
                                  : (_view.resultSource ?? labels),
                              textAlign: TextAlign.right,
                              maxLines: 1,
                              overflow: TextOverflow.ellipsis,
                              style: const TextStyle(
                                fontSize: 11,
                                color: LauncherStyle.muted,
                              ),
                            ),
                          ),
                        ],
                      ),
                    ),
                  ),
                  Expanded(
                    child: _view.output != null
                        ? SingleChildScrollView(
                            key: const Key('result-preview'),
                            padding: const EdgeInsets.symmetric(
                              horizontal: 20,
                              vertical: 12,
                            ),
                            child: SelectableText(
                              _view.output!,
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
                            padding: const EdgeInsets.fromLTRB(6, 0, 6, 8),
                            itemExtent: LauncherViewState.rowHeight,
                            itemCount: candidates.length,
                            itemBuilder: (context, index) =>
                                _candidateRow(candidates[index], index),
                          ),
                  ),
                  const Divider(
                      height: 1, thickness: 1, color: LauncherStyle.divider),
                  SizedBox(
                    height: LauncherViewState.footerHeight,
                    child: Padding(
                      padding: const EdgeInsets.symmetric(horizontal: 16),
                      child: Row(
                        children: [
                          Text(
                            _view.output == null
                                ? '${candidates.length} results'
                                : (_view.resultStatus == ActionStatusDto.failed
                                    ? 'Failed'
                                    : 'Succeeded'),
                            style: const TextStyle(
                              fontSize: 11,
                              color: LauncherStyle.secondary,
                            ),
                          ),
                          const Spacer(),
                          if (_view.output != null) ...[
                            TextButton(
                              onPressed: _controller.copyResult,
                              style: TextButton.styleFrom(
                                minimumSize: const Size(48, 26),
                                tapTargetSize: MaterialTapTargetSize.shrinkWrap,
                                padding:
                                    const EdgeInsets.symmetric(horizontal: 8),
                                foregroundColor: LauncherStyle.primary,
                                textStyle: const TextStyle(fontSize: 11),
                              ),
                              child: const Text('Copy'),
                            ),
                            const SizedBox(width: 8),
                          ],
                          if (_view.output == null) ...[
                            const Text('↑↓  Navigate',
                                style: TextStyle(
                                    fontSize: 11, color: LauncherStyle.muted)),
                            const SizedBox(width: 16),
                          ],
                          if (_view.output == null)
                            const Text(
                              'Run',
                              style: TextStyle(
                                fontSize: 11,
                                fontWeight: FontWeight.w500,
                                color: LauncherStyle.secondary,
                              ),
                            )
                          else
                            TextButton(
                              onPressed: _controller.back,
                              style: TextButton.styleFrom(
                                minimumSize: const Size(48, 26),
                                tapTargetSize: MaterialTapTargetSize.shrinkWrap,
                                padding:
                                    const EdgeInsets.symmetric(horizontal: 8),
                                foregroundColor: LauncherStyle.primary,
                                textStyle: const TextStyle(fontSize: 11),
                              ),
                              child: const Text('Back'),
                            ),
                          const SizedBox(width: 6),
                          LauncherKeycap(_view.output == null ? '↵' : 'esc'),
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
    final scopes = _view.scopes;
    final active = _view.scope?.id;
    return SizedBox(
      height: LauncherViewState.scopeHeight,
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

  Widget _scopeChip(ScopeDto scope, bool selected) => Semantics(
        selected: selected,
        child: Material(
          color: selected ? const Color(0x4b7e98f2) : const Color(0x18ffffff),
          clipBehavior: Clip.antiAlias,
          shape: StadiumBorder(
            side: BorderSide(
              color:
                  selected ? const Color(0x99b5c5ff) : const Color(0x24ffffff),
            ),
          ),
          child: InkWell(
            customBorder: const StadiumBorder(),
            onTap: () => _controller.selectScope(scope.id),
            child: Padding(
              padding: const EdgeInsets.symmetric(horizontal: 11, vertical: 4),
              child: Text(
                scope.label,
                style: TextStyle(
                  fontSize: 11,
                  fontWeight: selected ? FontWeight.w600 : FontWeight.w500,
                  color: selected
                      ? LauncherStyle.primary
                      : LauncherStyle.secondary,
                ),
              ),
            ),
          ),
        ),
      );

  Widget _candidateRow(CandidateDto candidate, int index) => CandidateRow(
        candidate: candidate,
        selected: index == _view.selectedIndex,
        iconBytes: _controller.icons.bytesFor(candidate.id),
        onTap: () => _controller.selectAndExecute(candidate.id),
      );
}

String _contextLabel(ContextDto context) {
  final source = switch (context.source) {
    ContextSourceDto.finderSelection => 'Finder selection',
    ContextSourceDto.selectedText => 'Selected text',
    ContextSourceDto.browserCurrentPage => 'Safari current page',
    ContextSourceDto.clipboard => 'Clipboard',
    ContextSourceDto.userInput ||
    ContextSourceDto.foregroundApplication ||
    ContextSourceDto.applicationContext =>
      null,
  };
  return source == null ? context.label : '$source · ${context.label}';
}
