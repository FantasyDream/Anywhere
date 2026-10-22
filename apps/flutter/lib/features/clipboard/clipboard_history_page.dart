import '../../l10n/app_language.dart';
import 'dart:async';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import '../../infrastructure/desktop_environment.dart';
import '../../ui/app_style.dart';
import '../../ui/app_icons.dart';
import '../../ui/app_components.dart';
import '../../src/rust/api.dart';
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

  void _select(ClipboardItemDto item) {
    if (!active) return;
    final index = session.items.indexWhere((row) => row.id == item.id);
    if (index < 0) return;
    unawaited(session.select(index));
    session.focus.requestFocus();
  }

  void _showActions([ClipboardItemDto? item]) {
    if (!active) return;
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
  Widget build(BuildContext context) => ToolPageShell(
        title: context.l10n.clipboardHistory,
        subtitle: context.l10n.clipboard,
        onBack: widget.onBack,
        editorFocus: session.focus,
        presentation: widget.presentation,
        visible: widget.visible,
        onKeyEvent: _key,
        notice: session.noticeFor(context.l10n),
        header: Row(children: [
          Expanded(
              child: Column(
                  mainAxisSize: MainAxisSize.min,
                  mainAxisAlignment: MainAxisAlignment.center,
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                Text(context.l10n.clipboardHistory,
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
                        hintText: context.l10n.searchHistory,
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
              ])),
          if (session.items.isEmpty)
            _actionMenu(AppMoreAction(
                key: const Key('history-empty-actions'),
                onPressed: active ? _showActions : null)),
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
                        selectedStatus: session.statusFor(context.l10n) == null
                            ? null
                            : context.l10n.copied,
                        selectedCommands: _actionMenu(AppRowActions(
                            primaryLabel: context.l10n.copy,
                            onPrimary: active && session.selected != null
                                ? () => unawaited(_action(_HistoryAction.copy))
                                : null,
                            onActions: active ? _showActions : null)),
                        onSelect: active ? _select : null,
                        onActivate: active
                            ? (item) => unawaited(
                                _action(_HistoryAction.copy, target: item.id))
                            : null,
                        onActions: active ? _showActions : null,
                      )),
            const VerticalDivider(width: 1),
            SizedBox(
                key: const Key('clipboard-preview-pane'),
                width: previewWidth,
                child: session.preview == null
                    ? Center(
                        child: Text(
                            session.items.isEmpty
                                ? context.l10n.preview
                                : session.hasNotice
                                    ? context.l10n.previewUnavailable
                                    : context.l10n.loadingPreview,
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
