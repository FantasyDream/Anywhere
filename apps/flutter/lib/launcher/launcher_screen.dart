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
        theme: LauncherStyle.theme,
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
  String? _displayedScope;
  String? _displayedPath;
  String? _submitted;

  void _onInput() {
    final value = _input.value;
    if (!value.composing.isCollapsed) return;
    if (value.text == _submitted) return;
    _submitted = value.text;
    _controller.update(value.text);
  }

  void _reportIconViewport() {
    final count = _view.visibleEntries.length;
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
    final searchText = _view.visible ? _view.searchText : '';
    if (_input.text != searchText) {
      _submitted = searchText;
      _input.value = TextEditingValue(
        text: searchText,
        selection: TextSelection.collapsed(offset: searchText.length),
      );
    }
    if (_view.visible) {
      if (!_wasVisible) _focus.requestFocus();
      if ((_view.snapshot?.revision != _displayedRevision ||
              _view.scopeId != _displayedScope ||
              _view.actionPath?.join(' / ') != _displayedPath) &&
          _scroll.hasClients) {
        _scroll.jumpTo(0);
      }
      _ensureSelectedVisible();
    } else {
      _focus.unfocus();
    }
    _wasVisible = _view.visible;
    _displayedRevision = _view.snapshot?.revision;
    _displayedScope = _view.scopeId;
    _displayedPath = _view.actionPath?.join(' / ');
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
      final top = _view.selectedTop;
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

  Future<void> _openEntry(int index) async {
    await _controller.selectEntry(index);
    if (mounted && _view.visible) _focus.requestFocus();
  }

  void _backMenu() {
    _controller.backMenu();
    _focus.requestFocus();
  }

  KeyEventResult _onKey(FocusNode node, KeyEvent event) {
    if (event is! KeyDownEvent && event is! KeyRepeatEvent) {
      return KeyEventResult.ignored;
    }
    // Leave candidate selection to the input method until composition has finished.
    if (!_input.value.composing.isCollapsed) return KeyEventResult.ignored;
    final control = HardwareKeyboard.instance.isControlPressed;
    final command = HardwareKeyboard.instance.isMetaPressed;
    if (event.logicalKey == LogicalKeyboardKey.escape) {
      _controller.escape();
    } else if (command &&
        event.logicalKey == LogicalKeyboardKey.keyK &&
        _view.canBrowseMenus &&
        _view.input.isEmpty) {
      _controller.browseMenus();
    } else if (event.logicalKey == LogicalKeyboardKey.arrowRight &&
        _view.input.isEmpty &&
        _view.visibleEntries.isNotEmpty &&
        _view.visibleEntries[_view.selectedIndex].navigationPath != null) {
      _controller.executeSelected();
    } else if (event.logicalKey == LogicalKeyboardKey.arrowLeft &&
        _view.input.isEmpty &&
        _view.browsing) {
      _controller.backMenu();
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
    final entries = _view.visibleEntries;
    final hasPanel = _view.hasPanel;
    final labels = _view.selectedScope?.label ?? 'Universal';
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
                          child: !_view.browsing
                              ? const Icon(
                                  Icons.search_rounded,
                                  size: 19,
                                  color: LauncherStyle.primary,
                                )
                              : IconButton(
                                  tooltip: 'Back to results',
                                  constraints: const BoxConstraints.tightFor(
                                    width: 34,
                                    height: 34,
                                  ),
                                  padding: EdgeInsets.zero,
                                  iconSize: 19,
                                  color: LauncherStyle.primary,
                                  icon: const Icon(Icons.arrow_back_rounded),
                                  onPressed: _backMenu,
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
                if (_view.hasScopeBar) _scopeBar(),
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
                              if (_view.noticeSettingsPane != null)
                                TextButton(
                                  onPressed: () =>
                                      _controller.openPrivacySettings(
                                          _view.noticeSettingsPane!),
                                  child: const Text('Open Settings',
                                      style: TextStyle(fontSize: 11)),
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
                        : entries.isEmpty
                            ? Center(
                                child: Text(
                                  _view.scopeId == null
                                      ? 'No results'
                                      : _view.emptyMessage,
                                  style: const TextStyle(
                                    fontSize: 12,
                                    color: LauncherStyle.secondary,
                                  ),
                                ),
                              )
                            : ListView.builder(
                                key: const Key('action-list'),
                                controller: _scroll,
                                padding: const EdgeInsets.fromLTRB(6, 0, 6, 8),
                                itemCount: entries.length,
                                itemBuilder: (context, index) => SizedBox(
                                  height: LauncherViewState.rowHeight,
                                  child: _entryRow(entries[index], index),
                                ),
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
                                ? '${entries.length} results'
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
                            if (_view.browsing)
                              TextButton(
                                  onPressed: _backMenu,
                                  child: const Text('Back')),
                            const SizedBox(width: 16),
                          ],
                          if (_view.output == null)
                            TextButton(
                              onPressed: entries.isEmpty ||
                                      _view.executing ||
                                      _view.resolving
                                  ? null
                                  : _controller.executeSelected,
                              style: TextButton.styleFrom(
                                  foregroundColor: LauncherStyle.secondary,
                                  minimumSize: const Size(42, 28),
                                  padding: const EdgeInsets.symmetric(
                                      horizontal: 8)),
                              child: Text(
                                  entries.isNotEmpty &&
                                          entries[_view.selectedIndex]
                                                  .navigationPath !=
                                              null
                                      ? 'Open'
                                      : 'Run',
                                  style: const TextStyle(
                                      fontSize: 11,
                                      fontWeight: FontWeight.w500,
                                      color: LauncherStyle.secondary)),
                            )
                          else
                            TextButton(
                              onPressed: () {
                                _controller.back();
                                _focus.requestFocus();
                              },
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

  Widget _scopeBar() => SizedBox(
        height: LauncherViewState.scopeHeight,
        child: ListView(
          key: const Key('scope-bar'),
          scrollDirection: Axis.horizontal,
          padding: const EdgeInsets.symmetric(horizontal: 14),
          children: [
            for (final scope in _view.scopes)
              _scopeChip(scope.label, scope.id == _view.scopeId, () {
                _controller.selectScope(scope.id);
                _focus.requestFocus();
              })
          ],
        ),
      );

  Widget _scopeChip(String label, bool selected, VoidCallback onTap) => Padding(
        padding: const EdgeInsets.symmetric(horizontal: 3, vertical: 4),
        child: Semantics(
            selected: selected,
            child: Material(
              color:
                  selected ? const Color(0x4b7e98f2) : const Color(0x18ffffff),
              clipBehavior: Clip.antiAlias,
              shape: StadiumBorder(
                  side: BorderSide(
                      color: selected
                          ? const Color(0x99b5c5ff)
                          : const Color(0x24ffffff))),
              child: InkWell(
                  customBorder: const StadiumBorder(),
                  onTap: onTap,
                  child: Padding(
                      padding: const EdgeInsets.symmetric(
                          horizontal: 11, vertical: 4),
                      child: Text(label,
                          style: TextStyle(
                              fontSize: 11,
                              fontWeight:
                                  selected ? FontWeight.w600 : FontWeight.w500,
                              color: selected
                                  ? LauncherStyle.primary
                                  : LauncherStyle.secondary)))),
            )),
      );

  Widget _entryRow(LauncherEntry entry, int index) {
    final candidate = entry.candidate;
    if (candidate != null) {
      return CandidateRow(
        candidate: candidate,
        selected: index == _view.selectedIndex,
        iconBytes: _controller.icons.bytesFor(candidate.id),
        sourceLabel: _view.sourceLabelFor(candidate),
        onTap: () => _openEntry(index),
      );
    }
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 2, vertical: 1),
      child: Material(
        color: index == _view.selectedIndex
            ? const Color(0x44778fe0)
            : Colors.transparent,
        borderRadius: BorderRadius.circular(11),
        child: InkWell(
          borderRadius: BorderRadius.circular(11),
          onTap: () => _openEntry(index),
          child: Padding(
            padding: const EdgeInsets.symmetric(horizontal: 19),
            child: Row(children: [
              const Icon(Icons.folder_open_rounded,
                  size: 23, color: LauncherStyle.secondary),
              const SizedBox(width: 12),
              Expanded(
                  child: Text(entry.title,
                      style: const TextStyle(
                          fontSize: 13.5,
                          fontWeight: FontWeight.w600,
                          color: LauncherStyle.primary))),
              const Icon(Icons.chevron_right_rounded,
                  size: 19, color: LauncherStyle.muted),
            ]),
          ),
        ),
      ),
    );
  }
}
