import '../l10n/app_language.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import '../ui/app_components.dart';
import '../ui/app_style.dart';
import '../ui/app_icons.dart';

/// Shared frame and keyboard / focus rules. Bodies remain ordinary Flutter widgets.
class ToolPageShell extends StatefulWidget {
  const ToolPageShell(
      {super.key,
      required this.title,
      required this.child,
      required this.onBack,
      this.subtitle,
      this.notice,
      this.header,
      this.leading,
      this.headerHeight = DesignTokens.pageHeaderHeight,
      this.footer,
      this.scrollController,
      this.editorFocus,
      this.presentation = 0,
      this.visible = true,
      this.onKeyEvent,
      this.escapeHint,
      this.detached = false});
  final String title;
  final String? subtitle;
  final String? notice;
  final Widget child;
  final Widget? header;
  final Widget? leading;
  final double headerHeight;
  final Widget? footer;
  final VoidCallback onBack;
  final ScrollController? scrollController;
  final FocusNode? editorFocus;
  final int presentation;
  final bool visible;
  final bool detached;
  final KeyEventResult Function(KeyEvent)? onKeyEvent;
  final String? escapeHint;
  @override
  State<ToolPageShell> createState() => _ToolPageShellState();
}

class _ToolPageShellState extends State<ToolPageShell> {
  final _focus = FocusNode(debugLabel: 'tool-page');
  int _request = 0;
  @override
  void initState() {
    super.initState();
    _requestFocus();
  }

  @override
  void didUpdateWidget(ToolPageShell old) {
    super.didUpdateWidget(old);
    if (widget.visible &&
        (!old.visible ||
            old.presentation != widget.presentation ||
            old.editorFocus != widget.editorFocus)) {
      _requestFocus();
    }
  }

  void _requestFocus() {
    final request = ++_request;
    if (!widget.visible) return;
    // A native presentation can leave the old platform input client attached
    // even though this editor still owns Flutter focus. End that connection
    // before reconnecting after layout; never interrupt an active composition.
    final active = widget.editorFocus;
    if (active?.hasFocus == true) {
      final editor =
          active!.context?.findAncestorStateOfType<EditableTextState>();
      if (editor != null &&
          !editor.widget.controller.value.composing.isCollapsed) {
        return;
      }
      active.unfocus();
    }
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!mounted || !widget.visible || request != _request) return;
      // Unfocus is deferred by FocusManager. Flush it before requestKeyboard,
      // otherwise the editor sees the old focus and only re-shows a stale client.
      FocusManager.instance.applyFocusChangesIfNeeded();
      final focus = widget.editorFocus ?? _focus;
      final editor =
          focus.context?.findAncestorStateOfType<EditableTextState>();
      if (editor != null) {
        editor.requestKeyboard();
      } else {
        focus.requestFocus();
      }
    });
  }

  @override
  void dispose() {
    ++_request;
    _focus.dispose();
    super.dispose();
  }

  KeyEventResult _key(FocusNode _, KeyEvent event) {
    if (!widget.visible ||
        (event is! KeyDownEvent && event is! KeyRepeatEvent)) {
      return KeyEventResult.ignored;
    }
    final editor = FocusManager.instance.primaryFocus?.context
        ?.findAncestorStateOfType<EditableTextState>();
    if (editor != null &&
        !editor.widget.controller.value.composing.isCollapsed) {
      return KeyEventResult.ignored;
    }
    if (event.logicalKey == LogicalKeyboardKey.escape) {
      widget.onBack();
      return KeyEventResult.handled;
    }
    final keyboard = HardwareKeyboard.instance;
    if (event.logicalKey == LogicalKeyboardKey.tab &&
        !keyboard.isControlPressed &&
        !keyboard.isAltPressed &&
        !keyboard.isMetaPressed) {
      return KeyEventResult.handled;
    }
    final handled = widget.onKeyEvent?.call(event);
    if (handled != null && handled != KeyEventResult.ignored) return handled;
    if (editor != null ||
        keyboard.isShiftPressed ||
        keyboard.isControlPressed ||
        keyboard.isAltPressed ||
        keyboard.isMetaPressed) {
      return KeyEventResult.ignored;
    }
    final distance = switch (event.logicalKey) {
      LogicalKeyboardKey.arrowDown => 40.0,
      LogicalKeyboardKey.arrowUp => -40.0,
      LogicalKeyboardKey.pageDown => 240.0,
      LogicalKeyboardKey.pageUp => -240.0,
      _ => null,
    };
    final scroll = widget.scrollController;
    if (distance == null || scroll == null || !scroll.hasClients) {
      return KeyEventResult.ignored;
    }
    scroll.jumpTo(
        (scroll.offset + distance).clamp(0, scroll.position.maxScrollExtent));
    return KeyEventResult.handled;
  }

  @override
  Widget build(BuildContext context) => Focus(
        // This node only catches page shortcuts. Keep the editor and the page
        // controls as distinct accessibility nodes instead of merging the page
        // into an additional focusable container.
        includeSemantics: false,
        autofocus: widget.visible && widget.editorFocus == null,
        focusNode: _focus,
        onKeyEvent: _key,
        child:
            Column(crossAxisAlignment: CrossAxisAlignment.stretch, children: [
          AppPageHeader(
              height: widget.headerHeight,
              child: Row(children: [
                widget.leading ??
                    IconButton(
                        tooltip: widget.escapeHint ??
                            (widget.detached
                                ? context.l10n.closeEsc
                                : context.l10n.backEsc),
                        onPressed: widget.visible ? widget.onBack : null,
                        icon: Icon(
                            widget.detached ? AppIcons.close : AppIcons.back,
                            size: 20)),
                const SizedBox(width: 12),
                Expanded(
                    child: widget.header ??
                        Column(
                            mainAxisAlignment: MainAxisAlignment.center,
                            crossAxisAlignment: CrossAxisAlignment.start,
                            children: [
                              if (widget.subtitle != null) ...[
                                Text(widget.subtitle!,
                                    style: AppStyle.caption.copyWith(
                                        fontSize: DesignTokens.pageModeSize,
                                        fontWeight:
                                            DesignTokens.pageModeWeight)),
                                const SizedBox(height: 3),
                              ],
                              Text(widget.title,
                                  maxLines: 1,
                                  overflow: TextOverflow.ellipsis,
                                  style: AppStyle.body.copyWith(
                                      fontSize: DesignTokens.pageHeadingSize)),
                            ])),
                const SizedBox(width: 14),
                Tooltip(
                    message: widget.escapeHint ??
                        (widget.detached
                            ? context.l10n.closeEsc
                            : context.l10n.backEsc),
                    child: InkWell(
                        onTap: widget.visible ? widget.onBack : null,
                        borderRadius:
                            BorderRadius.circular(DesignTokens.keycapRadius),
                        child: const AppKeycap('esc'))),
              ])),
          const Divider(height: 1, indent: 20, endIndent: 20),
          if (widget.notice != null)
            Padding(
                padding: const EdgeInsets.fromLTRB(24, 12, 24, 0),
                child: AppNotice(widget.notice!)),
          Expanded(child: widget.child),
          if (widget.footer != null) AppPageFooter(child: widget.footer!),
        ]),
      );
}
