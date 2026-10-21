import '../../l10n/app_language.dart';
import 'dart:async';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import '../../infrastructure/desktop_environment.dart';
import '../../src/rust/api.dart';
import '../../tools/tool_page_shell.dart';
import '../../ui/app_icons.dart';
import '../../ui/app_style.dart';
import 'clipboard_history_list.dart';
import 'clipboard_history_preview.dart';
import 'clipboard_history_session.dart';

enum _ShelfAction { copy, preview, delete, refresh }

/// Compact, persistent picker. Shares the history session and row renderer;
/// reading a full body is opt-in rather than a side effect of browsing the list.
class ClipboardShelfPage extends StatefulWidget {
  const ClipboardShelfPage(
      {super.key,
      required this.session,
      required this.onClose,
      required this.onCopied,
      required this.visible,
      required this.presentation});
  final ClipboardHistorySession session;
  final VoidCallback onClose;
  final Future<void> Function() onCopied;
  final bool visible;
  final int presentation;
  @override
  State<ClipboardShelfPage> createState() => _ClipboardShelfPageState();
}

class _ClipboardShelfPageState extends State<ClipboardShelfPage> {
  final _menu = GlobalKey<PopupMenuButtonState<_ShelfAction>>();
  double _listOffset = 0;
  ClipboardHistorySession get session => widget.session;
  bool get active =>
      widget.visible &&
      session.canInteract &&
      !session.mutating &&
      session.query.value.composing.isCollapsed;

