import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import '../../infrastructure/bridge_api.dart';
import '../../infrastructure/desktop_environment.dart';
import '../../l10n/app_language.dart';
import '../../tools/tool_page_shell.dart';
import '../../ui/app_components.dart';
import '../../ui/app_icons.dart';
import '../../ui/app_style.dart';
import 'clipboard_history_list.dart';
import 'clipboard_history_session.dart';

enum _HistoryAction { copy, preview, delete, refresh }

class ClipboardHistoryPage extends StatefulWidget {
  const ClipboardHistoryPage(
      {super.key,
      required this.session,
      required this.onBack,
      required this.presentation,
      required this.visible});
  final ClipboardHistorySession session;
  final VoidCallback onBack;
  final int presentation;
  final bool visible;
  @override
  State<ClipboardHistoryPage> createState() => _ClipboardHistoryPageState();
}

class _ClipboardHistoryPageState extends State<ClipboardHistoryPage> {
  final _menu = GlobalKey<PopupMenuButtonState<_HistoryAction>>();
  String? _menuTarget;
  ClipboardHistorySession get session => widget.session;
  bool get active =>
      widget.visible &&
      session.canInteract &&
      !session.mutating &&
      session.query.value.composing.isCollapsed;
  @override
  void didUpdateWidget(ClipboardHistoryPage old) {
    super.didUpdateWidget(old);
    if (!widget.visible) session.preview.close();
    if (widget.visible &&
        (!old.visible || old.presentation != widget.presentation)) {
      final presentation = widget.presentation;
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (mounted && widget.visible && widget.presentation == presentation) {
          unawaited(session.refresh());
        }
      });
    }
  }

  Future<void> _action(_HistoryAction action, {String? target}) async {
    final id = target ?? session.selected?.id;
    if (!active) return;
    switch (action) {
      case _HistoryAction.preview:
        session.preview.toggle();
      case _HistoryAction.copy:
        if (id != null) await session.copyItem(id);
      case _HistoryAction.delete:
        if (id != null) await session.deleteItem(id);
      case _HistoryAction.refresh:
        await session.refresh();
    }
    if (mounted && widget.visible) session.focus.requestFocus();
  }

  KeyEventResult _key(KeyEvent event) {
    if (!active) return KeyEventResult.ignored;
    final key = event.logicalKey;
    final keyboard = HardwareKeyboard.instance;
    if (event is! KeyDownEvent && event is! KeyRepeatEvent) {
      return KeyEventResult.ignored;
    }
    if (session.preview.handleKey(event, query: session.query.text)) {
      return KeyEventResult.handled;
    }
    if (DesktopEnvironment.current.commandPressed &&
        !keyboard.isAltPressed &&
        !keyboard.isShiftPressed &&
        key == LogicalKeyboardKey.keyK) {
      _showActions();
      return KeyEventResult.handled;
    }
    if (keyboard.isControlPressed ||
        keyboard.isMetaPressed ||
        keyboard.isAltPressed ||
        keyboard.isShiftPressed) {
      return KeyEventResult.ignored;
    }
    if (key == LogicalKeyboardKey.arrowDown ||
        key == LogicalKeyboardKey.arrowUp) {
      session.preview.browsing = true;
      session.select(session.selectedIndex +
          (key == LogicalKeyboardKey.arrowDown ? 1 : -1));
      return KeyEventResult.handled;
    }
    if (key == LogicalKeyboardKey.enter ||
        key == LogicalKeyboardKey.numpadEnter) {
      if (event is KeyDownEvent) unawaited(_action(_HistoryAction.copy));
      return KeyEventResult.handled;
    }
    return KeyEventResult.ignored;
  }

  void _select(ClipboardItemDto item) {
    if (!active) return;
    session.preview.browsing = true;
    final index = session.items.indexWhere((row) => row.id == item.id);
    if (index < 0) return;
    session.select(index);
    session.focus.requestFocus();
  }

  void _showActions([ClipboardItemDto? item]) {
    if (!active) return;
    session.preview.close();
    if (item != null) _select(item);
    final target = session.selected?.id;
    // A right-click can move the menu anchor to a newly selected row. Wait for
    // that row to mount and keep the original identity throughout the popup.
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!mounted || !active || session.selected?.id != target) return;
      _menuTarget = target;
      _menu.currentState?.showButtonMenu();
    });
    WidgetsBinding.instance.scheduleFrame();
  }

  Widget _actionMenu(Widget child) => ExcludeFocus(
      child: PopupMenuButton<_HistoryAction>(
          key: _menu,
          enabled: active,
          tooltip: context.l10n.shortcutHint(context.l10n.actions,
              DesktopEnvironment.current.commandLabel('K')),
          onCanceled: () {
            _menuTarget = null;
            if (active) session.focus.requestFocus();
          },
          onSelected: (action) {
            final target = _menuTarget;
            _menuTarget = null;
            unawaited(_action(action, target: target));
          },
          itemBuilder: (_) => [
                PopupMenuItem(
                    height: 32,
                    value: _HistoryAction.copy,
                    enabled: _menuTarget != null && active,
                    child: Text(context.l10n.copyWithReturnHint)),
                PopupMenuItem(
                    height: 32,
                    value: _HistoryAction.preview,
                    enabled: _menuTarget != null && active,
                    child: Text(context.l10n.shortcutHint(context.l10n.preview,
                        DesktopEnvironment.current.previewShortcutLabel))),
                PopupMenuItem(
                    height: 32,
                    value: _HistoryAction.delete,
                    enabled: _menuTarget != null && active,
                    child: Text(context.l10n.deleteEntry)),
                PopupMenuItem(
                    height: 32,
                    value: _HistoryAction.refresh,
                    enabled: active,
                    child: Text(context.l10n.refresh)),
              ],
          child: child));

  @override
  Widget build(BuildContext context) {
    final owner = session, presentation = widget.presentation;
    owner.measureQueryFrame(
        visible: widget.visible,
        presentation: presentation,
        current: () =>
            mounted &&
            widget.visible &&
            identical(widget.session, owner) &&
            widget.presentation == presentation);
    return ToolPageShell(
      title: context.l10n.clipboardHistory,
      subtitle: context.l10n.clipboard,
      onBack: () {
        if (session.preview.isOpen) {
          session.preview.close();
        } else {
          widget.onBack();
        }
      },
      editorFocus: session.focus,
      presentation: widget.presentation,
      visible: widget.visible,
      onKeyEvent: _key,
      notice: session.noticeFor(context.l10n),
      header: Row(children: [
        Expanded(
            child: AppPageTitle(
                label: context.l10n.clipboardHistory,
                child: TextField(
                    key: const Key('clipboard-search'),
                    controller: session.query,
                    focusNode: session.focus,
                    autofocus: widget.visible,
                    selectAllOnFocus: false,
                    textInputAction: TextInputAction.none,
                    onTap: session.preview.edited,
                    onEditingComplete: () {},
                    style: AppStyle.pageHeading,
                    cursorColor: AppStyle.primary,
                    cursorWidth: DesignTokens.searchCaretWidth,
                    decoration: AppStyle.searchDecoration(
                        context.l10n.searchHistory)))),
        if (session.items.isEmpty)
          _actionMenu(AppMoreAction(
              key: const Key('history-empty-actions'),
              onPressed: active ? _showActions : null)),
      ]),
      child: session.items.isEmpty
          ? _empty()
          : ClipboardHistoryList(
              items: session.items,
              selectedIndex: session.selectedIndex,
              scroll: session.scroll,
              selectedStatus: session.statusFor(context.l10n) == null
                  ? null
                  : context.l10n.copied,
              selectedCommands: _actionMenu(AppRowActions(
                  primaryLabel: context.l10n.copy,
                  onPreview: active ? session.preview.toggle : null,
                  previewSpace:
                      session.preview.browsing || session.query.text.isEmpty,
                  onPrimary: active && session.selected != null
                      ? () => unawaited(_action(_HistoryAction.copy))
                      : null,
                  onActions: active ? _showActions : null)),
              onSelect: active ? _select : null,
              onActivate: active
                  ? (item) =>
                      unawaited(_action(_HistoryAction.copy, target: item.id))
                  : null,
              onActions: active ? _showActions : null,
            ),
    );
  }

  Widget _empty() => Center(
      child: Padding(
          padding: const EdgeInsets.all(24),
          child: Column(mainAxisSize: MainAxisSize.min, children: [
            const Icon(AppIcons.history,
                size: 30, color: DesignTokens.iconPurple),
            const SizedBox(height: 12),
            Text(
                session.loading
                    ? context.l10n.loading
                    : session.query.text.isNotEmpty
                        ? context.l10n.noMatchingEntries
                        : context.l10n.noSavedEntries,
                style: AppStyle.body),
            if (!session.loading && session.query.text.isEmpty) ...[
              const SizedBox(height: 6),
              Text(
                  session.recording
                      ? context.l10n.newlyCopiedItemsWillAppearHereRefresh
                      : context.l10n.manageRecordingInSettingsClipboard,
                  style: AppStyle.caption,
                  textAlign: TextAlign.center),
            ],
          ])));
}
