import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import '../../infrastructure/desktop_environment.dart';
import '../../l10n/app_language.dart';
import '../../src/rust/api.dart';
import '../../tools/tool_page_shell.dart';
import '../../ui/app_components.dart';
import '../../ui/app_icons.dart';
import '../../ui/app_style.dart';
import 'clipboard_history_list.dart';
import 'clipboard_history_preview.dart';
import 'clipboard_history_session.dart';
import 'clipboard_shelf_controls.dart';

/// One keyboard owner for search, selection and inline commands. Browsing never
/// copies a payload or activates another app; those are explicit operations.
class ClipboardShelfPage extends StatefulWidget {
  const ClipboardShelfPage(
      {super.key,
      required this.session,
      required this.onClose,
      required this.onReturnToTarget,
      required this.visible,
      required this.presentation});
  final ClipboardHistorySession session;
  final VoidCallback onClose;
  final Future<void> Function() onReturnToTarget;
  final bool visible;
  final int presentation;
  @override
  State<ClipboardShelfPage> createState() => _ClipboardShelfPageState();
}

class _ClipboardShelfPageState extends State<ClipboardShelfPage> {
  final _previewScroll = ScrollController(keepScrollOffset: false);
  bool _actions = false, _confirmDelete = false;
  int _actionIndex = 0;
  ClipboardItemDto? _actionTarget, _deleteTarget;
  (String?, double)? _previewOrigin;
  int _previewLayoutEpoch = 0;
  double _lastListOffset = 0;
  ClipboardHistorySession get session => widget.session;
  bool get active =>
      widget.visible &&
      session.canInteract &&
      !session.mutating &&
      session.query.value.composing.isCollapsed;
  bool get _targetExists =>
      _actionTarget != null &&
      session.items.any((item) => item.id == _actionTarget!.id);
  List<bool> get _enabledActions =>
      [_targetExists, _targetExists, _targetExists, _targetExists, active];

  @override
  void didUpdateWidget(ClipboardShelfPage old) {
    super.didUpdateWidget(old);
    if (old.presentation != widget.presentation || !widget.visible) {
      _actions = false;
      _actionTarget = _deleteTarget = null;
      _confirmDelete = false;
    }
  }

  @override
  void dispose() {
    _previewScroll.dispose();
    super.dispose();
  }

  void _focusSearch({bool selectAll = false}) {
    if (!mounted || !widget.visible || !session.canInteract) return;
    session.focus.requestFocus();
    if (selectAll) {
      session.query.selection =
          TextSelection(baseOffset: 0, extentOffset: session.query.text.length);
    }
  }

  void _back() {
    if (!session.query.value.composing.isCollapsed) return;
    if (_deleteTarget != null || _actions) {
      setState(() {
        _deleteTarget = _actionTarget = null;
        _confirmDelete = false;
        _actions = false;
      });
    } else if (session.previewEnabled) {
      _togglePreview();
    } else if (session.query.text.isNotEmpty) {
      session.query.clear();
    } else {
      widget.onClose();
    }
    _focusSearch();
  }

  void _select(ClipboardItemDto item) {
    if (!active) return;
    final index = session.items.indexWhere((row) => row.id == item.id);
    if (index >= 0) unawaited(session.select(index));
    _focusSearch();
  }

  Future<void> _copy(
      {ClipboardItemDto? item, bool returnToTarget = false}) async {
    if (!active) return;
    final copied =
        item == null ? await session.copy() : await session.copyItem(item.id);
    if (!mounted || !widget.visible || !session.canInteract) return;
    if (copied && returnToTarget) await widget.onReturnToTarget();
    _focusSearch();
  }