  void _back() {
    if (session.previewEnabled) {
      session.setPreviewEnabled(false);
      session.focus.requestFocus();
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (!mounted ||
            !widget.visible ||
            session.previewEnabled ||
            !session.scroll.hasClients) {
          return;
        }
        session.scroll.jumpTo(
            _listOffset.clamp(0, session.scroll.position.maxScrollExtent));
        unawaited(session.select(session.selectedIndex));
      });
    } else {
      widget.onClose();
    }
  }

  Future<void> _copy([ClipboardItemDto? item]) async {
    if (!active) return;
    final copied =
        item == null ? await session.copy() : await session.copyItem(item.id);
    if (!mounted || !widget.visible || !session.canInteract) return;
    if (copied) await widget.onCopied();
    if (mounted && widget.visible) session.focus.requestFocus();
  }

  Future<void> _action(_ShelfAction action) async {
    if (!active) return;
    switch (action) {
      case _ShelfAction.copy:
        await _copy();
      case _ShelfAction.preview:
        if (session.selected != null && !session.previewEnabled) {
          _listOffset = session.scroll.hasClients ? session.scroll.offset : 0;
          session.setPreviewEnabled(true);
        }
      case _ShelfAction.delete:
        session.setPreviewEnabled(false);
        await session.delete();
      case _ShelfAction.refresh:
        await session.refresh();
    }
    if (mounted && widget.visible) session.focus.requestFocus();
  }

  KeyEventResult _key(KeyEvent event) {
    if (!active) return KeyEventResult.ignored;
    final key = event.logicalKey;
    final keyboard = HardwareKeyboard.instance;
    if (DesktopEnvironment.current.commandPressed &&
        !keyboard.isAltPressed &&
        !keyboard.isShiftPressed) {
      if (key == LogicalKeyboardKey.keyK) {
        _menu.currentState?.showButtonMenu();
        return KeyEventResult.handled;
      }
      final index = const [
        LogicalKeyboardKey.digit1,
        LogicalKeyboardKey.digit2,
        LogicalKeyboardKey.digit3,
        LogicalKeyboardKey.digit4,
        LogicalKeyboardKey.digit5,
        LogicalKeyboardKey.digit6,
        LogicalKeyboardKey.digit7,
        LogicalKeyboardKey.digit8,
        LogicalKeyboardKey.digit9,
      ].indexOf(key);
      if (index >= 0) {
        if (index < session.items.length) {
          unawaited(_copy(session.items[index]));
        }
        return KeyEventResult.handled;
      }
    }
    if (keyboard.isControlPressed ||
        keyboard.isMetaPressed ||
        keyboard.isAltPressed ||
        keyboard.isShiftPressed) {
      return KeyEventResult.ignored;
    }
    if (!session.previewEnabled &&
        (key == LogicalKeyboardKey.arrowDown ||
            key == LogicalKeyboardKey.arrowUp)) {
      unawaited(session.select(session.selectedIndex +
          (key == LogicalKeyboardKey.arrowDown ? 1 : -1)));
      return KeyEventResult.handled;
    }
    if (key == LogicalKeyboardKey.enter ||
        key == LogicalKeyboardKey.numpadEnter) {
      unawaited(_copy());
      return KeyEventResult.handled;
    }
    return KeyEventResult.ignored;
  }

  @override
  Widget build(BuildContext context) => ToolPageShell(
      title: context.l10n.clipboardShelf,
      detached: !session.previewEnabled,
      headerHeight: 60,
      leading: session.previewEnabled
          ? IconButton(
              tooltip: 'Back to list · Esc',
              onPressed: _back,
              icon: const Icon(AppIcons.back, size: 18))
          : const SizedBox.square(
              dimension: 26,
              child: Icon(AppIcons.clipboard,
                  size: 18, color: AppStyle.secondary)),
      header: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(
                session.previewEnabled
                    ? 'Clipboard · Preview'
                    : 'Clipboard · Always on top',
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: AppStyle.caption.copyWith(fontSize: 10)),
            const SizedBox(height: 3),
            TextField(
                key: const Key('clipboard-search'),
                controller: session.query,
                focusNode: session.focus,
                autofocus: widget.visible,
                selectAllOnFocus: false,
                textInputAction: TextInputAction.none,
                onEditingComplete: () {},
                onChanged: (_) {
                  if (session.previewEnabled) session.setPreviewEnabled(false);
                },
                style: AppStyle.body.copyWith(fontSize: 15),
                cursorColor: AppStyle.primary,
                cursorWidth: 1.2,
                decoration: InputDecoration(
                    hintText: 'Search clipboard…',
                    hintStyle: AppStyle.body
                        .copyWith(fontSize: 15, color: AppStyle.muted),
                    isCollapsed: true,
                    isDense: true,
                    filled: false,
                    contentPadding: EdgeInsets.zero,
                    border: InputBorder.none,
                    enabledBorder: InputBorder.none,
                    focusedBorder: InputBorder.none)),
          ]),
      onBack: _back,
      showFooterBack: false,
      editorFocus: session.focus,
      presentation: widget.presentation,
      visible: widget.visible,
      onKeyEvent: _key,
      notice: session.notice,
      footer: Row(children: [
        Expanded(
            child: Text(
                session.status != null
                    ? 'Copied'
                    : '${session.items.length} entries${!session.recording ? ' · Paused' : ''}',
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: AppStyle.caption.copyWith(
                    color: session.status != null
                        ? AppStyle.success
                        : AppStyle.muted))),
        PopupMenuButton<_ShelfAction>(
            key: _menu,
            enabled: active,
            tooltip:
                'Actions · ${DesktopEnvironment.current.commandLabel('K')}',
            onCanceled: () {
              if (widget.visible && session.canInteract) {
                session.focus.requestFocus();
              }
            },
            onSelected: (action) => unawaited(_action(action)),
            itemBuilder: (_) => [
                  PopupMenuItem(
                      value: _ShelfAction.copy,
                      height: 32,
                      enabled: session.selected != null,
                      child: const Text('Copy · ↵')),
                  PopupMenuItem(
                      value: _ShelfAction.preview,
                      height: 32,
                      enabled:
                          session.selected != null && !session.previewEnabled,
                      child: Text(context.l10n.preview)),
                  PopupMenuItem(
                      value: _ShelfAction.delete,
                      height: 32,
                      enabled: session.selected != null,
                      child: const Text('Delete Entry')),
                  const PopupMenuItem(
                      value: _ShelfAction.refresh,
                      height: 32,
                      child: Text('Refresh')),
                ],
            child: Semantics(
                label:
                    'Actions · ${DesktopEnvironment.current.commandLabel('K')}',
                button: true,
                child: Padding(
                    padding:
                        const EdgeInsets.symmetric(horizontal: 6, vertical: 4),
                    child: Row(mainAxisSize: MainAxisSize.min, children: [
                      const Icon(AppIcons.more,
                          size: 16, color: AppStyle.secondary),
                      const SizedBox(width: 4),
                      Text(DesktopEnvironment.current.commandLabel('K'),
                          style: AppStyle.caption.copyWith(fontSize: 10)),
                    ])))),
        const SizedBox(width: 4),
        OutlinedButton(
            onPressed: active && session.selected != null ? _copy : null,
            child: const Text('Copy ↵')),
      ]),
      child: session.previewEnabled
          ? session.preview == null
              ? Center(
                  child: Text(
                      session.notice == null
                          ? 'Loading preview…'
                          : 'Preview unavailable',
                      style: AppStyle.caption))
              : ClipboardHistoryPreview(
                  key: ValueKey(session.preview!.item.id),
                  detail: session.preview!)
          : session.items.isEmpty
              ? Center(
                  child: Padding(
                      padding: const EdgeInsets.all(20),
                      child: Text(
                          session.loading
                              ? 'Searching…'
                              : session.query.text.isNotEmpty
                                  ? 'No matching entries'
                                  : 'No saved entries.\nManage recording in Settings → Clipboard.',
                          textAlign: TextAlign.center,
                          style: AppStyle.caption)))
              : ClipboardHistoryList(
                  items: session.items,
                  selectedIndex: session.selectedIndex,
                  scroll: session.scroll,
                  compact: true,
                  onSelect: active ? (item) => unawaited(_copy(item)) : null));
}
