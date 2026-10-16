import 'package:flutter/material.dart';
import '../src/rust/api.dart';
import '../ui/app_components.dart';
import '../ui/app_icons.dart';
import '../ui/app_style.dart';
import 'settings_components.dart';
import 'settings_controller.dart';
import 'extension_presentation.dart';

/// Presentation only: Core owns the inspected definition, approval and persistence.
class FeaturesSection extends StatelessWidget {
  const FeaturesSection({super.key, required this.controller});
  final SettingsController controller;

  @override
  Widget build(BuildContext context) =>
      Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
        Row(children: [
          const Expanded(
              child: AppSectionHeading('Features', icon: AppIcons.layers)),
          TextButton.icon(
              key: const ValueKey('refresh-features'),
              onPressed: controller.refreshingPlugins
                  ? null
                  : controller.refreshFeatureData,
              icon: const Icon(AppIcons.refresh, size: 14),
              label:
                  Text(controller.refreshingPlugins ? 'Updating…' : 'Refresh')),
        ]),
        const Text('Choose the tools available in Anywhere.',
            style: AppStyle.caption),
        const SizedBox(height: 12),
        if (controller.extensions.any((e) => e.firstParty)) ...[
          const AppSectionHeading('Built in', icon: AppIcons.layers),
          _group(context,
              controller.extensions.where((e) => e.firstParty).toList()),
          const SizedBox(height: 16),
        ],
        const AppSectionHeading('Installed plugins', icon: AppIcons.plugin),
        if (controller.extensions.any((e) => !e.firstParty))
          _group(context,
              controller.extensions.where((e) => !e.firstParty).toList())
        else
          const AppCard(
              child: Padding(
                  padding: EdgeInsets.all(12),
                  child: Text('No external extensions installed.',
                      style: AppStyle.caption))),
      ]);

  Widget _group(BuildContext context, List<ExtensionStatusDto> extensions) =>
      AppCard(
          child: Column(children: [
        for (var i = 0; i < extensions.length; i++) ...[
          if (i > 0) const Divider(height: 1),
          _row(context, extensions[i]),
        ],
      ]));

  Widget _row(BuildContext context, ExtensionStatusDto e) {
    final attention = e.needsAttention;
    return Padding(
        padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 10),
        child: Row(crossAxisAlignment: CrossAxisAlignment.start, children: [
          Padding(
              padding: const EdgeInsets.only(top: 3),
              child: Icon(
                  attention
                      ? AppIcons.error
                      : (e.firstParty ? AppIcons.layers : AppIcons.plugin),
                  size: 17,
                  color: attention ? AppStyle.warning : AppStyle.secondary)),
          const SizedBox(width: 10),
          Expanded(
              child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                Text(e.name,
                    style:
                        const TextStyle(fontSize: 13, color: AppStyle.primary)),
                const SizedBox(height: 2),
                Text(
                    [
                      if (e.version.isNotEmpty) e.version,
                      e.stateLabel,
                      if (e.detail != e.stateLabel) e.detail
                    ].join(' · '),
                    style: AppStyle.caption.copyWith(
                        color:
                            attention ? AppStyle.warning : AppStyle.secondary)),
                if (e.capabilities.isNotEmpty) ...[
                  const SizedBox(height: 4),
                  Text(e.capabilities.map(_capabilityLabel).join(' · '),
                      style: AppStyle.caption),
                ],
                if (e.legacy)
                  const Padding(
                      padding: EdgeInsets.only(top: 4),
                      child: Text(
                          'Legacy v1 permissions cannot be revoked individually. Disable to stop its actions.',
                          style: AppStyle.caption)),
                if (!e.firstParty && !e.legacy && e.configurable) ...[
                  const SizedBox(height: 6),
                  TextButton(
                      key: ValueKey('${e.key}-permission'),
                      onPressed: controller.refreshingPlugins
                          ? null
                          : () => e.needsApproval
                              ? _approve(context, e)
                              : controller.revokeExtension(e),
                      child: Text(e.needsApproval
                          ? 'Review permissions…'
                          : 'Revoke permissions')),
                ],
              ])),
          if (e.configurable) ...[
            const SizedBox(width: 10),
            SettingsToggle(
                key: ValueKey('${e.key}-toggle'),
                label: 'Enable ${e.name}',
                value: e.enabled,
                onChanged: controller.refreshingPlugins
                    ? null
                    : (value) => controller.setExtensionEnabled(e, value)),
          ],
        ]));
  }

  Future<void> _approve(BuildContext context, ExtensionStatusDto e) async {
    final accepted = await showDialog<bool>(
        context: context,
        builder: (context) => AlertDialog(
                title: Text('Allow ${e.name}?'),
                content: Column(
                    mainAxisSize: MainAxisSize.min,
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text('${e.id} · ${e.version}', style: AppStyle.caption),
                      const SizedBox(height: 12),
                      const Text('This extension requests permission to:'),
                      const SizedBox(height: 8),
                      for (final capability in e.capabilities)
                        Padding(
                            padding: const EdgeInsets.only(bottom: 6),
                            child: Text('• ${_capabilityLabel(capability)}')),
                      if (e.capabilities.isEmpty)
                        const Text('No host permissions requested.'),
                      const SizedBox(height: 8),
                      const Text(
                          'Approval applies to this inspected definition. Restart Anywhere to activate a newly approved plugin. Revoking permission takes effect immediately.',
                          style: AppStyle.caption),
                    ]),
                actions: [
                  TextButton(
                      autofocus: true,
                      onPressed: () => Navigator.pop(context, false),
                      child: const Text('Cancel')),
                  FilledButton(
                      onPressed: () => Navigator.pop(context, true),
                      child: const Text('Allow for next launch'))
                ]));
    if (accepted == true && context.mounted) {
      await controller.approveExtension(e);
    }
  }
}

String _capabilityLabel(String capability) => switch (capability) {
      'ui.render' => 'Show a plugin view in Anywhere',
      'clipboard.write' => 'Write to the clipboard',
      'url.open' => 'Open web addresses',
      'file.open' => 'Open files',
      'git.read' => 'Read repository information',
      'process.start' => 'Run project tasks',
      'storage' => 'Save extension data',
      _ => capability,
    };
