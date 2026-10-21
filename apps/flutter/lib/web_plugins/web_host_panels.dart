import 'package:flutter/material.dart';
import '../ui/app_style.dart';
import '../ui/app_components.dart';
import '../ui/app_icons.dart';

/// Host chrome only: these widgets never dispatch RPCs or grant capabilities.
class WebActionsPanel extends StatelessWidget {
  const WebActionsPanel(
      {super.key, required this.selected, required this.onSelect});
  final int selected;
  final ValueChanged<int> onSelect;
  @override
  Widget build(BuildContext context) => Align(
      alignment: Alignment.bottomRight,
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
            _action(0, AppIcons.refresh, 'Reload tool'),
            const SizedBox(height: 4),
            _action(1, AppIcons.back, 'Back to search · Esc'),
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

class WebConsentPanel extends StatelessWidget {
  const WebConsentPanel(
      {super.key,
      required this.title,
      required this.toolTitle,
      required this.allowSelected,
      required this.onAnswer,
      this.preview});
  final String title, toolTitle;
  final String? preview;
  final bool allowSelected;
  final ValueChanged<bool> onAnswer;
  @override
  Widget build(BuildContext context) => Positioned.fill(
      child: ColoredBox(
          color: Colors.black54,
          child: Center(
              child: Container(
                  width: 420,
                  padding: const EdgeInsets.all(24),
                  decoration: BoxDecoration(
                      color: AppStyle.overlay,
                      borderRadius:
                          BorderRadius.circular(DesignTokens.cardRadius),
                      border: Border.all(
                          color: AppStyle.controlBorder,
                          width: DesignTokens.dividerWidth)),
                  child: SingleChildScrollView(
                      child: Column(
                          mainAxisSize: MainAxisSize.min,
                          crossAxisAlignment: CrossAxisAlignment.start,
                          children: [
                        Text(title, style: AppStyle.heading),
                        const SizedBox(height: 12),
                        Text(
                            '$toolTitle is requesting a clipboard operation. Approval applies once.',
                            style: AppStyle.caption),
                        if (preview != null) ...[
                          const SizedBox(height: 12),
                          Text(preview!,
                              maxLines: 4, overflow: TextOverflow.ellipsis),
                        ],
                        const SizedBox(height: 20),
                        Wrap(
                            alignment: WrapAlignment.end,
                            spacing: 12,
                            runSpacing: 8,
                            children: [
                              TextButton(
                                  onPressed: () => onAnswer(false),
                                  child: Text(allowSelected
                                      ? 'Cancel · Esc'
                                      : '✓ Cancel · Esc')),
                              FilledButton(
                                  onPressed: () => onAnswer(true),
                                  child: Text(allowSelected
                                      ? '✓ Allow once'
                                      : 'Allow once')),
                            ])
                      ]))))));
}