  void _togglePreview() {
    if (!active || (!session.previewEnabled && session.selected == null)) {
      return;
    }
    final closing = session.previewEnabled;
    final origin = _previewOrigin;
    if (!closing) {
      _previewOrigin = (
        session.selected?.id,
        session.scroll.hasClients ? session.scroll.offset : _lastListOffset
      );
    }
    final epoch = ++_previewLayoutEpoch;
    session.setPreviewEnabled(!closing);
    // Resizing the list must keep its selected row in view, without changing it.
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!mounted || !active || epoch != _previewLayoutEpoch) return;
      if (closing &&
          origin != null &&
          origin.$1 == session.selected?.id &&
          session.scroll.hasClients) {
        session.scroll.jumpTo(
            origin.$2.clamp(0, session.scroll.position.maxScrollExtent));
      }
      unawaited(session.select(session.selectedIndex));
    });
    _focusSearch();
  }

  void _toggleActions([ClipboardItemDto? item]) {
    if (!active) return;
    if (item != null) _select(item);
    if (session.scroll.hasClients) _lastListOffset = session.scroll.offset;
    setState(() {
      _actions = !_actions;
      _actionTarget = _actions ? session.selected : null;
      _actionIndex = _actionTarget == null ? ShelfAction.refresh.index : 0;
    });
    _focusSearch();
  }

  Future<void> _runAction(ShelfAction action) async {
    if (!active) return;
    final target = _actionTarget;
    if (action != ShelfAction.refresh && !_targetExists) return;
    setState(() {
      _actions = false;
      _actionTarget = null;
    });
    switch (action) {
      case ShelfAction.copy:
        await _copy(item: target);
      case ShelfAction.copyAndReturn:
        await _copy(item: target, returnToTarget: true);
      case ShelfAction.preview:
        _select(target!);
        _togglePreview();
      case ShelfAction.delete:
        _askDelete(target!);
      case ShelfAction.refresh:
        await session.refresh();
    }
    _focusSearch();
  }

  void _askDelete(ClipboardItemDto item) {
    if (!active) return;
    setState(() {
      _deleteTarget = item;
      _confirmDelete = false;
    });
    _focusSearch();
  }

  Future<void> _delete() async {
    final target = _deleteTarget;
    if (!active || target == null) return;
    setState(() {
      _deleteTarget = null;
      _confirmDelete = false;
    });
    await session.deleteItem(target.id);
    _focusSearch();
  }

  void _moveAction(int direction) {
    final enabled = _enabledActions;
    for (var step = 1; step <= enabled.length; step++) {
      final next = (_actionIndex + direction * step) % enabled.length;
      if (enabled[next]) {
        setState(() => _actionIndex = next);
        return;
      }
    }
  }

  KeyEventResult _key(KeyEvent event) {
    if (!active || (event is! KeyDownEvent && event is! KeyRepeatEvent)) {
      return KeyEventResult.ignored;
    }
    final key = event.logicalKey;
    // Pressing a modifier to invoke a command must not dismiss its action list.
    if (const [
      LogicalKeyboardKey.metaLeft,
      LogicalKeyboardKey.metaRight,
      LogicalKeyboardKey.controlLeft,
      LogicalKeyboardKey.controlRight,
      LogicalKeyboardKey.altLeft,
      LogicalKeyboardKey.altRight,
      LogicalKeyboardKey.shiftLeft,
      LogicalKeyboardKey.shiftRight,
    ].contains(key)) {
      return KeyEventResult.ignored;
    }
    final keyboard = HardwareKeyboard.instance;
    final command = DesktopEnvironment.current.commandPressed;
    final plain = !keyboard.isControlPressed &&
        !keyboard.isMetaPressed &&
        !keyboard.isAltPressed &&
        !keyboard.isShiftPressed;
    final enter = key == LogicalKeyboardKey.enter ||
        key == LogicalKeyboardKey.numpadEnter;
    if (_deleteTarget != null) {
      if (plain &&
          (key == LogicalKeyboardKey.arrowLeft ||
              key == LogicalKeyboardKey.arrowRight)) {
        setState(() => _confirmDelete = key == LogicalKeyboardKey.arrowRight);
      } else if (plain && enter && event is KeyDownEvent) {
        if (_confirmDelete) {
          unawaited(_delete());
        } else {
          _back();
        }
      }
      return KeyEventResult.handled;
    }
    if (_actions) {
      if (plain &&
          (key == LogicalKeyboardKey.arrowDown ||
              key == LogicalKeyboardKey.arrowUp)) {
        _moveAction(key == LogicalKeyboardKey.arrowDown ? 1 : -1);
        return KeyEventResult.handled;
      }
      if (plain && enter) {
        if (event is KeyDownEvent) {
          unawaited(_runAction(ShelfAction.values[_actionIndex]));
        }
        return KeyEventResult.handled;
      }
      if (command && key == LogicalKeyboardKey.keyK) {
        if (event is KeyDownEvent) _toggleActions();
        return KeyEventResult.handled;
      }
      // Typing resumes search instead of leaving a hidden menu in control.
      setState(() {
        _actions = false;
        _actionTarget = null;
      });
    }
    if (command &&
        keyboard.isShiftPressed &&
        !keyboard.isAltPressed &&
        key == LogicalKeyboardKey.backspace) {
      if (event is KeyDownEvent && session.selected != null) {
        _askDelete(session.selected!);
      }
      return KeyEventResult.handled;
    }
    if (command && !keyboard.isAltPressed && !keyboard.isShiftPressed) {
      if (event is KeyRepeatEvent) return KeyEventResult.handled;
      if (key == LogicalKeyboardKey.keyF) {
        _focusSearch(selectAll: true);
      } else if (key == LogicalKeyboardKey.keyK) {
        _toggleActions();
      } else if (key == LogicalKeyboardKey.keyY) {
        _togglePreview();
      } else if (key == LogicalKeyboardKey.keyR) {
        unawaited(session.refresh());
      } else if (enter) {
        unawaited(_copy(returnToTarget: true));
      } else if (key == LogicalKeyboardKey.arrowUp ||
          key == LogicalKeyboardKey.arrowDown) {
        unawaited(session.select(
            key == LogicalKeyboardKey.arrowUp ? 0 : session.items.length - 1));
      } else {
        final index = const [
          LogicalKeyboardKey.digit1,
          LogicalKeyboardKey.digit2,
          LogicalKeyboardKey.digit3,
          LogicalKeyboardKey.digit4,
          LogicalKeyboardKey.digit5,
          LogicalKeyboardKey.digit6,
          LogicalKeyboardKey.digit7,
          LogicalKeyboardKey.digit8,
          LogicalKeyboardKey.digit9
        ].indexOf(key);
        if (index < 0) return KeyEventResult.ignored;
        if (index < session.items.length) {
          unawaited(_copy(item: session.items[index]));
        }
      }
      return KeyEventResult.handled;
    }
    if (keyboard.isAltPressed &&
        !keyboard.isControlPressed &&
        !keyboard.isMetaPressed &&
        !keyboard.isShiftPressed &&
        session.previewEnabled &&
        [
          LogicalKeyboardKey.arrowUp,
          LogicalKeyboardKey.arrowDown,
          LogicalKeyboardKey.pageUp,
          LogicalKeyboardKey.pageDown
        ].contains(key)) {
      if (_previewScroll.hasClients) {
        final forward = key == LogicalKeyboardKey.arrowDown ||
            key == LogicalKeyboardKey.pageDown;
        final distance = key == LogicalKeyboardKey.pageUp ||
                key == LogicalKeyboardKey.pageDown
            ? _previewScroll.position.viewportDimension * .8
            : 40.0;
        _previewScroll.jumpTo(
            (_previewScroll.offset + (forward ? distance : -distance))
                .clamp(0, _previewScroll.position.maxScrollExtent));
      }
      return KeyEventResult.handled;
    }
    if (!plain) return KeyEventResult.ignored;
    if ([
      LogicalKeyboardKey.arrowDown,
      LogicalKeyboardKey.arrowUp,
      LogicalKeyboardKey.pageDown,
      LogicalKeyboardKey.pageUp
    ].contains(key)) {
      final forward = key == LogicalKeyboardKey.arrowDown ||
          key == LogicalKeyboardKey.pageDown;
      final page = key == LogicalKeyboardKey.pageDown ||
          key == LogicalKeyboardKey.pageUp;
      final count = page && session.scroll.hasClients
          ? (session.scroll.position.viewportDimension / session.rowExtent)
              .floor()
              .clamp(1, 100)
          : 1;
      unawaited(
          session.select(session.selectedIndex + (forward ? count : -count)));
      return KeyEventResult.handled;
    }
    if (enter) {
      if (event is KeyDownEvent) unawaited(_copy());
      return KeyEventResult.handled;
    }
    if (session.query.text.isEmpty &&
        (key == LogicalKeyboardKey.space ||
            key == LogicalKeyboardKey.arrowRight ||
            key == LogicalKeyboardKey.arrowLeft)) {
      if (event is KeyDownEvent &&
          (key == LogicalKeyboardKey.space ||
              session.previewEnabled ==
                  (key == LogicalKeyboardKey.arrowLeft))) {
        _togglePreview();
      }
      return KeyEventResult.handled;
    }
    return KeyEventResult.ignored;
  }

  @override
  Widget build(BuildContext context) {
    final command = DesktopEnvironment.current.commandLabel;
    final selected = session.selected;
    final labels = [
      context.l10n.copy,
      context.l10n.shelfCopyAndReturn,
      session.previewEnabled ? context.l10n.hidePreview : context.l10n.preview,
      context.l10n.deleteEntry,
      context.l10n.refresh
    ];
    return ToolPageShell(
      title: context.l10n.clipboardShelf,
      detached: true,
      headerHeight: 64,
      onBack: _back,
      escapeHint: _actions || _deleteTarget != null
          ? context.l10n.cancelEsc
          : session.previewEnabled
              ? context.l10n.backToListEsc
              : session.query.text.isNotEmpty
                  ? context.l10n.shortcutHint(context.l10n.clearSearch, 'Esc')
                  : context.l10n.closeEsc,
      leading: Tooltip(
          message: context.l10n.shelfSelectHint,
          child: const Icon(AppIcons.shelf,
              size: 20, color: DesignTokens.iconAmber)),
      header: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(
                session.recording
                    ? context.l10n.clipboardAlwaysOnTop
                    : '${context.l10n.clipboardAlwaysOnTop} · ${context.l10n.paused}',
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: AppStyle.caption.copyWith(fontSize: 10)),
            Row(children: [
              Expanded(
                  child: TextField(
                      key: const Key('clipboard-search'),
                      controller: session.query,
                      focusNode: session.focus,
                      autofocus: widget.visible,
                      selectAllOnFocus: false,
                      textInputAction: TextInputAction.none,
                      onEditingComplete: () {},
                      onChanged: (_) {
                        if (_actions || _deleteTarget != null) {
                          setState(() {
                            _actions = false;
                            _deleteTarget = null;
                          });
                        }
                      },
                      style: AppStyle.body.copyWith(fontSize: 15),
                      cursorColor: AppStyle.primary,
                      cursorWidth: 1.2,
                      decoration: InputDecoration(
                          hintText: context.l10n.searchClipboard,
                          hintStyle: AppStyle.body
                              .copyWith(fontSize: 15, color: AppStyle.muted),
                          isCollapsed: true,
                          isDense: true,
                          filled: false,
                          contentPadding: EdgeInsets.zero,
                          border: InputBorder.none,
                          enabledBorder: InputBorder.none,
                          focusedBorder: InputBorder.none))),
              if (session.query.text.isNotEmpty)
                ExcludeFocus(
                    child: IconButton(
                        key: const Key('shelf-clear-search'),
                        tooltip: context.l10n.clearSearch,
                        visualDensity: VisualDensity.compact,
                        iconSize: 14,
                        onPressed: !active
                            ? null
                            : () {
                                session.query.clear();
                                _focusSearch();
                              },
                        icon: const Icon(AppIcons.close))),
              if (session.items.isEmpty)
                ExcludeFocus(
                    child: AppMoreAction(
                        key: const Key('shelf-empty-actions'),
                        onPressed: active ? _toggleActions : null)),
            ]),
          ]),
      editorFocus: session.focus,
      presentation: widget.presentation,
      visible: widget.visible,
      onKeyEvent: _key,
      notice: session.noticeFor(context.l10n),
      child: _deleteTarget != null
          ? ShelfDeletePrompt(
              summary: _deleteTarget!.summary,
              confirmSelected: _confirmDelete,
              onCancel: _back,
              onConfirm: () => unawaited(_delete()))
          : _actions
              ? ShelfActionList(
                  labels: labels,
                  shortcuts: [
                    '↵',
                    command('↵'),
                    command('Y'),
                    DesktopEnvironment.current.commandShiftLabel('⌫'),
                    command('R')
                  ],
                  target: _actionTarget?.summary,
                  enabled: _enabledActions,
                  selected: _actionIndex,
                  onInvoke: (index) =>
                      unawaited(_runAction(ShelfAction.values[index])))
              : LayoutBuilder(
                  builder: (context, constraints) => Column(children: [
                        Expanded(
                            child: session.items.isEmpty
                                ? Center(
                                    child: Padding(
                                        padding: const EdgeInsets.all(16),
                                        child: Text(
                                            session.loading
                                                ? context.l10n.searching
                                                : session.query.text.isNotEmpty
                                                    ? context
                                                        .l10n.noMatchingEntries
                                                    : context.l10n
                                                        .noSavedEntriesManageRecordingInSettings,
                                            textAlign: TextAlign.center,
                                            style: AppStyle.caption)))
                                : ClipboardHistoryList(
                                    items: session.items,
                                    selectedIndex: session.selectedIndex,
                                    scroll: session.scroll,
                                    compact: true,
                                    selectedStatus: session.status == null
                                        ? null
                                        : context.l10n.copied,
                                    selectedCommands: AppRowActions(
                                        primaryLabel: context.l10n.copy,
                                        onPrimary: active && selected != null
                                            ? () =>
                                                unawaited(_copy(item: selected))
                                            : null,
                                        onActions: active && selected != null
                                            ? () => _toggleActions(selected)
                                            : null),
                                    onSelect: active ? _select : null,
                                    onActivate: active
                                        ? (item) => unawaited(_copy(item: item))
                                        : null,
                                    onActions: active ? _toggleActions : null)),
                        if (session.previewEnabled) ...[
                          const Divider(height: 1),
                          SizedBox(
                              height:
                                  (constraints.maxHeight * .42).clamp(60, 200),
                              child: Column(children: [
                                Padding(
                                    padding: const EdgeInsets.only(
                                        left: 12, right: 4),
                                    child: Row(children: [
                                      Expanded(
                                          child: Tooltip(
                                              message: context.l10n
                                                  .shelfPreviewScrollHint(
                                                      DesktopEnvironment.current
                                                          .altLabel('↑↓')),
                                              child: Text(context.l10n.preview,
                                                  maxLines: 1,
                                                  overflow:
                                                      TextOverflow.ellipsis,
                                                  style: AppStyle.caption))),
                                      ExcludeFocus(
                                          child: IconButton(
                                              iconSize: 14,
                                              visualDensity:
                                                  VisualDensity.compact,
                                              tooltip: context.l10n.hidePreview,
                                              onPressed: active
                                                  ? _togglePreview
                                                  : null,
                                              icon:
                                                  const Icon(AppIcons.close))),
                                    ])),
                                Expanded(
                                    child: session.preview == null
                                        ? Center(
                                            child: Text(
                                                session.selected == null &&
                                                        !session.loading
                                                    ? context.l10n
                                                        .selectEntryToPreview
                                                    : !session.hasNotice
                                                        ? context
                                                            .l10n.loadingPreview
                                                        : context.l10n
                                                            .previewUnavailable,
                                                style: AppStyle.caption))
                                        : ClipboardHistoryPreview(
                                            key: ValueKey(
                                                session.preview!.item.id),
                                            detail: session.preview!,
                                            compact: true,
                                            scrollController: _previewScroll)),
                              ])),
                        ],
                      ])),
    );
  }
}
