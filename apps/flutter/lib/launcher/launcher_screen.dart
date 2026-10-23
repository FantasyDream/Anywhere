import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart' show ScrollDirection;
import 'package:flutter/services.dart';

import '../infrastructure/desktop_environment.dart';
import '../infrastructure/desktop_image_cache.dart';
import '../infrastructure/window_host.dart';
import '../l10n/app_language.dart';
import '../settings/settings_components.dart';
import '../settings/settings_screen.dart';
import '../src/rust/api.dart';
import '../ui/app_components.dart';
import '../ui/app_icons.dart';
import '../ui/app_style.dart';
import 'activation_prefill.dart';
import 'file_search_controller.dart' show FileFilter, LoadFilePreview;
import 'launcher_controller.dart';
import 'launcher_view_state.dart';
import 'widgets/candidate_row.dart';
import 'widgets/file_search_components.dart';
import 'widgets/launcher_components.dart';

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
  Widget build(BuildContext context) => LocalizedApp(
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
  Object? _displayedTool;
  (Object, double)? _toolReturnScroll;
  LauncherController get _controller => widget.controller;
  LauncherViewState get _view => _controller.view;
  bool _wasVisible = false;
  bool _hadVisiblePresentation = false;
  int _searchEditorEpoch = 0;
  int? _displayedRevision;
  String? _displayedScope;
  String? _displayedPath;
  // A first focus notification is not an edit of the initial empty controller.
  String _submitted = '';
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
    if (count == 0 || _view.toolPage != null || _view.output != null) return;
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
    _hadVisiblePresentation = _view.visible;
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
      _hadVisiblePresentation = _view.visible;
    }
  }

  bool _releaseExpiredPresentation() {
    if (_view.visible) {
      _hadVisiblePresentation = true;
      return false;
    }
    if (!_hadVisiblePresentation ||
        _view.phase != LauncherPhase.hidden ||
        _controller.hasRetainedState) {
      return false;
    }
    _hadVisiblePresentation = false;
    // Clearing TextEditingController.text alone leaves EditableText's undo stack
    // alive. Recreate only the expired editor, after its 15-second workspace ends.
    ++_searchEditorEpoch;
    final owner = _controller;
    // Let Image widgets release their live listeners before clearing keep-alive
    // entries. A wake or controller replacement before this frame cancels cleanup.
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (mounted &&
          identical(_controller, owner) &&
          _view.phase == LauncherPhase.hidden &&
          !_controller.hasRetainedState) {
        releaseIdleDesktopImages();
      }
    });
    return true;
  }

  void _refresh() {
    if (!mounted) return;
    final releaseIdleResources = _releaseExpiredPresentation();
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
    final queryKey = (
      _view.snapshot?.sessionId,
      _view.snapshot?.revision,
      _view.input,
      _view.scopeId,
      _view.actionPath
    );
    if (_view.toolPage != null &&
        _displayedTool == null &&
        _scroll.hasClients) {
      _toolReturnScroll = (queryKey, _scroll.offset);
    }
    final toolPosition = _toolReturnScroll;
    final restoreToolScroll = _view.toolPage == null &&
        _displayedTool != null &&
        toolPosition?.$1 == queryKey;
    _displayedTool = _view.toolPage;
    if (restoreToolScroll) {
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (mounted &&
            _view.visible &&
            _view.toolPage == null &&
            _scroll.hasClients) {
          _scroll.jumpTo(
              toolPosition!.$2.clamp(0, _scroll.position.maxScrollExtent));
        }
      });
    }
    if (_toolReturnScroll?.$1 != queryKey) _toolReturnScroll = null;
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
    final inputVisible = _view.visible &&
        _view.phase != LauncherPhase.settings &&
        _view.toolPage == null;
    final searchText =
        _view.visible || _controller.hasRetainedState ? _view.searchText : '';
    // Progress, preview and catalog updates still carry the last committed
    // query. The input method owns any composing suffix until it commits.
    // Only a deliberate new Host query (or expired workspace) may replace it.
    final composing = !_input.value.composing.isCollapsed;
    final hostQueryChanged = searchText != _submitted;
    if (_input.text != searchText &&
        (!composing || hostQueryChanged || releaseIdleResources)) {
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
              _view.toolPage == null &&
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
      if (!restoreToolScroll &&
          !restoreFilePosition &&
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
    if (releaseIdleResources && !WidgetsBinding.instance.framesEnabled) {
      // Hidden desktop engines can stop vsync. Flush this one expired subtree
      // without waiting for the next wake; do not restart a repeating frame loop.
      WidgetsBinding.instance.scheduleWarmUpFrame();
    }
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
    if (mounted && _view.visible && _view.toolPage == null) {
      _focus.requestFocus();
    }
  }

  Future<void> _runAndFocus(Future<void> Function() action) async {
    await action();
    if (mounted && _view.visible && _view.toolPage == null) {
      _focus.requestFocus();
    }
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
    if (_view.toolPage != null) return KeyEventResult.ignored;
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
        !HardwareKeyboard.instance.isAltPressed &&
        !HardwareKeyboard.instance.isMetaPressed) {
      // Tab (and the existing Ctrl+Tab alias) cycles the visible category bar,
      // without traversing controls or changing the layer behind an action list.
      if (_focus.hasFocus &&
          _view.phase == LauncherPhase.ready &&
          _view.hasControls &&
          _view.bindingChoices == null &&
          _view.objectActionTitle == null &&
          _view.output == null) {
        _controller
            .cycleScope(HardwareKeyboard.instance.isShiftPressed ? -1 : 1);
      }
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
    final owner = _controller;
    final view = _view;
    final snapshot = view.snapshot;
    if (owner.performance.pending &&
        view.phase == LauncherPhase.ready &&
        snapshot != null &&
        view.toolPage == null &&
        view.output == null) {
      owner.performance.ready(
        generation: view.generation,
        revision: snapshot.revision,
        presentation: owner.windowPresentation,
        rows: view.visibleEntries.length,
        afterFrame: (callback) =>
            WidgetsBinding.instance.addPostFrameCallback((_) => callback()),
        current: () =>
            mounted &&
            identical(_controller, owner) &&
            _view.generation == view.generation &&
            _view.phase == LauncherPhase.ready &&
            _view.snapshot?.revision == snapshot.revision,
      );
    }
    final platformPage = _view.toolPage?.embedsPlatformView == true;
    final settings = _view.phase == LauncherPhase.settings;
    // Settings reuses this engine. Keep the retained native page mounted but
    // offstage, so navigating to Settings does not destroy its 15-second state.
    // Stable sibling slots also keep the Settings draft mounted when retention expires.
    return Stack(fit: StackFit.expand, children: [
      Offstage(
          offstage: !platformPage || settings,
          child:
              platformPage ? _buildToolPanel(_view) : const SizedBox.shrink()),
      Offstage(
          offstage: platformPage && !settings,
          child: platformPage && !settings
              ? const SizedBox.shrink()
              : _buildContent(context)),
    ]);
  }

  Widget _buildToolPanel(LauncherViewState view) {
    return Align(
      alignment: Alignment.topCenter,
      child: SizedBox(
        key: const Key('launcher-panel'),
        width: view.width,
        height: view.height,
        child: Material(
          color: AppStyle.surface,
          shape: AppStyle.panelShape,
          // Foreground border paint becomes a full-size native hit-test overlay
          // on macOS, even where the stroke is transparent. Paint it behind WebKit.
          borderOnForeground: !view.toolPage!.embedsPlatformView,
          clipBehavior:
              view.toolPage!.embedsPlatformView ? Clip.none : Clip.antiAlias,
          child: KeyedSubtree(
              key: ObjectKey(view.toolPage),
              child: ListenableBuilder(
                  listenable: view.toolPage!,
                  builder: (context, _) => view.toolPage!.build(context,
                          presentation: _controller.windowPresentation,
                          visible: view.visible &&
                              view.phase != LauncherPhase.settings,
                          notice: view.message,
                          onCopy: view.output == null
                              ? null
                              : _controller.copyResult, onBack: () {
                        _controller.back();
                        _focus.requestFocus();
                      }))),
        ),
      ),
    );
  }

  Widget _buildContent(BuildContext context) {
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
    if (view.toolPage != null) return _buildToolPanel(view);
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
                        key: ValueKey((_controller, _searchEditorEpoch)),
                        state: view,
                        input: _input,
                        focus: _focus,
                        onToggleNavigation: () =>
                            _runAndFocus(_controller.toggleNavigation),
                        onBack: view.isFileSearch
                            ? _backFiles
                            : view.browsing
                                ? _backMenu
                                : null),
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
                              view.objectActionTitle == null,
                          onSelect: _focus.requestFocus),
                    if (!view.navigationOpen &&
                        view.hasFileControls &&
                        view.searchStatusFor(context.l10n) != null)
                      FileSearchStatus(
                          label: _controller.fileMoreLoading
                              ? context.l10n.loadingMoreFiles
                              : view.searchStatusFor(context.l10n)!,
                          onRetry:
                              (view.searchUnavailable || view.searchPartial) &&
                                      !_fileSearchNeedsAuthorization &&
                                      !view.executing &&
                                      !view.resolving
                                  ? () => _runAndFocus(_controller.retrySearch)
                                  : null),
                    if (!view.navigationOpen && view.notice != null)
                      _notice(view),
                    if (!view.navigationOpen && view.hasPanel) ...[
                      if (view.hasResultHeading)
                        LauncherResultHeading(
                            title: view.resultHeadingFor(context.l10n),
                            detail: view.isContent
                                ? context.l10n.globalSearch
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
                                              width: (constraints.maxWidth *
                                                      DesignTokens
                                                          .previewFraction)
                                                  .clamp(220.0, 360.0),
                                              child: FileSearchPreview(
                                                  files: _controller.files)),
                                        ],
                                      ]))),
                    ],
                  ]))),
        ));
  }

  Widget _resultsBody(LauncherViewState view) {
    final entries = view.visibleEntries;
    if (entries.isEmpty) {
      if (view.isFileSearch) {
        if (_fileSearchNeedsAuthorization) {
          return LauncherEmptyState(
              icon: AppIcons.folder,
              title: context.l10n.authorizeSearchDirectories,
              detail: context.l10n.authorizeSearchHelp);
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
                ? context.l10n.fileSearchOff
                : idle
                    ? context.l10n.findFilesFolders
                    : view.searchLoading
                        ? context.l10n.searching
                        : view.searchUnavailable
                            ? context.l10n.searchUnavailable
                            : view.searchPartial
                                ? context.l10n.partialResults
                                : context.l10n.noMatchingFiles,
            detail: _fileEmptyMessage(view));
      }
      final message = view.isContent
          ? context.l10n.noContentActions
          : view.scopeId != null
              ? (view.input.isNotEmpty
                  ? context.l10n.noMatchingActions
                  : context.l10n.noActionsAvailable)
              : view.resolving || view.searchLoading
                  ? context.l10n.searchingFiles
                  : view.searchUnavailable
                      ? context.l10n.fileSearchUnavailable
                      : context.l10n.noResults;
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
      return context.l10n.fileSearchDisabledHelp;
    }
    if (view.input.trim().isEmpty) {
      return view.snapshot?.fileSearch == null
          ? context.l10n.typeFileName
          : context.l10n.searchingFiles;
    }
    if (view.searchLoading) return context.l10n.searchingFiles;
    if (view.searchPartial) {
      return context.l10n.fileSearchIncompleteHelp;
    }
    if (view.searchUnavailable) return context.l10n.fileSearchUnavailable;
    if (_controller.files.filter != FileFilter.all) {
      return context.l10n
          .emptyFileType(_controller.files.filter.labelFor(context.l10n));
    }
    return context.l10n.noMatchingFiles;
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
                  : view.noticeFor(context.l10n)!,
              child: AppNotice(view.noticeFor(context.l10n)!,
                  compact: true,
                  trailing: view.noticeSettingsPane == null
                      ? null
                      : TextButton(
                          onPressed: () => _controller
                              .openPrivacySettings(view.noticeSettingsPane!),
                          child: Text(context.l10n.openSettings,
                              style: TextStyle(fontSize: 11)))))));

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
      final sourceLabel = _view.sourceLabelFor(candidate, context.l10n);
      final shortcut = _controller.shortcutLabelFor(candidate);
      Widget row() => CandidateRow(
          fileObject: _view.objectActionTitle == null,
          actionShortcut: shortcut,
          primaryLabel: _view.objectActionTitle == null &&
                  (candidate.kind == CandidateKindDto.file ||
                      candidate.kind == CandidateKindDto.application)
              ? context.l10n.open
              : candidate.commandId == 'core.files.search'
                  ? context.l10n.search
                  : context.l10n.run,
          onPrimary: !_view.resolving && !_view.executing
              ? () => _controller.selectAndExecute(candidate.id)
              : null,
          onActions: selected &&
                  !_view.resolving &&
                  !_view.executing &&
                  _controller.canShowObjectActions
              ? () => _runAndFocus(_controller.showObjectActions)
              : null,
          onDoubleTap: !_view.resolving && !_view.executing
              ? () => _controller.selectAndExecute(candidate.id)
              : null,
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
      return child;
    }
    return MenuNavigationRow(
        title: entry.navigationPath?.isEmpty == true
            ? context.l10n.browseMenus
            : entry.title,
        selected: index == _view.selectedIndex,
        onTap: () => _openEntry(index));
  }
}
