import 'package:flutter/material.dart';
import '../ui/app_style.dart';
import '../ui/app_icons.dart';

/// Desktop-sized controls shared by settings and its sheets. Rows have no
/// interactive background; hover and focus feedback belong to the control.
abstract final class SettingsStyle {
  static const window = AppStyle.window;

  static ThemeData theme(ThemeData base) {
    const density = VisualDensity.standard;
    const size = Size(0, 24);
    const padding = EdgeInsets.symmetric(horizontal: 9, vertical: 3);
    const text = TextStyle(fontSize: 11, fontWeight: FontWeight.w400);
    const compact = ButtonStyle(
        minimumSize: WidgetStatePropertyAll(size),
        padding: WidgetStatePropertyAll(padding),
        textStyle: WidgetStatePropertyAll(text),
        tapTargetSize: MaterialTapTargetSize.shrinkWrap,
        visualDensity: density);
    return base.copyWith(
      visualDensity: density,
      textButtonTheme: TextButtonThemeData(
          style: base.textButtonTheme.style?.merge(compact)),
      outlinedButtonTheme: OutlinedButtonThemeData(
          style: base.outlinedButtonTheme.style?.merge(compact)),
      filledButtonTheme: FilledButtonThemeData(
          style: base.filledButtonTheme.style?.merge(compact)),
      iconButtonTheme: IconButtonThemeData(
          style: base.iconButtonTheme.style?.merge(const ButtonStyle(
              minimumSize: WidgetStatePropertyAll(Size(26, 26)),
              padding: WidgetStatePropertyAll(EdgeInsets.all(5)),
              tapTargetSize: MaterialTapTargetSize.shrinkWrap))),
      listTileTheme: base.listTileTheme
          .copyWith(dense: true, minVerticalPadding: 6, minTileHeight: 40),
      inputDecorationTheme: base.inputDecorationTheme.copyWith(
          hoverColor: Colors.transparent,
          contentPadding:
              const EdgeInsets.symmetric(horizontal: 9, vertical: 7)),
    );
  }
}

/// A compact macOS-style preference row using the shared menu palette.
class SettingsChoiceRow<T> extends StatelessWidget {
  const SettingsChoiceRow(
      {super.key,
      required this.controlKey,
      required this.title,
      required this.description,
      required this.value,
      required this.choices,
      required this.onChanged});
  final Key controlKey;
  final String title, description;
  final T value;
  final Map<T, String> choices;
  final ValueChanged<T> onChanged;
  @override
  Widget build(BuildContext context) => Padding(
      padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 10),
      child: Row(children: [
        Expanded(
            child:
                Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
          Text(title,
              style: const TextStyle(fontSize: 13, color: AppStyle.primary)),
          const SizedBox(height: 2),
          Text(description, style: AppStyle.caption),
        ])),
        const SizedBox(width: 12),
        PopupMenuButton<T>(
            key: controlKey,
            tooltip: title,
            onSelected: onChanged,
            itemBuilder: (_) => [
                  for (final choice in choices.entries)
                    PopupMenuItem<T>(
                        value: choice.key,
                        height: 32,
                        child: Text(choice.value,
                            style: AppStyle.body.copyWith(fontSize: 12)))
                ],
            child: Padding(
                padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 6),
                child: Row(mainAxisSize: MainAxisSize.min, children: [
                  Text(choices[value] ?? '',
                      style: AppStyle.body.copyWith(fontSize: 12)),
                  const SizedBox(width: 4),
                  const Icon(AppIcons.chevronDown,
                      size: 14, color: AppStyle.secondary),
                ]))),
      ]));
}

/// A compact switch with a larger hit area, semantics and button keyboard
/// activation. Its geometry stays stable at different text sizes.
class SettingsToggle extends StatelessWidget {
  const SettingsToggle(
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

class SettingsToggleRow extends StatelessWidget {
  const SettingsToggleRow(
      {super.key,
      required this.title,
      required this.description,
      required this.value,
      required this.onChanged,
      this.icon,
      this.status});
  final String title, description;
  final bool value;
  final ValueChanged<bool>? onChanged;
  final IconData? icon;
  final Widget? status;

  @override
  Widget build(BuildContext context) => Padding(
      padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 10),
      child: Row(crossAxisAlignment: CrossAxisAlignment.start, children: [
        if (icon != null) ...[
          Padding(
              padding: const EdgeInsets.only(top: 3),
              child: Icon(icon, size: 17, color: AppStyle.secondary)),
          const SizedBox(width: 10),
        ],
        Expanded(
            child:
                Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
          GestureDetector(
              excludeFromSemantics: true,
              behavior: HitTestBehavior.opaque,
              onTap: onChanged == null ? null : () => onChanged!(!value),
              child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(title,
                        style: const TextStyle(
                            fontSize: 13,
                            fontWeight: FontWeight.w400,
                            color: AppStyle.primary)),
                    const SizedBox(height: 2),
                    Text(description, style: AppStyle.caption),
                  ])),
          if (status != null) ...[const SizedBox(height: 5), status!],
        ])),
        const SizedBox(width: 10),
        SettingsToggle(label: title, value: value, onChanged: onChanged),
      ]));
}
