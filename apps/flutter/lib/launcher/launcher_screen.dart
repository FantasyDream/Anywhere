import 'activation_prefill.dart';
import 'dart:async';
import '../infrastructure/desktop_environment.dart';
import 'package:flutter/rendering.dart' show ScrollDirection;
import '../ui/app_icons.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter/gestures.dart' show kPrimaryMouseButton;
import '../src/rust/api.dart';
import '../infrastructure/window_host.dart';
import 'launcher_controller.dart';
import 'widgets/file_search_components.dart';
import 'file_search_controller.dart' show FileFilter, LoadFilePreview;
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
    this.waitForSnapshotDelta,
    this.loadIcon,
    this.promptAccessibility,
    this.startupMessage = '',
    this.fileSearchInitializationFailed = false,
    this.shortcutActions,
    this.supportedShortcutActions,
    this.shortcutApplications,
    this.resolveIntent,
    this.waitIntent,
    this.resolveBinding,
    this.waitBinding,
    this.loadFilePreview,
    this.updateFiles,
    this.fileMore,
    this.fileMoreDelta,
    this.freezeFiles,
    this.objectActions,
    this.suspend,
    this.resume,
    this.chooseActivationInput,
  });
  final Future<LauncherStateDto?> Function(int, String, int, String)?
      updateFiles;
  final Future<LauncherStateDto?> Function(int, int)? fileMore;
  final Future<SnapshotWaitDto> Function(int, int, int, int)? fileMoreDelta;
  final Future<void> Function(int, int)? freezeFiles;
  final Future<List<CandidateDto>> Function(int, int, String)? objectActions;
  final LoadFilePreview? loadFilePreview;
  final ChooseActivationInput? chooseActivationInput;
  final BeginActivation beginActivation;
  final Activate activate;
  final UpdateInput updateInput;
  final Execute execute;
  final Future<void> Function(int) deactivate;
  final Future<void> Function(int)? suspend;
  final Future<LauncherStateDto?> Function(int)? resume;
  final MethodChannel window;

  /// Awaits late context. Absent in tests that only exercise the first snapshot.
  final WaitForSnapshot? waitForSnapshot;
  final Future<SnapshotWaitDto> Function(int, int, int, int)?
      waitForSnapshotDelta;

  /// Loads application icons lazily. Absent in tests that do not need icons.
  final LoadIcon? loadIcon;
  final PromptAccessibility? promptAccessibility;
  final String startupMessage;
  final bool fileSearchInitializationFailed;
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
        suspend: widget.suspend,
        resume: widget.resume,
        chooseActivationInput: widget.chooseActivationInput,
        waitForSnapshot: widget.waitForSnapshot,
        waitForSnapshotDelta: widget.waitForSnapshotDelta,
        loadIcon: widget.loadIcon,
        promptAccessibility: widget.promptAccessibility,
        shortcutActions: widget.shortcutActions,
        supportedShortcutActions: widget.supportedShortcutActions,
        shortcutApplications: widget.shortcutApplications,
        resolveIntent: widget.resolveIntent,
        waitIntent: widget.waitIntent,
        resolveBinding: widget.resolveBinding,
        waitBinding: widget.waitBinding,
        loadFilePreview: widget.loadFilePreview,
        updateFiles: widget.updateFiles,
        fileMore: widget.fileMore,
        fileMoreDelta: widget.fileMoreDelta,
        freezeFiles: widget.freezeFiles,
        objectActions: widget.objectActions,
      ),
      window: WindowHost(widget.window),
      startupMessage: widget.startupMessage,
      fileSearchInitializationFailed: widget.fileSearchInitializationFailed,
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
  double? _hiddenScrollOffset;
  LauncherController get _controller => widget.controller;
  LauncherViewState get _view => _controller.view;
  bool _wasVisible = false;
  int? _displayedRevision;
  String? _displayedScope;
  String? _displayedPath;
  String? _submitted;
  InputModeDto? _displayedMode;
  int? _displayedPresentation;
  int _focusRequest = 0;
  bool _fileScrollRequested = false;
  bool _pointerSelection = false;
  Object? _moreRequestedFor;
  String? _displayedFileSelection;
  int? _displayedFileIndex;
  bool _displayedObjectActions = false;
  (Object, double)? _objectResultsPosition;

  Object get _fileQuery => (
        _view.snapshot?.sessionId,
        _view.snapshot?.revision,
        _view.inputMode,
        _view.input,
        _view.fileFilter,
      );

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
    if (!_view.visible &&
        _wasVisible &&
        _controller.hasRetainedState &&
        _scroll.hasClients) {
      _hiddenScrollOffset = _scroll.offset;
    }
    final restoreHiddenScroll =
        _view.visible && !_wasVisible ? _hiddenScrollOffset : null;
    if (_view.visible || !_controller.hasRetainedState) {
      _hiddenScrollOffset = null;
    }
    if (restoreHiddenScroll != null) {
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (mounted && _view.visible && _scroll.hasClients) {
          _scroll.jumpTo(
              restoreHiddenScroll.clamp(0, _scroll.position.maxScrollExtent));
        }
      });
    }
    final objectActions = _view.objectActionTitle != null;
    if (objectActions && !_displayedObjectActions && _scroll.hasClients) {
      // Actions temporarily replace the list with a much shorter one. Save its
      // position before layout clamps the shared scroll controller to zero.
      _objectResultsPosition = (_fileQuery, _scroll.offset);
    }
    final savedPosition = _objectResultsPosition;
    final restoreFilePosition = _displayedObjectActions &&
        !objectActions &&
        _view.bindingChoices == null &&
        _view.output == null &&
        savedPosition?.$1 == _fileQuery;
    if (restoreFilePosition) {
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (!mounted ||
            !_view.visible ||
            _view.bindingChoices != null ||
            _view.output != null ||
            _fileQuery != savedPosition!.$1 ||
            !_scroll.hasClients) {
          return;
        }
        _scroll.jumpTo(savedPosition.$2.clamp(_scroll.position.minScrollExtent,
            _scroll.position.maxScrollExtent));
      });
    }
    if (savedPosition?.$1 != _fileQuery) {
      _objectResultsPosition = null;
    }
    _displayedObjectActions = objectActions;
    final inputVisible = _view.visible && _view.phase != LauncherPhase.settings;
    final searchText =
        _view.visible || _controller.hasRetainedState ? _view.searchText : '';
    if (_input.text != searchText) {
      _submitted = searchText;
      _input.value = TextEditingValue(
        text: searchText,
        selection: TextSelection.collapsed(offset: searchText.length),
      );
    }
    if (inputVisible) {
      if (!_wasVisible ||
          _displayedMode != _view.inputMode ||
          _displayedPresentation != _controller.windowPresentation) {
        final request = ++_focusRequest;
        final mode = _view.inputMode;
        // Host show can finish after the field already has logical focus.
        // Request the editor connection too, without cycling focus or IME state.
        WidgetsBinding.instance.addPostFrameCallback((_) {
          if (mounted &&
              request == _focusRequest &&
              _view.visible &&
              _view.phase != LauncherPhase.settings &&
              _view.inputMode == mode) {
            final editor =
                _focus.context?.findAncestorStateOfType<EditableTextState>();
            if (editor != null) {
              editor.requestKeyboard();
            } else {
              _focus.requestFocus();
            }
          }
        });
      }
      if ((_view.snapshot?.revision != _displayedRevision ||
              _view.scopeId != _displayedScope ||
              _view.actionPath?.join(' / ') != _displayedPath) &&
          _scroll.hasClients) {
        _fileScrollRequested = false;
        _moreRequestedFor = null;
        _scroll.jumpTo(0);
      }
      final entries = _view.visibleEntries;
      final selection =
          entries.isEmpty ? null : entries[_view.selectedIndex].key;
      // Progress-only batches must not scroll a mouse user back to the selected row.
      if (!restoreFilePosition &&
          !_pointerSelection &&
          (!_view.isFileSearch ||
              selection != _displayedFileSelection ||
              _view.selectedIndex != _displayedFileIndex)) {
        _ensureSelectedVisible();
      }
      _displayedFileSelection = selection;
      _displayedFileIndex = _view.selectedIndex;
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (mounted) _loadMoreNearEnd();
      });
    } else {
      ++_focusRequest;
      _focus.unfocus();
    }
    _wasVisible = inputVisible;
    _displayedMode = _view.inputMode;
    _displayedPresentation = _controller.windowPresentation;
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

  Future<void> _runAndFocus(Future<void> Function() action) async {
    await action();
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
    if (_view.navigationOpen) {
      if (event.logicalKey == LogicalKeyboardKey.escape) {
        unawaited(_controller.escape());
      } else if (event.logicalKey == LogicalKeyboardKey.arrowDown ||
          event.logicalKey == LogicalKeyboardKey.arrowUp) {
        _controller.moveNavigation(
            event.logicalKey == LogicalKeyboardKey.arrowDown ? 1 : -1);
      } else if (event.logicalKey == LogicalKeyboardKey.enter) {
        unawaited(_runAndFocus(_controller.chooseNavigation));
      } else {
        return KeyEventResult.ignored;
      }
      _focus.requestFocus();
      return KeyEventResult.handled;
    }
    if (_view.visible && _view.output == null) {
      for (final binding in _controller.hostSettings.actionShortcuts) {
        if (binding.matches(event)) {
          if (event is KeyDownEvent) _controller.executeShortcut(binding);
          return KeyEventResult.handled;
        }
      }
    }
    if (event.logicalKey == LogicalKeyboardKey.tab &&
        !HardwareKeyboard.instance.isControlPressed &&
        !HardwareKeyboard.instance.isAltPressed &&
        !HardwareKeyboard.instance.isMetaPressed) {
      return KeyEventResult.handled;
    }
    // Controls keep their standard activation keys. Otherwise Enter
    // on a focused toolbar button would execute the selected result instead.
    if (!_focus.hasFocus) {
      if (event.logicalKey == LogicalKeyboardKey.escape) {
        unawaited(_controller.escape());
        _focus.requestFocus();
        return KeyEventResult.handled;
      }
      return KeyEventResult.ignored;
    }
    final control = HardwareKeyboard.instance.isControlPressed;
    final command = DesktopEnvironment.current.commandPressed;
    if (event.logicalKey == LogicalKeyboardKey.escape) {
      _controller.escape();
    } else if (command &&
        event.logicalKey == LogicalKeyboardKey.keyK &&
        _controller.canShowObjectActions) {
      _controller.showObjectActions();
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
      final direction = HardwareKeyboard.instance.isShiftPressed ? -1 : 1;
      _controller.cycleScope(direction);
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
          width: view.width,
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
                        onToggleNavigation: () =>
                            _runAndFocus(_controller.toggleNavigation),
                        onBack: view.isFileSearch
                            ? _backFiles
                            : view.browsing
                                ? _backMenu
                                : null,
                        trailing: selectedIcon),
                    if (view.navigationOpen ||
                        view.hasPanel ||
                        view.hasControls)
                      const Padding(
                          padding: EdgeInsets.symmetric(horizontal: 20),
                          child: Divider(height: 1)),
                    if (view.navigationOpen)
                      LauncherRootNavigation(
                          state: view,
                          onSelect: (mode) => _runAndFocus(() => _controller
                              .navigateRoot(mode == InputModeDto.search
                                  ? WakeMode.universal
                                  : WakeMode.currentApp))),
                    if (!view.navigationOpen && view.hasScopeBar)
                      LauncherScopeBar(
                          scopes: view.scopes,
                          selectedId: view.scopeId,
                          onSelect: (id) {
                            _controller.selectScope(id);
                            _focus.requestFocus();
                          }),
                    if (!view.navigationOpen && view.hasFileControls)
                      FileSearchControls(
                          files: _controller.files,
                          enabled: !view.resolving &&
                              !view.executing &&
                              view.objectActionTitle == null &&
                              view.output == null,
                          onSelect: _focus.requestFocus),
                    if (!view.navigationOpen && view.notice != null)
                      _notice(view),
                    if (!view.navigationOpen && view.hasPanel) ...[
                      if (view.hasResultHeading)
                        LauncherResultHeading(
                            title: view.resultHeading,
                            detail: view.output != null
                                ? view.resultSource
                                : view.isContent
                                    ? 'Global Search'
                                    : null),
                      Expanded(
                          child: LayoutBuilder(
                              builder: (context, constraints) => Row(
                                      crossAxisAlignment:
                                          CrossAxisAlignment.stretch,
                                      children: [
                                        Expanded(child: _resultsBody(view)),
                                        if (view.hasFilePreview) ...[
                                          const VerticalDivider(width: 1),
                                          SizedBox(
                                              width:
                                                  (constraints.maxWidth * .38)
                                                      .clamp(220.0, 360.0),
                                              child: FileSearchPreview(
                                                  files: _controller.files)),
                                        ],
                                      ]))),
                      LauncherFooter(
                          onRetry:
                              (view.searchUnavailable || view.searchPartial) &&
                                      !_fileSearchNeedsAuthorization &&
                                      !view.executing &&
                                      !view.resolving &&
                                      view.output == null &&
                                      view.objectActionTitle == null
                                  ? () => _runAndFocus(_controller.retrySearch)
                                  : null,
                          onActions: _controller.canShowObjectActions
                              ? () =>
                                  _runAndFocus(_controller.showObjectActions)
                              : null,
                          label: view.output == null
                              ? (view.hasFileControls && _controller.fileMoreLoading
                                      ? 'Loading more files…'
                                      : view.searchStatus) ??
                                  (entries.length == 1
                                      ? '1 result'
                                      : '${entries.length} results')
                              : view.resultStatus == ActionStatusDto.failed
                                  ? 'Failed'
                                  : 'Succeeded',
                          primaryLabel: view.objectActionTitle == null &&
                                  entries.isNotEmpty &&
                                  (entries[view.selectedIndex].navigationPath !=
                                          null ||
                                      entries[view.selectedIndex].candidate?.kind ==
                                          CandidateKindDto.file)
                              ? 'Open'
                              : selected?.commandId == 'core.files.search'
                                  ? 'Search'
                                  : selected?.kind ==
                                          CandidateKindDto.application
                                      ? 'Open'
                                      : 'Run',
                          onPrimary: entries.isEmpty ||
                                  view.executing ||
                                  view.resolving
                              ? null
                              : () => _runAndFocus(_controller.executeSelected),
                          result: view.output != null,
                          onCopy: view.output == null
                              ? null
                              : _controller.copyResult,
                          onBack: view.output != null
                              ? () {
                                  _controller.back();
                                  _focus.requestFocus();
                                }
                              : view.objectActionTitle != null
                                  ? _backFiles
                                  : view.browsing
                                      ? _backMenu
                                      : null),
                    ],
                  ]))),
        ));
  }

  Widget _resultsBody(LauncherViewState view) {
    if (view.output != null) {
      return SingleChildScrollView(
          key: const Key('result-preview'),
          padding: const EdgeInsets.symmetric(horizontal: 22, vertical: 12),
          child: SelectableText(view.output!,
              style: const TextStyle(
                  fontFamily: 'monospace',
                  fontSize: 12,
                  height: 1.65,
                  color: AppStyle.primary)));
    }
    final entries = view.visibleEntries;
    if (entries.isEmpty) {
      if (view.isFileSearch) {
        if (_fileSearchNeedsAuthorization) {
          return const LauncherEmptyState(
              icon: AppIcons.folder,
              title: 'Authorize search directories',
              detail:
                  'Open Settings → Search and authorize saved directories. No file search runs before access is prepared.');
        }
        final disabled = !_controller.hostSettings.fileSearchEnabled ||
            _controller.settings.fileSearchNeedsRecovery;
        final idle = view.input.trim().isEmpty;
        return LauncherEmptyState(
            icon: disabled
                ? AppIcons.settings
                : view.searchUnavailable
                    ? AppIcons.error
                    : AppIcons.search,
            title: disabled
                ? 'File search is turned off'
                : idle
                    ? 'Find files and folders'
                    : view.searchLoading
                        ? 'Searching…'
                        : view.searchUnavailable
                            ? 'Search is unavailable'
                            : view.searchPartial
                                ? 'Partial results'
                                : 'No matching files',
            detail: _fileEmptyMessage(view));
      }
      final message = view.isContent
          ? 'No actions for this content'
          : view.scopeId != null
              ? view.emptyMessage
              : view.resolving || view.searchLoading
                  ? 'Searching files…'
                  : view.searchUnavailable
                      ? 'File search unavailable'
                      : 'No results';
      return Center(
          child: Padding(
              padding: const EdgeInsets.symmetric(horizontal: 20),
              child: Text(message,
                  textAlign: TextAlign.center,
                  style: const TextStyle(
                      fontSize: 13, color: AppStyle.secondary))));
    }
    // Results use arrow-key selection from the search field, not Tab traversal.
    return ExcludeFocus(
        child: NotificationListener<ScrollNotification>(
            onNotification: (notification) {
              if (!view.hasFileControls) return false;
              if (notification is UserScrollNotification &&
                  notification.direction == ScrollDirection.reverse) {
                _fileScrollRequested = true;
                _controller.freezeFileResults();
              }
              if (notification is ScrollUpdateNotification ||
                  notification is ScrollEndNotification) {
                _loadMoreNearEnd(userGesture: true);
              }
              return false;
            },
            child: ListView.builder(
                key: const Key('action-list'),
                controller: _scroll,
                padding: const EdgeInsets.fromLTRB(10, 0, 10, 8),
                itemCount: entries.length,
                itemBuilder: (_, index) => SizedBox(
                    height: LauncherViewState.rowHeight,
                    child: _entryRow(entries[index], index)))));
  }

  void _loadMoreNearEnd({bool userGesture = false}) {
    if (!_fileScrollRequested ||
        !_scroll.hasClients ||
        !_view.hasFileControls ||
        !_controller.canLoadMoreFiles ||
        _scroll.position.extentAfter > LauncherViewState.rowHeight * 3) {
      return;
    }
    final token = (_view.snapshot?.revision, _view.snapshot?.snapshotSeq);
    if (!userGesture && _moreRequestedFor == token) return;
    _moreRequestedFor = token;
    unawaited(_controller.loadMoreFiles());
  }

  void _backFiles() {
    if (_view.bindingChoices != null || _view.output != null) {
      _controller.escape();
    } else {
      _controller.closeFileSearch();
    }
    _focus.requestFocus();
  }

  String _fileEmptyMessage(LauncherViewState view) {
    if (!_controller.hostSettings.fileSearchEnabled ||
        _controller.settings.fileSearchNeedsRecovery) {
      return 'File search is disabled. Enable it in Settings → Search.';
    }
    if (view.input.trim().isEmpty) {
      return view.snapshot?.fileSearch == null
          ? 'Type a file or folder name to search'
          : 'Searching files…';
    }
    if (view.searchLoading) return 'Searching files…';
    if (view.searchPartial) {
      return 'File search is incomplete. Check the issue details and retry.';
    }
    if (view.searchUnavailable) return 'File search unavailable';
    if (_controller.files.filter != FileFilter.all) {
      return 'No ${_controller.files.filter.label.toLowerCase()} in these results. Try All types or another search.';
    }
    return 'No matching files';
  }

  bool get _fileSearchNeedsAuthorization =>
      DesktopEnvironment.current.fileSearchAccess &&
      _controller.hostSettings.fileSearchEnabled &&
      _controller.hostSettings.fileSearchAccessReady == false;

  Widget _notice(LauncherViewState view) => SizedBox(
      height: 44,
      child: Padding(
          padding: const EdgeInsets.symmetric(horizontal: 20, vertical: 4),
          child: Tooltip(
              message: view.isFileSearch &&
                      view.searchPartial &&
                      (view.snapshot?.fileSearch?.issueExamples.isNotEmpty ??
                          false)
                  ? view.snapshot!.fileSearch!.issueExamples.join('\n')
                  : view.notice!,
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

  void _selectPointerResult(int index) {
    // Pointer-down selects before double-click recognition, but must not queue
    // a scroll-to-selection that would undo the following drag or wheel scroll.
    _pointerSelection = true;
    try {
      _controller.selectResult(index);
    } finally {
      _pointerSelection = false;
    }
    _focus.requestFocus();
  }

  Widget _entryRow(LauncherEntry entry, int index) {
    final candidate = entry.candidate;
    if (candidate != null) {
      final selected = index == _view.selectedIndex;
      final sourceLabel = _view.sourceLabelFor(candidate);
      final shortcut = _controller.shortcutLabelFor(candidate);
      Widget row() => CandidateRow(
          fileObject: _view.objectActionTitle == null,
          actionShortcut: shortcut,
          candidate: candidate,
          selected: selected,
          iconBytes: _controller.icons.bytesFor(candidate.id),
          sourceLabel: sourceLabel,
          onTap: () {
            _selectPointerResult(index);
          });
      final child = candidate.kind == CandidateKindDto.application
          ? ListenableBuilder(
              listenable: _controller.icons, builder: (_, __) => row())
          : row();
      return GestureDetector(
          onDoubleTap: !_view.resolving &&
                  !_view.executing &&
                  candidate.availability == AvailabilityDto.enabled
              ? () => _controller.selectAndExecute(candidate.id)
              : null,
          // Select on pointer down so double-click recognition never delays
          // selection or lets an immediate Return activate the previous row.
          child: Listener(
              onPointerDown: (event) {
                if (event.buttons == kPrimaryMouseButton) {
                  _selectPointerResult(index);
                }
              },
              child: child));
    }
    return MenuNavigationRow(
        title: entry.title,
        selected: index == _view.selectedIndex,
        onTap: () => _openEntry(index));
  }
}
