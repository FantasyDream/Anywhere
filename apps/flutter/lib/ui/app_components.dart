import 'package:flutter/material.dart';

import '../infrastructure/desktop_environment.dart';
import '../l10n/app_language.dart';
import 'app_icons.dart';
import 'app_style.dart';

/// Progressive disclosure for secondary information, with an ordinary keyboard
/// button. Trailing controls stay separate so expanding never toggles a setting.
class AppDisclosure extends StatefulWidget {
  const AppDisclosure(
      {super.key,
      required this.title,
      required this.child,
      this.leading,
      this.trailing,
      this.subtitle,
      this.compact = false,
      this.initiallyExpanded = false});
  final String title;
  final Widget child;
  final Widget? leading, trailing, subtitle;
  final bool initiallyExpanded, compact;
  @override
  State<AppDisclosure> createState() => _AppDisclosureState();
}

class _AppDisclosureState extends State<AppDisclosure> {
  late bool _expanded = widget.initiallyExpanded;
  bool _restored = false;
  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    if (_restored) return;
    _restored = true;
    if (widget.key != null) {
      final stored = PageStorage.maybeOf(context)
          ?.readState(context, identifier: widget.key);
      if (stored is bool) _expanded = stored;
    }
  }

  @override
  void didUpdateWidget(AppDisclosure oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (!oldWidget.initiallyExpanded && widget.initiallyExpanded) {
      _expanded = true;
    }
  }

  void _toggle() {
    setState(() => _expanded = !_expanded);
    if (widget.key != null) {
      PageStorage.maybeOf(context)
          ?.writeState(context, _expanded, identifier: widget.key);
    }
  }

  @override
  Widget build(BuildContext context) =>
      Column(crossAxisAlignment: CrossAxisAlignment.stretch, children: [
        Row(children: [
          Expanded(
              child: Semantics(
                  expanded: _expanded,
                  child: TextButton(
                      onPressed: _toggle,
                      style: TextButton.styleFrom(
                          alignment: Alignment.centerLeft,
                          padding: const EdgeInsets.symmetric(
                              horizontal: 12, vertical: 10)),
                      child: Row(children: [
                        if (widget.leading != null) ...[
                          widget.leading!,
                          const SizedBox(width: 10)
                        ],
                        Expanded(
                            child: Column(
                                crossAxisAlignment: CrossAxisAlignment.start,
                                children: [
                              Text(widget.title,
                                  style: widget.compact
                                      ? AppStyle.caption
                                          .copyWith(color: AppStyle.secondary)
                                      : AppStyle.body,
                                  maxLines: 2,
                                  overflow: TextOverflow.ellipsis),
                              if (widget.subtitle != null) ...[
                                const SizedBox(height: 3),
                                widget.subtitle!
                              ]
                            ])),
                        const SizedBox(width: 8),
                        Icon(
                            _expanded ? AppIcons.chevronDown : AppIcons.chevron,
                            size: 12,
                            color: AppStyle.secondary),
                      ])))),
          if (widget.trailing != null)
            Padding(
                padding: const EdgeInsets.only(right: 12),
                child: widget.trailing!),
        ]),
        if (_expanded)
          Padding(
              padding: const EdgeInsets.fromLTRB(12, 0, 12, 12),
              child: widget.child),
      ]);
}

/// One transparent selection treatment for search, menus, history and tasks.
/// Hover never adds a second wash over the keyboard-selected row.
class AppSelectionSurface extends StatelessWidget {
  const AppSelectionSurface(
      {super.key,
      required this.selected,
      required this.child,
      this.onTap,
      this.onTapDown,
      this.onDoubleTap,
      this.onSecondaryTap,
      this.canRequestFocus = true});
  final bool selected;
  final Widget child;
  final VoidCallback? onTap;
  final GestureTapDownCallback? onTapDown;
  final VoidCallback? onDoubleTap;
  final VoidCallback? onSecondaryTap;
  final bool canRequestFocus;
  @override
  Widget build(BuildContext context) => Material(
      color: selected ? AppStyle.selectedSurface : Colors.transparent,
      shape: RoundedRectangleBorder(
          borderRadius: BorderRadius.circular(AppStyle.resultRadius),
          side: BorderSide(
              color: selected ? AppStyle.selectionOutline : Colors.transparent,
              width: DesignTokens.dividerWidth)),
      child: InkWell(
          borderRadius: BorderRadius.circular(AppStyle.resultRadius),
          hoverColor: selected ? Colors.transparent : AppStyle.hover,
          onTap: onTap,
          onTapDown: onTapDown,
          onDoubleTap: onDoubleTap,
          onSecondaryTap: onSecondaryTap,
          canRequestFocus: canRequestFocus,
          child: child));
}

