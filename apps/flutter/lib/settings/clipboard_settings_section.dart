import 'package:flutter/material.dart';
import '../ui/app_components.dart';
import '../ui/app_icons.dart';
import '../ui/app_style.dart';
import 'clipboard_settings_controller.dart';
import 'settings_components.dart';

class ClipboardSettingsSection extends StatelessWidget {
  const ClipboardSettingsSection(
      {super.key,
      required this.controller,
      required this.autoFill,
      this.autoFillStatus,
      required this.onAutoFill});
  final ClipboardSettingsController controller;
  final bool autoFill;
  final Widget? autoFillStatus;
  final ValueChanged<bool>? onAutoFill;

  Future<void> _clear(BuildContext context) async {
    final confirmed = await showDialog<bool>(
        context: context,
        builder: (context) => AlertDialog(
              title: const Text('Clear clipboard history?'),
              content: const Text(
                  'All saved entries will be permanently removed. The current system clipboard is unchanged.'),
              actions: [
                TextButton(
                    autofocus: true,
                    onPressed: () => Navigator.pop(context, false),
                    child: const Text('Cancel')),
                TextButton(
                    onPressed: () => Navigator.pop(context, true),
                    child: const Text('Clear History')),
              ],
            ));
    if (confirmed == true &&
        context.mounted &&
        !controller.busy &&
        controller.available) {
      await controller.clear();
    }
  }

  @override
  Widget build(BuildContext context) => ListenableBuilder(
      listenable: controller,
      builder: (context, _) {
        final enabled =
            controller.loaded && controller.available && !controller.busy;
        return Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              const AppSectionHeading('Clipboard history',
                  icon: AppIcons.clipboard),
              if (controller.notice != null) ...[
                AppNotice(controller.notice!),
                const SizedBox(height: 10),
              ],
              AppCard(
                  child: SettingsToggleRow(
                      title: 'Record clipboard history',
                      description:
                          'Save future copies while Anywhere is running. Existing clipboard contents are not imported.',
                      icon: AppIcons.clipboard,
                      value: controller.recording,
                      onChanged: enabled
                          ? (value) => controller.setRecording(value)
                          : null)),
              const SizedBox(height: 8),
              const Text(
                  'Text, images, rich text and file references are saved locally. Pausing keeps existing history.',
                  style: AppStyle.caption),
              if (!controller.loaded || !controller.available)
                Align(
                    alignment: Alignment.centerRight,
                    child: TextButton(
                        onPressed: controller.busy ? null : controller.load,
                        child:
                            Text(controller.busy ? 'Loading…' : 'Try Again'))),
              const SizedBox(height: 20),
              const AppSectionHeading('When opening Launcher',
                  icon: AppIcons.search),
              AppCard(
                  child: SettingsToggleRow(
                      title: 'Clipboard text',
                      description:
                          'Auto-fill supported content copied within the last 15 seconds. Independent of history recording.',
                      icon: AppIcons.quote,
                      status: autoFillStatus,
                      value: autoFill,
                      onChanged: onAutoFill)),
              const SizedBox(height: 20),
              const AppSectionHeading('History storage', icon: AppIcons.folder),
              AppCard(
                  child: Padding(
                      padding: const EdgeInsets.all(12),
                      child: Column(
                          crossAxisAlignment: CrossAxisAlignment.start,
                          children: [
                            const Text('Clear saved history',
                                style: AppStyle.body),
                            const SizedBox(height: 4),
                            const Text(
                                'Remove all saved entries. This does not change recording or the current system clipboard.',
                                style: AppStyle.caption),
                            const SizedBox(height: 10),
                            Align(
                                alignment: Alignment.centerRight,
                                child: OutlinedButton(
                                    onPressed:
                                        enabled ? () => _clear(context) : null,
                                    style: OutlinedButton.styleFrom(
                                        foregroundColor: AppStyle.danger),
                                    child: const Text('Clear History…'))),
                          ]))),
            ]);
      });
}
