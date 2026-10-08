import '../infrastructure/desktop_environment.dart';
import '../ui/app_icons.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import '../src/rust/api.dart';
import '../infrastructure/window_host.dart';
import 'launcher_controller.dart';
import '../ui/app_style.dart';
import 'launcher_view_state.dart';
import 'widgets/candidate_row.dart';
import 'widgets/candidate_icon.dart';
import 'widgets/launcher_components.dart';
import '../settings/settings_screen.dart';
import '../settings/settings_components.dart';

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
  final MethodChannel window;

  /// Awaits late context. Absent in tests that only exercise the first snapshot.
  final WaitForSnapshot? waitForSnapshot;

  /// Loads application icons lazily. Absent in tests that do not need icons.
  final LoadIcon? loadIcon;
  final PromptAccessibility? promptAccessibility;
  final String startupMessage;
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
        shortcutActions: widget.shortcutActions,
        supportedShortcutActions: widget.supportedShortcutActions,
        shortcutApplications: widget.shortcutApplications,
        resolveIntent: widget.resolveIntent,
        waitIntent: widget.waitIntent,
        resolveBinding: widget.resolveBinding,
        waitBinding: widget.waitBinding,
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
        theme: AppStyle.theme,
        builder: AppStyle.windowBuilder,
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
    if (_view.visible && _view.output == null) {
      for (final binding in _controller.hostSettings.actionShortcuts) {
        if (binding.matches(event)) {
          if (event is KeyDownEvent) _controller.executeShortcut(binding);
          return KeyEventResult.handled;
        }
      }
    }
    final control = HardwareKeyboard.instance.isControlPressed;
    final command = DesktopEnvironment.current.commandPressed;
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
            color: SettingsStyle.window,
            shape: AppStyle.panelShape,
            clipBehavior: Clip.antiAlias,
            child: Theme(
                data: SettingsStyle.theme(Theme.of(context)),
                child: SettingsScreen(
                    controller: _controller.settings,
                    onClose: _controller.closeSettings)),
          ),
        ),
      );
    }
    final view = _view;
    final entries = view.visibleEntries;
    final selected =
        entries.isEmpty ? null : entries[view.selectedIndex].candidate;
    Widget? selectedIcon;
    if (selected?.kind == CandidateKindDto.application) {
      selectedIcon = ListenableBuilder(
          listenable: _controller.icons,
          builder: (_, __) => CandidateIcon(
              candidate: selected!,
              bytes: _controller.icons.bytesFor(selected.id),
              size: 32));
    }
    return Align(
        alignment: Alignment.topCenter,
        child: SizedBox(
          key: const Key('launcher-panel'),
          width: 648,
          height: view.height,
          child: Material(
              color: AppStyle.surface,
              shape: AppStyle.panelShape,
              clipBehavior: Clip.antiAlias,
              child: Focus(
                  onKeyEvent: _onKey,
                  child: Column(children: [
                    LauncherSearchArea(
                        state: view,
                        input: _input,
                        focus: _focus,
                        onBack: view.browsing ? _backMenu : null,
                        trailing: selectedIcon),
                    if (view.hasPanel || view.hasScopeBar)
                      const Padding(
                          padding: EdgeInsets.symmetric(horizontal: 20),
                          child: Divider(height: 1)),
                    if (view.hasScopeBar)
                      LauncherScopeBar(
                          scopes: view.scopes,
                          selectedId: view.scopeId,
                          onSelect: (id) {
                            _controller.selectScope(id);
                            _focus.requestFocus();
                          }),
                    if (view.notice != null) _notice(view),
                    if (view.hasPanel) ...[
                      if (view.hasResultHeading)
                        LauncherResultHeading(
                            title: view.resultHeading,
                            detail: view.output != null
                                ? view.resultSource
                                : view.isContent
                                    ? 'Global Search'
                                    : null),
                      Expanded(
                          child: view.output != null
                              ? SingleChildScrollView(
                                  key: const Key('result-preview'),
                                  padding: const EdgeInsets.symmetric(
                                      horizontal: 22, vertical: 12),
                                  child: SelectableText(view.output!,
                                      style: const TextStyle(
                                          fontFamily: 'monospace',
                                          fontSize: 12,
                                          height: 1.65,
                                          color: AppStyle.primary)))
                              : entries.isEmpty
                                  ? Center(
                                      child: Text(
                                          view.isContent
                                              ? 'No actions for this content'
                                              : view.scopeId == null
                                                  ? 'No results'
                                                  : view.emptyMessage,
                                          style: const TextStyle(
                                              fontSize: 13,
                                              color: AppStyle.secondary)))
                                  : ListView.builder(
                                      key: const Key('action-list'),
                                      controller: _scroll,
                                      padding: const EdgeInsets.fromLTRB(
                                          10, 0, 10, 8),
                                      itemCount: entries.length,
                                      itemBuilder: (_, index) => SizedBox(
                                          height: LauncherViewState.rowHeight,
                                          child: _entryRow(
                                              entries[index], index)))),
                      LauncherFooter(
                          label: view.output == null
                              ? '${entries.length} results'
                              : view.resultStatus == ActionStatusDto.failed
                                  ? 'Failed'
                                  : 'Succeeded',
                          primaryLabel: entries.isNotEmpty &&
                                  entries[view.selectedIndex].navigationPath !=
                                      null
                              ? 'Open'
                              : 'Run',
                          onPrimary: entries.isEmpty ||
                                  view.executing ||
                                  view.resolving
                              ? null
                              : _controller.executeSelected,
                          result: view.output != null,
                          onCopy: view.output == null
                              ? null
                              : _controller.copyResult,
                          onBack: view.output != null
                              ? () {
                                  _controller.back();
                                  _focus.requestFocus();
                                }
                              : view.browsing
                                  ? _backMenu
                                  : null),
                    ],
                  ]))),
        ));
  }

  Widget _notice(LauncherViewState view) => SizedBox(
      height: 44,
      child: Padding(
          padding: const EdgeInsets.symmetric(horizontal: 20, vertical: 4),
          child: Tooltip(
              message: view.notice!,
              child: Container(
                  padding: const EdgeInsets.symmetric(horizontal: 10),
                  decoration: BoxDecoration(
                      color: AppStyle.warning.withValues(alpha: .09),
                      borderRadius: BorderRadius.circular(10)),
                  child: Row(children: [
                    const Icon(AppIcons.info,
                        size: 15, color: AppStyle.warning),
                    const SizedBox(width: 8),
                    Expanded(
                        child: Text(view.notice!,
                            maxLines: 2,
                            overflow: TextOverflow.ellipsis,
                            style: const TextStyle(
                                fontSize: 11, color: AppStyle.warning))),
                    if (view.noticeSettingsPane != null)
                      TextButton(
                          onPressed: () => _controller
                              .openPrivacySettings(view.noticeSettingsPane!),
                          child: const Text('Open Settings',
                              style: TextStyle(fontSize: 11))),
                  ])))));

  Widget _entryRow(LauncherEntry entry, int index) {
    final candidate = entry.candidate;
    if (candidate != null) {
      final selected = index == _view.selectedIndex;
      final sourceLabel = _view.sourceLabelFor(candidate);
      final shortcut = _controller.shortcutLabelFor(candidate);
      Widget row() => CandidateRow(
          actionShortcut: shortcut,
          candidate: candidate,
          selected: selected,
          iconBytes: _controller.icons.bytesFor(candidate.id),
          sourceLabel: sourceLabel,
          onTap: () => _openEntry(index));
      return candidate.kind == CandidateKindDto.application
          ? ListenableBuilder(
              listenable: _controller.icons, builder: (_, __) => row())
          : row();
    }
    return MenuNavigationRow(
        title: entry.title,
        selected: index == _view.selectedIndex,
        onTap: () => _openEntry(index));
  }
}