/// Shared Launcher and tool-page geometry; individual pages supply the content.
class AppPageHeader extends StatelessWidget {
  const AppPageHeader(
      {super.key,
      required this.child,
      this.height = DesignTokens.pageHeaderHeight});
  final Widget child;
  final double height;
  @override
  Widget build(BuildContext context) => SizedBox(
      height: height,
      child: Padding(
          padding: const EdgeInsets.symmetric(horizontal: 20), child: child));
}

class AppPageFooter extends StatelessWidget {
  const AppPageFooter({super.key, required this.child});
  final Widget child;
  @override
  Widget build(BuildContext context) => SizedBox(
      height: DesignTokens.pageFooterHeight,
      child: Column(children: [
        const Divider(height: 1),
        Expanded(
            child: Padding(
                padding: const EdgeInsets.fromLTRB(20, 3, 24, 10),
                child: child)),
      ]));
}

/// More commands expose their shortcut without requiring a hover or a footer.
class AppMoreAction extends StatelessWidget {
  const AppMoreAction({super.key, required this.onPressed});
  final VoidCallback? onPressed;

  @override
  Widget build(BuildContext context) => Tooltip(
      message: context.l10n.shortcutHint(
          context.l10n.actions, DesktopEnvironment.current.commandLabel('K')),
      child: TextButton(
          onPressed: onPressed,
          child: Row(mainAxisSize: MainAxisSize.min, children: [
            const Icon(AppIcons.more, size: 14),
            const SizedBox(width: 4),
            Text(DesktopEnvironment.current.commandLabel('K')),
          ])));
}

/// Commands belong to the selected object, so they travel with its row.
/// Parent rows still select on a single click; these targets explicitly execute.
class AppRowActions extends StatelessWidget {
  const AppRowActions({
    super.key,
    required this.primaryLabel,
    required this.onPrimary,
    this.onActions,
  });
  final String primaryLabel;
  final VoidCallback? onPrimary, onActions;

  @override
  Widget build(BuildContext context) => ExcludeFocus(
          child: Row(mainAxisSize: MainAxisSize.min, children: [
        if (onActions != null)
          AppMoreAction(
              key: const Key('selected-row-actions'), onPressed: onActions),
        Tooltip(
            message: context.l10n.shortcutHint(primaryLabel, '↵'),
            child: TextButton(
                key: const Key('selected-row-primary'),
                onPressed: onPrimary,
                child: Row(mainAxisSize: MainAxisSize.min, children: [
                  Text(primaryLabel),
                  if (onActions == null) ...[
                    const SizedBox(width: 5),
                    const Icon(AppIcons.enter, size: 13),
                  ],
                ]))),
      ]));
}

/// A quiet macOS-style toolbar: title on the leading side, commands trailing.
/// Native windows reserve their own traffic-light inset above this component.
class AppToolbar extends StatelessWidget {
  const AppToolbar(
      {super.key, required this.title, this.subtitle, this.actions = const []});
  final String title;
  final String? subtitle;
  final List<Widget> actions;
  @override
  Widget build(BuildContext context) => Padding(
      padding: const EdgeInsets.symmetric(horizontal: 18, vertical: 10),
      child: Row(children: [
        Expanded(
            child:
                Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
          Text(title,
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
              style: AppStyle.body.copyWith(fontWeight: FontWeight.w600)),
          if (subtitle != null) ...[
            const SizedBox(height: 2),
            Text(subtitle!,
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: AppStyle.caption)
          ],
        ])),
        const SizedBox(width: 10),
        ...actions,
      ]));
}

class AppSidebarItem extends StatelessWidget {
  const AppSidebarItem(
      {super.key,
      required this.label,
      required this.icon,
      required this.selected,
      required this.onPressed,
      this.iconColor});
  final String label;
  final IconData icon;
  final bool selected;
  final VoidCallback onPressed;
  final Color? iconColor;
  @override
  Widget build(BuildContext context) => Semantics(
      selected: selected,
      child: TextButton(
          onPressed: onPressed,
          style: TextButton.styleFrom(
              minimumSize: const Size(0, 32),
              alignment: Alignment.centerLeft,
              padding: const EdgeInsets.symmetric(horizontal: 8),
              shape: RoundedRectangleBorder(
                  borderRadius: BorderRadius.circular(AppStyle.controlRadius),
                  side: BorderSide(
                      color: selected
                          ? AppStyle.selectionOutline
                          : Colors.transparent,
                      width: .5)),
              backgroundColor:
                  selected ? AppStyle.selectedSurface : Colors.transparent,
              foregroundColor: AppStyle.primary,
              textStyle: TextStyle(
                  fontSize: DesignTokens.bodySize,
                  fontWeight: selected ? FontWeight.w500 : FontWeight.w400)),
          child: Row(children: [
            SizedBox(
                width: 20,
                height: 20,
                child: Icon(icon,
                    size: 18,
                    color: iconColor == null
                        ? AppStyle.secondary
                        : Color.lerp(iconColor, Colors.white, .35))),
            const SizedBox(width: 8),
            Expanded(
                child:
                    Text(label, maxLines: 1, overflow: TextOverflow.ellipsis)),
          ])));
}

