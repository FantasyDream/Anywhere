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
import 'session.dart';

class AppHistoryPage extends StatefulWidget {
  const AppHistoryPage(
      {super.key,
      required this.session,
      required this.onBack,
      required this.presentation,
      required this.visible});
  final AppHistorySession session;
  final VoidCallback onBack;
  final int presentation;
  final bool visible;
  @override
  State<AppHistoryPage> createState() => _AppHistoryPageState();
}

class _AppHistoryPageState extends State<AppHistoryPage> {
  final _menu = GlobalKey<PopupMenuButtonState<String>>();
  String? _menuTarget;
  final _firstAction = GlobalKey();
  AppHistorySession get session => widget.session;
  @override
  void initState() {
    super.initState();
    session.setVisible(widget.visible);
  }

  @override
  void didUpdateWidget(AppHistoryPage old) {
    super.didUpdateWidget(old);
    session.setVisible(widget.visible);
  }

  KeyEventResult _key(KeyEvent event) {
    if (!session.interactive) return KeyEventResult.ignored;
    final key = event.logicalKey, keyboard = HardwareKeyboard.instance;
    if (DesktopEnvironment.current.commandPressed &&
        key == LogicalKeyboardKey.keyK) {
      _actions();
      return KeyEventResult.handled;
    }
    if (keyboard.isMetaPressed ||
        keyboard.isControlPressed ||
        keyboard.isAltPressed) {
      return KeyEventResult.ignored;
    }
    if (keyboard.isShiftPressed) return KeyEventResult.ignored;
    if (key == LogicalKeyboardKey.arrowDown ||
        key == LogicalKeyboardKey.arrowUp) {
      session.select(session.selectedIndex +
          (key == LogicalKeyboardKey.arrowDown ? 1 : -1));
      return KeyEventResult.handled;
    }
    if (key == LogicalKeyboardKey.pageDown ||
        key == LogicalKeyboardKey.pageUp) {
      session.movePage(key == LogicalKeyboardKey.pageDown ? 1 : -1);
      return KeyEventResult.handled;
    }
    if (key == LogicalKeyboardKey.enter ||
        key == LogicalKeyboardKey.numpadEnter) {
      unawaited(session.useSelection());
      return KeyEventResult.handled;
    }
    return KeyEventResult.ignored;
  }

