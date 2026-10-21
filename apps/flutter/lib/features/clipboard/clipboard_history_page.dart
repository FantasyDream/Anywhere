import 'dart:async';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import '../../infrastructure/desktop_environment.dart';
import '../../ui/app_style.dart';
import '../../ui/app_icons.dart';
import 'clipboard_history_list.dart';
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
  ClipboardHistorySession get session => widget.session;
  bool get active =>
      widget.visible &&
      session.canInteract &&
      !session.mutating &&
      session.query.value.composing.isCollapsed;
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
        await session.copy();
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
        !keyboard.isAltPressed &&
        !keyboard.isShiftPressed &&
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
        subtitle: 'Clipboard',
        onBack: widget.onBack,
        editorFocus: session.focus,
        presentation: widget.presentation,
        visible: widget.visible,
        onKeyEvent: _key,
        notice: session.notice,
        showFooterBack: false,
        header: Column(
            mainAxisSize: MainAxisSize.min,
            mainAxisAlignment: MainAxisAlignment.center,
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text('Clipboard History',
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: AppStyle.caption.copyWith(
                      fontSize: DesignTokens.pageModeSize,
                      fontWeight: DesignTokens.pageModeWeight)),
              const SizedBox(height: 3),
              TextField(
                  key: const Key('clipboard-search'),
                  controller: session.query,
                  focusNode: session.focus,
                  autofocus: widget.visible,
                  selectAllOnFocus: false,
                  textInputAction: TextInputAction.none,
                  onEditingComplete: () {},
                  style: AppStyle.body
                      .copyWith(fontSize: DesignTokens.pageHeadingSize),
                  cursorColor: AppStyle.primary,
                  cursorWidth: 1.4,
                  decoration: InputDecoration(
                      hintText: 'Search clipboard history…',
                      hintStyle: AppStyle.body.copyWith(
                          fontSize: DesignTokens.pageHeadingSize,
                          color: AppStyle.muted),
                      isCollapsed: true,
                      isDense: true,
                      filled: false,
                      border: InputBorder.none,
                      enabledBorder: InputBorder.none,
                      focusedBorder: InputBorder.none,
                      contentPadding: EdgeInsets.zero)),
            ]),
        footer: Row(children: [
          Expanded(
              child: Row(children: [
            Flexible(
                child: Text(
                    '${session.items.length} ${session.items.length == 1 ? 'entry' : 'entries'}${!session.recording ? ' · Paused' : ''}${session.loading ? ' · Searching…' : ''}',
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    style: AppStyle.caption)),
            if (session.status != null) ...[
              const SizedBox(width: 12),
              Tooltip(
                  message: session.status!,
                  child: const Row(mainAxisSize: MainAxisSize.min, children: [
                    Icon(AppIcons.check, size: 12, color: AppStyle.success),
                    SizedBox(width: 4),
                    Text('Copied',
                        style:
                            TextStyle(fontSize: 11, color: AppStyle.success)),
                  ])),
            ]
          ])),
          PopupMenuButton<_HistoryAction>(
              key: _menu,
              enabled: active,
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
              child: Semantics(
                  button: true,
                  child: Padding(
                      padding: const EdgeInsets.symmetric(
                          horizontal: 10, vertical: 6),
                      child: Text(
                          'Actions · ${DesktopEnvironment.current.commandLabel('K')}',
                          style: AppStyle.caption)))),
          const SizedBox(width: 8),
          OutlinedButton(
              onPressed: active && session.selected != null
                  ? () => _action(_HistoryAction.copy)
                  : null,
              child: const Text('Copy ↵')),
        ]),
        child: LayoutBuilder(builder: (context, constraints) {
          // Give browsing most of the window; short snippets should not reserve
          // half the page. Keep a readable preview in narrower history windows.
          final previewWidth = (constraints.maxWidth * .32).clamp(220.0, 340.0);
          return Row(crossAxisAlignment: CrossAxisAlignment.stretch, children: [
            Expanded(
                key: const Key('clipboard-list-pane'),
                child: session.items.isEmpty
                    ? _empty()
                    : ClipboardHistoryList(
                        items: session.items,
                        selectedIndex: session.selectedIndex,
                        scroll: session.scroll,
                        onSelect: !active
                            ? null
                            : (item) {
                                final index = session.items
                                    .indexWhere((row) => row.id == item.id);
                                if (index < 0) return;
                                unawaited(session.select(index));
                                session.focus.requestFocus();
                              },
                      )),
            const VerticalDivider(width: 1),
            SizedBox(
                key: const Key('clipboard-preview-pane'),
                width: previewWidth,
                child: session.preview == null
                    ? Center(
                        child: Text(
                            session.items.isEmpty
                                ? 'Preview'
                                : session.notice != null
                                    ? 'Preview unavailable'
                                    : 'Loading preview…',
                            style: AppStyle.caption))
                    : ClipboardHistoryPreview(
                        key: ValueKey(session.preview!.item.id),
                        detail: session.preview!)),
          ]);
        }),
      );

  Widget _empty() => Center(
      child: Padding(
          padding: const EdgeInsets.all(24),
          child: Column(mainAxisSize: MainAxisSize.min, children: [
            const Icon(AppIcons.clipboard, size: 28, color: AppStyle.secondary),
            const SizedBox(height: 12),
            Text(
                session.loading
                    ? 'Loading…'
                    : session.query.text.isNotEmpty
                        ? 'No matching entries'
                        : 'No saved entries',
                style: AppStyle.body),
            if (!session.loading && session.query.text.isEmpty) ...[
              const SizedBox(height: 6),
              Text(
                  session.recording
                      ? 'Newly copied items will appear here.\nRefresh to see new copies.'
                      : 'Manage recording in Settings → Clipboard.',
                  style: AppStyle.caption,
                  textAlign: TextAlign.center),
            ],
          ])));
}