class AppCard extends StatelessWidget {
  const AppCard({super.key, required this.child});
  final Widget child;
  @override
  Widget build(BuildContext context) => Material(
      color: AppStyle.card,
      shape: RoundedRectangleBorder(
          borderRadius: BorderRadius.circular(AppStyle.cardRadius),
          side: const BorderSide(color: AppStyle.divider, width: .8)),
      clipBehavior: Clip.antiAlias,
      child: child);
}

class AppIconPlate extends StatelessWidget {
  const AppIconPlate(this.icon,
      {super.key, this.accent = false, this.size = 32});
  final IconData icon;
  final bool accent;
  final double size;
  @override
  Widget build(BuildContext context) => SizedBox(
      width: size,
      height: size,
      child: Icon(icon,
          size: size * .76,
          color: accent ? AppStyle.primary : AppStyle.secondary,
          shadows: AppStyle.textShadows));
}

class AppSectionHeading extends StatelessWidget {
  const AppSectionHeading(this.title, {super.key, required this.icon});
  final String title;
  final IconData icon;
  @override
  Widget build(BuildContext context) => Padding(
      padding: const EdgeInsets.only(bottom: 10),
      child: Row(children: [
        Icon(icon, size: 14, color: AppStyle.secondary),
        const SizedBox(width: 8),
        Text(title,
            style: const TextStyle(
                fontSize: 12,
                fontWeight: FontWeight.w600,
                color: AppStyle.secondary)),
      ]));
}

enum NoticeKind { info, warning, error, success }

class AppNotice extends StatelessWidget {
  const AppNotice(this.message,
      {super.key,
      this.kind = NoticeKind.warning,
      this.onDismiss,
      this.compact = false,
      this.trailing});
  final String message;
  final NoticeKind kind;
  final bool compact;
  final Widget? trailing;
  final VoidCallback? onDismiss;
  @override
  Widget build(BuildContext context) {
    final color = switch (kind) {
      NoticeKind.info => AppStyle.accent,
      NoticeKind.warning => AppStyle.warning,
      NoticeKind.error => AppStyle.danger,
      NoticeKind.success => AppStyle.success,
    };
    return Semantics(
        liveRegion: true,
        child: Container(
            padding:
                EdgeInsets.symmetric(horizontal: 10, vertical: compact ? 3 : 8),
            decoration: BoxDecoration(
                color: AppStyle.card,
                borderRadius: BorderRadius.circular(AppStyle.controlRadius),
                border: Border.all(
                    color: color.withValues(alpha: .24),
                    width: DesignTokens.controlBorderWidth)),
            child: Row(
                crossAxisAlignment: compact
                    ? CrossAxisAlignment.center
                    : CrossAxisAlignment.start,
                children: [
                  Icon(
                      switch (kind) {
                        NoticeKind.success => AppIcons.success,
                        NoticeKind.error => AppIcons.error,
                        NoticeKind.warning => AppIcons.warning,
                        _ => AppIcons.info,
                      },
                      size: 16,
                      color: color),
                  const SizedBox(width: 9),
                  Expanded(
                      child: Text(message,
                          maxLines: compact ? 2 : null,
                          overflow: compact ? TextOverflow.ellipsis : null,
                          style: TextStyle(
                              fontSize: compact ? 11 : 12,
                              height: 1.2,
                              color: AppStyle.primary))),
                  if (trailing != null) trailing!,
                  if (onDismiss != null)
                    IconButton(
                        tooltip: context.l10n.dismissMessage,
                        onPressed: onDismiss,
                        icon: Icon(AppIcons.close, size: 16, color: color)),
                ])));
  }
}

class AppEmptyState extends StatelessWidget {
  const AppEmptyState(
      {super.key,
      required this.icon,
      required this.title,
      required this.message,
      this.action});
  final IconData icon;
  final String title, message;
  final Widget? action;
  @override
  Widget build(BuildContext context) => Center(
      child: Padding(
          padding: const EdgeInsets.all(24),
          child: Column(mainAxisSize: MainAxisSize.min, children: [
            AppIconPlate(icon, size: 42),
            const SizedBox(height: 16),
            Text(title,
                style: AppStyle.heading.copyWith(fontSize: 15),
                textAlign: TextAlign.center),
            const SizedBox(height: 8),
            Text(message,
                style: AppStyle.caption.copyWith(fontSize: 12),
                textAlign: TextAlign.center),
            if (action != null) ...[const SizedBox(height: 14), action!],
          ])));
}

