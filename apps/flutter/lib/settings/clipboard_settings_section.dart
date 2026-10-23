import 'package:flutter/material.dart';
import 'package:intl/intl.dart';

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

  Future<void> _retention(BuildContext context, int days) async {
    final removes = days != 0 &&
        (controller.retentionDays == 0 || days < controller.retentionDays);
    if (removes && controller.storedEntries > 0) {
      final confirmed = await showDialog<bool>(
          context: context,
          builder: (context) => AlertDialog(
                title: Text(context.l10n.changeHistoryRetention),
                content: Text(context.l10n.historyRetentionWarning),
                actions: [
                  TextButton(
                      autofocus: true,
                      onPressed: () => Navigator.pop(context, false),
                      child: Text(context.l10n.cancel)),
                  TextButton(
                      onPressed: () => Navigator.pop(context, true),
                      child: Text(context.l10n.saveChanges)),
                ],
              ));
      if (confirmed != true || !context.mounted) return;
    }
    if (!controller.busy && controller.available) {
      await controller.setRetention(days);
    }
  }

  Widget _storageControls(BuildContext context, bool enabled) => Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(children: [
            Expanded(
                child:
                    Text(context.l10n.keepHistoryFor, style: AppStyle.caption)),
            DropdownButtonHideUnderline(
                child: DropdownButton<int>(
              value: controller.retentionDays,
              isDense: true,
              style: AppStyle.caption,
              onChanged: enabled
                  ? (value) {
                      if (value != null && value != controller.retentionDays) {
                        _retention(context, value);
                      }
                    }
                  : null,
              items: [
                for (final days in [0, 1, 7, 30, 90])
                  DropdownMenuItem(
                      value: days,
                      child: Text(days == 0
                          ? context.l10n.untilStorageLimit
                          : context.l10n.retentionDays(days)))
              ],
            )),
          ]),
          const SizedBox(height: 6),
          Text(
              context.l10n.historyStorageUsage(controller.storedEntries,
                  (controller.storedBytes / (1024 * 1024)).toStringAsFixed(1)),
              style: AppStyle.caption),
          const SizedBox(height: 4),
          Text(context.l10n.historyStorageExplanation, style: AppStyle.caption),
        ],
      );

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
                  child: Column(children: [
                SettingsToggleRow(
                    title: context.l10n.recordClipboardHistory,
                    description:
                        context.l10n.saveFutureCopiesWhileAnywhereIsRunning,
                    icon: AppIcons.clipboard,
                    value: controller.recording,
                    onChanged: enabled
                        ? (value) => controller.setRecording(value)
                        : null),
                if (controller.recording)
                  Padding(
                    padding: const EdgeInsets.fromLTRB(39, 0, 12, 10),
                    child: Row(children: [
                      Expanded(
                          child: Text(
                              controller.pausedUntilMs == null
                                  ? context.l10n.pauseHistoryRecording
                                  : context.l10n.historyPausedUntil(
                                      DateFormat.Hm(context.l10n.localeName)
                                          .format(DateTime
                                              .fromMillisecondsSinceEpoch(
                                                  controller.pausedUntilMs!))),
                              style: AppStyle.caption)),
                      if (controller.pausedUntilMs != null)
                        OutlinedButton(
                            onPressed:
                                enabled ? () => controller.pause(0) : null,
                            child: Text(context.l10n.resumeRecording))
                      else
                        DropdownButtonHideUnderline(
                            child: DropdownButton<int>(
                          hint: Text(context.l10n.pauseFor),
                          isDense: true,
                          style: AppStyle.caption,
                          onChanged: enabled
                              ? (value) {
                                  if (value != null) controller.pause(value);
                                }
                              : null,
                          items: [
                            for (final minutes in [5, 15, 60])
                              DropdownMenuItem(
                                  value: minutes,
                                  child:
                                      Text(context.l10n.pauseMinutes(minutes)))
                          ],
                        )),
                    ]),
                  ),
              ])),
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
                            _storageControls(context, enabled),
                            const SizedBox(height: 14),
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