  void _actions() {
    if (!session.interactive) return;
    final target = session.selected?.id;
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!mounted || !session.interactive || session.selected?.id != target) {
        return;
      }
      _menuTarget = target;
      _menu.currentState?.showButtonMenu();
    });
    WidgetsBinding.instance.scheduleFrame();
  }

  Widget _actionMenu(Widget child) => ExcludeFocus(
          child: PopupMenuButton<String>(
        key: _menu,
        enabled: session.interactive,
        requestFocus: true,
        onOpened: () => WidgetsBinding.instance.addPostFrameCallback((_) {
          final target = _firstAction.currentContext;
          if (mounted && target != null) FocusScope.of(target).nextFocus();
        }),
        initialValue: session.selected == null ? 'refresh' : 'copy',
        onCanceled: () {
          _menuTarget = null;
          if (session.interactive) session.focus.requestFocus();
        },
        onSelected: (action) async {
          final target = _menuTarget;
          _menuTarget = null;
          if (!session.interactive) return;
          if (action == 'copy' &&
              target != null &&
              session.selected?.id == target) {
            await session.useItem(copy: true);
          }
          if (action == 'refresh') await session.refresh();
          if (mounted && widget.visible) session.focus.requestFocus();
        },
        itemBuilder: (_) => [
          if (session.selected != null)
            PopupMenuItem(
                key: _firstAction,
                value: 'copy',
                height: 32,
                enabled: !session.loading,
                child: Text(context.l10n.appHistoryCopyTarget)),
          PopupMenuItem(
              key: session.selected == null ? _firstAction : null,
              value: 'refresh',
              height: 32,
              child: Text(context.l10n.refresh)),
        ],
        child: child,
      ));

  String? _notice() {
    final l = context.l10n;
    if (session.failed) return l.appHistoryOperationFailed;
    return switch (session.notice) {
      AppHistoryNoticeDto.disabled => l.appHistoryDisabled,
      AppHistoryNoticeDto.readFailed => l.appHistoryReadFailed,
      AppHistoryNoticeDto.busy => l.appHistoryBusy,
      AppHistoryNoticeDto.stale => l.appHistoryExpired,
      _ => null,
    };
  }

  @override
  Widget build(BuildContext context) => ToolPageShell(
        title: session.labelFor(Localizations.localeOf(context).languageCode),
        onBack: widget.onBack,
        escapeHint: session.query.text.isEmpty
            ? context.l10n.backEsc
            : context.l10n.clearSearch,
        onEscape: () {
          if (session.query.text.isNotEmpty) {
            session.query.clear();
            session.focus.requestFocus();
          } else {
            widget.onBack();
          }
        },
        presentation: widget.presentation,
        visible: widget.visible,
        editorFocus: session.focus,
        onKeyEvent: _key,
        notice: _notice(),
        header: Row(children: [
          Expanded(
              child: Column(
                  mainAxisAlignment: MainAxisAlignment.center,
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                Text(
                    session
                        .labelFor(Localizations.localeOf(context).languageCode),
                    style: AppStyle.caption.copyWith(
                        fontSize: DesignTokens.pageModeSize,
                        fontWeight: DesignTokens.pageModeWeight)),
                const SizedBox(height: 3),
                TextField(
                    key: const Key('app-history-search'),
                    controller: session.query,
                    focusNode: session.focus,
                    autofocus: widget.visible,
                    selectAllOnFocus: false,
                    textInputAction: TextInputAction.none,
                    onEditingComplete: () {},
                    style: AppStyle.body
                        .copyWith(fontSize: DesignTokens.pageHeadingSize),
                    decoration: InputDecoration(
                        hintText: context.l10n.appHistorySearch,
                        hintStyle: AppStyle.body.copyWith(
                            fontSize: DesignTokens.pageHeadingSize,
                            color: AppStyle.muted),
                        isCollapsed: true,
                        isDense: true,
                        filled: false,
                        border: InputBorder.none,
                        enabledBorder: InputBorder.none,
                        focusedBorder: InputBorder.none)),
              ])),
          if (session.items.isEmpty)
            _actionMenu(AppMoreAction(onPressed: _actions)),
        ]),
        child: Column(children: [
          if (session.loading) const LinearProgressIndicator(minHeight: 1),
          Expanded(
              child: session.items.isEmpty
                  ? Center(
                      child: Padding(
                          padding: const EdgeInsets.all(24),
                          child:
                              Column(mainAxisSize: MainAxisSize.min, children: [
                            const Icon(AppIcons.history,
                                size: 30, color: DesignTokens.iconPurple),
                            const SizedBox(height: 12),
                            Text(
                                session.loading
                                    ? context.l10n.loading
                                    : session.notice ==
                                            AppHistoryNoticeDto.unconfigured
                                        ? context.l10n.appHistoryUnconfigured
                                        : context.l10n.noMatchingEntries,
                                style: AppStyle.body,
                                textAlign: TextAlign.center),
                          ])))
                  : ListView.builder(
                      controller: session.scroll,
                      itemExtent: 64,
                      itemCount: session.items.length,
                      padding: const EdgeInsets.symmetric(
                          horizontal: 10, vertical: 3),
                      itemBuilder: (context, index) => _row(index))),
        ]),
      );
  Widget _row(int index) {
    final item = session.items[index],
        selected = index == session.selectedIndex;
    return Padding(
        padding: const EdgeInsets.symmetric(vertical: 3),
        child: AppSelectionSurface(
            selected: selected,
            canRequestFocus: false,
            onTap: () {
              session.select(index);
              session.focus.requestFocus();
            },
            onDoubleTap: () {
              session.select(index);
              unawaited(session.useSelection());
            },
            onSecondaryTap: () {
              session.select(index);
              _actions();
            },
            child: Padding(
                padding: const EdgeInsets.symmetric(horizontal: 12),
                child: Row(children: [
                  Icon(
                      item.kind == 'directory'
                          ? AppIcons.folder
                          : item.kind == 'workspace'
                              ? AppIcons.code
                              : AppIcons.file,
                      size: 23,
                      color: item.kind == 'directory'
                          ? DesignTokens.iconBlue
                          : DesignTokens.iconPurple),
                  const SizedBox(width: 12),
                  Expanded(
                      child: Column(
                          mainAxisAlignment: MainAxisAlignment.center,
                          crossAxisAlignment: CrossAxisAlignment.start,
                          children: [
                        Text(item.title,
                            maxLines: 1,
                            overflow: TextOverflow.ellipsis,
                            style: AppStyle.body),
                        const SizedBox(height: 3),
                        Text(
                            selected && session.copied
                                ? context.l10n.copied
                                : item.value,
                            maxLines: 1,
                            overflow: TextOverflow.ellipsis,
                            style: AppStyle.caption),
                      ])),
                  if (selected)
                    _actionMenu(AppRowActions(
                        primaryLabel: item.canOpen
                            ? context.l10n.open
                            : context.l10n.copy,
                        onPrimary: session.interactive && !session.loading
                            ? () => unawaited(session.useSelection())
                            : null,
                        onActions: _actions)),
                ]))));
  }
}
