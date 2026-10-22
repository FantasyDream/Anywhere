import 'package:flutter/material.dart';
import '../ui/app_style.dart';
import '../ui/app_components.dart';
export '../ui/app_components.dart' show AppToggle;

/// Desktop-sized controls shared by settings and its sheets. Rows have no
/// interactive background; hover and focus feedback belong to the control.
abstract final class SettingsStyle {
  static const window = AppStyle.window;

  static ThemeData theme(ThemeData base) => base.copyWith(
      listTileTheme: base.listTileTheme
          .copyWith(dense: true, minVerticalPadding: 6, minTileHeight: 40));
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
        AppToggle(label: title, value: value, onChanged: onChanged),
      ]));
}
