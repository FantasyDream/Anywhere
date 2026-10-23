import 'package:flutter/material.dart';

import '../l10n/app_language.dart';
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
              title: Text(context.l10n.clearClipboardHistory),
              content:
                  Text(context.l10n.allSavedEntriesWillBePermanentlyRemoved),
              actions: [
                TextButton(
                    autofocus: true,
                    onPressed: () => Navigator.pop(context, false),
                    child: Text(context.l10n.cancel)),
                TextButton(
                    onPressed: () => Navigator.pop(context, true),
                    child: Text(context.l10n.clearHistory)),
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
              AppSectionHeading(context.l10n.clipboardHistorySetting,
                  icon: AppIcons.clipboard),
              if (controller.noticeFor(context.l10n) != null) ...[
                AppNotice(controller.noticeFor(context.l10n)!),
                const SizedBox(height: 10),
              ],
              AppCard(
                  child: SettingsToggleRow(
                      title: context.l10n.recordClipboardHistory,
                      description:
                          context.l10n.saveFutureCopiesWhileAnywhereIsRunning,
                      icon: AppIcons.clipboard,
                      value: controller.recording,
                      onChanged: enabled
                          ? (value) => controller.setRecording(value)
                          : null)),
              const SizedBox(height: 8),
              Text(context.l10n.textImagesRichTextAndFileReferences,
                  style: AppStyle.caption),
              if (!controller.loaded || !controller.available)
                Align(
                    alignment: Alignment.centerRight,
                    child: TextButton(
                        onPressed: controller.busy ? null : controller.load,
                        child: Text(controller.busy
                            ? context.l10n.loading
                            : context.l10n.tryAgain))),
              const SizedBox(height: 20),
              AppSectionHeading(context.l10n.whenOpeningLauncher,
                  icon: AppIcons.search),
              AppCard(
                  child: SettingsToggleRow(
                      title: context.l10n.clipboardText,
                      description:
                          context.l10n.autoFillSupportedContentCopiedWithinThe,
                      icon: AppIcons.quote,
                      status: autoFillStatus,
                      value: autoFill,
                      onChanged: onAutoFill)),
              const SizedBox(height: 20),
              AppCard(
                  child: AppDisclosure(
                      title: context.l10n.historyStorage,
                      child: Column(
                          crossAxisAlignment: CrossAxisAlignment.start,
                          children: [
                            Text(context.l10n.removeAllSavedEntriesThisDoesNot,
                                style: AppStyle.caption),
                            const SizedBox(height: 10),
                            Align(
                                alignment: Alignment.centerRight,
                                child: OutlinedButton(
                                    onPressed:
                                        enabled ? () => _clear(context) : null,
                                    style: OutlinedButton.styleFrom(
                                        foregroundColor: AppStyle.danger),
                                    child:
                                        Text(context.l10n.clearHistoryAction))),
                          ]))),
            ]);
      });
}
