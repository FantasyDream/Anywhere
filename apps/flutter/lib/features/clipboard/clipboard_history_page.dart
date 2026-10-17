import 'dart:async';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import '../../infrastructure/desktop_environment.dart';
import '../../ui/app_style.dart';
import 'clipboard_history_session.dart';
import 'clipboard_history_preview.dart';
import '../../tools/tool_page_shell.dart';

enum _HistoryAction { copy, delete, refresh }

class ClipboardHistoryPage extends StatefulWidget {
  const ClipboardHistoryPage(
      {super.key,
      required this.session,
      required this.onBack,
      required this.presentation,
      required this.visible,
      this.floating = false,
      this.onCopied});
  final ClipboardHistorySession session;
  final VoidCallback onBack;
  final int presentation;
  final bool visible;
  final bool floating;
  final Future<void> Function()? onCopied;
  @override
  State<ClipboardHistoryPage> createState() => _ClipboardHistoryPageState();
}

class _ClipboardHistoryPageState extends State<ClipboardHistoryPage> {
  final _menu = GlobalKey<PopupMenuButtonState<_HistoryAction>>();
  ClipboardHistorySession get session => widget.session;
  bool get active => widget.visible && session.canInteract && !session.mutating;
  @override
  void didUpdateWidget(ClipboardHistoryPage old) {
    super.didUpdateWidget(old);
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

  Future<void> _action(_HistoryAction action) async {
    if (!active) return;
    switch (action) {
      case _HistoryAction.copy:
        if (await session.copy() && mounted && widget.visible) {
          await widget.onCopied?.call();
        }
      case _HistoryAction.delete:
        await session.delete();
      case _HistoryAction.refresh:
        await session.refresh();
    }
    if (mounted && widget.visible) session.focus.requestFocus();
  }

  KeyEventResult _key(KeyEvent event) {
    if (!active) return KeyEventResult.ignored;
    final key = event.logicalKey;
    final keyboard = HardwareKeyboard.instance;
    if (DesktopEnvironment.current.commandPressed &&
        key == LogicalKeyboardKey.keyK) {
      _menu.currentState?.showButtonMenu();
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
      unawaited(session.select(session.selectedIndex +
          (key == LogicalKeyboardKey.arrowDown ? 1 : -1)));
      return KeyEventResult.handled;
    }
    if (key == LogicalKeyboardKey.enter ||
        key == LogicalKeyboardKey.numpadEnter) {
      unawaited(_action(_HistoryAction.copy));
      return KeyEventResult.handled;
    }
    return KeyEventResult.ignored;
  }

  @override
  Widget build(BuildContext context) => ToolPageShell(
        title: session.title,
        detached: widget.floating,
        subtitle: 'Clipboard',
        onBack: widget.onBack,
        editorFocus: session.focus,
        presentation: widget.presentation,
        visible: widget.visible,
        onKeyEvent: _key,
        notice: session.notice ?? (widget.floating ? null : session.status),
        header: Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
          Text(
              widget.floating
                  ? 'Clipboard Shelf · Always on top'
                  : 'Clipboard History',
              style: AppStyle.caption),
          const SizedBox(height: 4),
          TextField(
              key: const Key('clipboard-search'),
              controller: session.query,
              focusNode: session.focus,
              autofocus: true,
              // Returning from Actions restores the caret, not a select-all.
              selectAllOnFocus: false,
              textInputAction: TextInputAction.none,
              onEditingComplete: () {},
              style: AppStyle.body.copyWith(fontSize: 21),
              decoration: const InputDecoration(
                  hintText: 'Search clipboard history…',
                  filled: false,
                  border: InputBorder.none,
                  enabledBorder: InputBorder.none,
                  focusedBorder: InputBorder.none,
                  contentPadding: EdgeInsets.zero))
        ]),
        footer: Row(children: [
          Expanded(
              child: Text(
                  '${session.items.length} ${session.items.length == 1 ? 'entry' : 'entries'}${!session.recording ? ' · Paused' : ''}${session.loading ? ' · Searching…' : widget.floating && session.status != null ? ' · Copied' : ''}',
                  style: AppStyle.caption)),
          PopupMenuButton<_HistoryAction>(
              key: _menu,
              onCanceled: () {
                if (widget.visible && session.canInteract) {
                  session.focus.requestFocus();
                }
              },
              tooltip:
                  'Actions · ${DesktopEnvironment.current.commandLabel('K')}',
              onSelected: (action) => unawaited(_action(action)),
              itemBuilder: (_) => [
                    PopupMenuItem(
                        height: 32,
                        value: _HistoryAction.copy,
                        enabled: session.selected != null && active,
                        child: const Text('Copy · ↵')),
                    PopupMenuItem(
                        height: 32,
                        value: _HistoryAction.delete,
                        enabled: session.selected != null && active,
                        child: const Text('Delete Entry')),
                    PopupMenuItem(
                        height: 32,
                        value: _HistoryAction.refresh,
                        enabled: active,
                        child: const Text('Refresh')),
                  ],
              child: Padding(
                  padding:
                      const EdgeInsets.symmetric(horizontal: 10, vertical: 6),
                  child: Text(
                      'Actions · ${DesktopEnvironment.current.commandLabel('K')}',
                      style: AppStyle.caption))),
          const SizedBox(width: 8),
          OutlinedButton(
              onPressed: active && session.selected != null
                  ? () => _action(_HistoryAction.copy)
                  : null,
              child: const Text('Copy ↵')),
        ]),
        child: Column(children: [
          Expanded(
              child: Row(
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  children: [
                Expanded(
                    flex: 5,
                    child: session.items.isEmpty
                        ? Center(
                            child: Padding(
                                padding: const EdgeInsets.all(24),
                                child: Text(
                                    session.loading
                                        ? 'Loading…'
                                        : session.query.text.isNotEmpty
                                            ? 'No matching entries'
                                            : session.recording
                                                ? widget.floating
                                                    ? 'Newly copied items will appear here automatically.'
                                                    : 'Newly copied items will appear here.\nRefresh to see new copies.'
                                                : 'No saved entries.\nManage recording in Settings → Clipboard.',
                                    style: AppStyle.body,
                                    textAlign: TextAlign.center)))
                        : ListView.builder(
                            controller: session.scroll,
                            itemExtent: 64,
                            padding: EdgeInsets.zero,
                            itemCount: session.items.length,
                            itemBuilder: (context, index) {
                              final item = session.items[index];
                              return Padding(
                                  padding: const EdgeInsets.symmetric(
                                      horizontal: 10, vertical: 3),
                                  child: Material(
                                      color: index == session.selectedIndex
                                          ? AppStyle.selectedSurface
                                          : Colors.transparent,
                                      shape: RoundedRectangleBorder(
                                          borderRadius: BorderRadius.circular(
                                              AppStyle.resultRadius),
                                          side: BorderSide(
                                              color: index ==
                                                      session.selectedIndex
                                                  ? AppStyle.selectionOutline
                                                  : Colors.transparent)),
                                      child: InkWell(
                                          onDoubleTap: widget.floating && active
                                              ? () async {
                                                  await session.select(index);
                                                  if (mounted &&
                                                      active &&
                                                      session.selected?.id ==
                                                          item.id) {
                                                    await _action(
                                                        _HistoryAction.copy);
                                                  }
                                                }
                                              : null,
                                          onTap: active
                                              ? () {
                                                  unawaited(
                                                      session.select(index));
                                                  session.focus.requestFocus();
                                                }
                                              : null,
                                          borderRadius: BorderRadius.circular(
                                              AppStyle.resultRadius),
                                          child: Semantics(
                                              selected: index ==
                                                  session.selectedIndex,
                                              child: Padding(
                                                  padding: const EdgeInsets
                                                      .symmetric(
                                                      horizontal: 14,
                                                      vertical: 8),
                                                  child: Row(children: [
                                                    Icon(
                                                        clipboardKindIcon(
                                                            item.kind),
                                                        size: 22,
                                                        color:
                                                            AppStyle.secondary),
                                                    const SizedBox(width: 12),
                                                    Expanded(
                                                        child: Column(
                                                            crossAxisAlignment:
                                                                CrossAxisAlignment
                                                                    .start,
                                                            mainAxisAlignment:
                                                                MainAxisAlignment
                                                                    .center,
                                                            children: [
                                                          Text(item.summary,
                                                              maxLines: 1,
                                                              overflow:
                                                                  TextOverflow
                                                                      .ellipsis,
                                                              style: AppStyle
                                                                  .body),
                                                          const SizedBox(
                                                              height: 3),
                                                          Text(
                                                              '${clipboardKindLabel(item.kind)} · ${_size(item.byteCount)} · ${_date(context, item.copiedAtMs.toInt())}',
                                                              style: AppStyle
                                                                  .caption),
                                                        ]))
                                                  ]))))));
                            })),
                const VerticalDivider(width: 1),
                Expanded(
                    flex: 4,
                    child: Padding(
                        padding: const EdgeInsets.all(20),
                        child: Column(
                            crossAxisAlignment: CrossAxisAlignment.start,
                            children: [
                              const Text('Preview', style: AppStyle.caption),
                              const SizedBox(height: 16),
                              Expanded(
                                  child: session.preview == null
                                      ? const Center(
                                          child: Text('Select an entry',
                                              style: AppStyle.caption))
                                      : ClipboardHistoryPreview(
                                          key: ValueKey(
                                              session.preview!.item.id),
                                          detail: session.preview!)),
                            ]))),
              ])),
        ]),
      );
  String _size(int bytes) => bytes >= 1024 * 1024
      ? '${(bytes / (1024 * 1024)).toStringAsFixed(1)} MB'
      : bytes >= 1024
          ? '${(bytes / 1024).toStringAsFixed(1)} KB'
          : '$bytes B';
  String _date(BuildContext context, int milliseconds) {
    final date = DateTime.fromMillisecondsSinceEpoch(milliseconds);
    final localizations = MaterialLocalizations.of(context);
    return '${localizations.formatShortDate(date)} ${localizations.formatTimeOfDay(TimeOfDay.fromDateTime(date))}';
  }
}
