import 'app_icons.dart';
import 'package:flutter/material.dart';
import 'app_style.dart';

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
                  borderRadius: BorderRadius.circular(6),
                  side: BorderSide(
                      color: selected
                          ? AppStyle.selectionOutline
                          : Colors.transparent,
                      width: .5)),
              backgroundColor:
                  selected ? AppStyle.selectedSurface : Colors.transparent,
              foregroundColor: AppStyle.primary,
              textStyle:
                  const TextStyle(fontSize: 13, fontWeight: FontWeight.w400)),
          child: Row(children: [
            SizedBox(
                width: 20,
                height: 20,
                child: Icon(icon,
                    size: 16,
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
          size: size * .62,
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
      {super.key, this.kind = NoticeKind.warning, this.onDismiss});
  final String message;
  final NoticeKind kind;
  final VoidCallback? onDismiss;
  @override
  Widget build(BuildContext context) {
    final color = switch (kind) {
      NoticeKind.info => AppStyle.accent,
      NoticeKind.warning => AppStyle.warning,
      NoticeKind.error => AppStyle.danger,
      NoticeKind.success => AppStyle.success,
    };
    return Container(
        padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 8),
        decoration: BoxDecoration(
            color: AppStyle.card,
            borderRadius: BorderRadius.circular(AppStyle.controlRadius),
            border: Border.all(color: AppStyle.divider, width: .8)),
        child: Row(crossAxisAlignment: CrossAxisAlignment.start, children: [
          Icon(kind == NoticeKind.success ? AppIcons.success : AppIcons.info,
              size: 16, color: color),
          const SizedBox(width: 9),
          Expanded(
              child: Text(message,
                  style: TextStyle(
                      fontSize: 12, height: 1.3, color: AppStyle.secondary))),
          if (onDismiss != null)
            IconButton(
                tooltip: 'Dismiss message',
                onPressed: onDismiss,
                icon: Icon(AppIcons.close, size: 16, color: color)),
        ]));
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
      constraints: const BoxConstraints(minWidth: 20),
      padding: const EdgeInsets.symmetric(horizontal: 5, vertical: 3),
      decoration: BoxDecoration(
          color: AppStyle.keycapSurface,
          borderRadius: BorderRadius.circular(5),
          border: Border.all(color: AppStyle.keycapBorder, width: .5)),
      child: label == '↵'
          ? Semantics(
              label: 'Return',
              child: const Icon(AppIcons.enter,
                  size: 12, color: AppStyle.secondary))
          : Text(label,
              textAlign: TextAlign.center,
              style: const TextStyle(
                  fontSize: 10.5,
                  height: 1.1,
                  fontWeight: FontWeight.w500,
                  color: AppStyle.secondary)));
}

/// Startup failures use the same surface and typography as regular windows.
class AppFailureView extends StatelessWidget {
  const AppFailureView({super.key, required this.title, required this.detail});
  final String title, detail;
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
                          child: Text(title,
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
