import 'package:flutter/material.dart';

import '../l10n/app_language.dart';
import '../ui/app_components.dart';
import '../ui/app_icons.dart';
import '../ui/app_style.dart';

/// Host chrome only: these widgets never dispatch RPCs or grant capabilities.
class WebActionsPanel extends StatelessWidget {
  const WebActionsPanel(
      {super.key, required this.selected, required this.onSelect});
  final int selected;
  final ValueChanged<int> onSelect;
  @override
  Widget build(BuildContext context) => Align(
      alignment: Alignment.topRight,
      child: Container(
          width: 230,
          margin: const EdgeInsets.all(16),
          padding: const EdgeInsets.all(6),
          decoration: BoxDecoration(
              color: AppStyle.overlay,
              borderRadius: BorderRadius.circular(DesignTokens.cardRadius),
              border: Border.all(
                  color: AppStyle.controlBorder,
                  width: DesignTokens.dividerWidth)),
          child: Column(mainAxisSize: MainAxisSize.min, children: [
            _action(0, AppIcons.refresh, context.l10n.reloadTool),
            const SizedBox(height: 4),
            _action(1, AppIcons.back, context.l10n.backToSearchEsc),
          ])));

  Widget _action(int index, IconData icon, String title) => Semantics(
      button: true,
      selected: selected == index,
      child: AppSelectionSurface(
          selected: selected == index,
          onTap: () => onSelect(index),
          child: Padding(
              padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 9),
              child: Row(children: [
                Icon(icon,
                    size: DesignTokens.iconSize, color: AppStyle.secondary),
                const SizedBox(width: 10),
                Expanded(child: Text(title, style: AppStyle.body)),
              ]))));
}