class AppKeycap extends StatelessWidget {
  const AppKeycap(this.label, {super.key});
  final String label;
  @override
  Widget build(BuildContext context) => Container(
      constraints: const BoxConstraints(minWidth: DesignTokens.keycapMinWidth),
      padding: const EdgeInsets.symmetric(
          horizontal: DesignTokens.keycapPaddingX,
          vertical: DesignTokens.keycapPaddingY),
      decoration: BoxDecoration(
          color: AppStyle.keycapSurface,
          borderRadius: BorderRadius.circular(DesignTokens.keycapRadius),
          border: Border.all(
              color: AppStyle.keycapBorder, width: DesignTokens.dividerWidth)),
      child: label == '↵'
          ? Semantics(
              label: context.l10n.returnKey,
              child: const Icon(AppIcons.enter,
                  size: 12, color: AppStyle.secondary))
          : Text(label,
              textAlign: TextAlign.center,
              style: const TextStyle(
                  fontSize: DesignTokens.keycapFontSize,
                  height: DesignTokens.keycapLineHeight,
                  fontWeight: DesignTokens.keycapWeight,
                  color: AppStyle.secondary)));
}

/// Startup failures use the same surface and typography as regular windows.
enum AppFailureKind { launcher, tasks, clipboardShelf }

class AppFailureView extends StatelessWidget {
  const AppFailureView({super.key, this.title, this.kind, required this.detail})
      : assert(title != null || kind != null);
  final String? title;
  final AppFailureKind? kind;
  final String detail;
  @override
  Widget build(BuildContext context) => Scaffold(
      body: Material(
          color: AppStyle.surface,
          child: Padding(
              padding: const EdgeInsets.all(20),
              child: Column(
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  children: [
                    Row(children: [
                      const AppIconPlate(AppIcons.error),
                      const SizedBox(width: 12),
                      Expanded(
                          child: Text(
                              switch (kind) {
                                AppFailureKind.launcher =>
                                  context.l10n.launcherStartupFailed,
                                AppFailureKind.tasks =>
                                  context.l10n.tasksStartupFailed,
                                AppFailureKind.clipboardShelf =>
                                  context.l10n.shelfStartupFailed,
                                null => title!,
                              },
                              style: AppStyle.heading.copyWith(fontSize: 15))),
                    ]),
                    const SizedBox(height: 14),
                    Expanded(
                        child: SingleChildScrollView(
                            child: SelectableText(detail,
                                style: AppStyle.body.copyWith(
                                    fontSize: 12, color: AppStyle.secondary)))),
                  ]))));
}

/// A passive status label, distinct from outlined actionable controls.
class AppStatusBadge extends StatelessWidget {
  const AppStatusBadge(this.label,
      {super.key, required this.color, required this.icon});
  final String label;
  final Color color;
  final IconData icon;
  @override
  Widget build(BuildContext context) => Container(
      padding: const EdgeInsets.symmetric(horizontal: 7, vertical: 3),
      decoration: BoxDecoration(
          color: Colors.transparent, borderRadius: BorderRadius.circular(6)),
      child: Row(mainAxisSize: MainAxisSize.min, children: [
        Icon(icon, size: 12, color: color),
        const SizedBox(width: 5),
        Text(label, style: TextStyle(fontSize: 11, height: 1.2, color: color)),
      ]));
}

/// A compact switch with a larger hit area, semantics and button keyboard
/// activation. Its geometry stays stable at different text sizes.
class AppToggle extends StatelessWidget {
  const AppToggle(
      {super.key,
      required this.label,
      required this.value,
      required this.onChanged});
  final String label;
  final bool value;
  final ValueChanged<bool>? onChanged;

  @override
  Widget build(BuildContext context) => Semantics(
      container: true,
      label: label,
      toggled: value,
      enabled: onChanged != null,
      child: TextButton(
          onPressed: onChanged == null ? null : () => onChanged!(!value),
          style: TextButton.styleFrom(
              minimumSize: const Size(40, 30),
              padding: const EdgeInsets.all(5),
              backgroundColor: Colors.transparent),
          child: Opacity(
              opacity: onChanged == null ? .45 : 1,
              child: Container(
                  width: 30,
                  height: 18,
                  padding: const EdgeInsets.all(2),
                  decoration: BoxDecoration(
                      color: value ? AppStyle.accentFill : AppStyle.elevated,
                      borderRadius: BorderRadius.circular(9),
                      border: Border.all(
                          color: value ? AppStyle.accentFill : AppStyle.border,
                          width: .6)),
                  child: Align(
                      alignment:
                          value ? Alignment.centerRight : Alignment.centerLeft,
                      child: Container(
                          width: 14,
                          height: 14,
                          decoration: const BoxDecoration(
                              color: Colors.white,
                              shape: BoxShape.circle)))))));
}
