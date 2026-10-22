import '../l10n/app_language.dart';
import 'package:flutter/material.dart';
import '../web_plugins/web_packages_section.dart';
import '../src/rust/api.dart';
import '../ui/app_components.dart';
import '../ui/app_icons.dart';
import '../ui/app_style.dart';
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
          Expanded(
              child: Text(context.l10n.chooseTheToolsAvailableInAnywhere,
                  style: AppStyle.caption)),
          IconButton(
              key: const ValueKey('refresh-features'),
              onPressed: controller.refreshingPlugins
                  ? null
                  : controller.refreshFeatureData,
              icon: const Icon(AppIcons.refresh, size: 16),
              tooltip: context.l10n.refresh),
        ]),
        const SizedBox(height: 12),
        if (controller.extensions.any((e) => e.firstParty)) ...[
          AppSectionHeading(context.l10n.builtIn, icon: AppIcons.features),
          _group(context,
              controller.extensions.where((e) => e.firstParty).toList()),
          const SizedBox(height: 16),
        ],
        if (controller.extensions.any((e) => !e.firstParty)) ...[
          AppSectionHeading(context.l10n.installedPlugins,
              icon: AppIcons.plugin),
          _group(context,
              controller.extensions.where((e) => !e.firstParty).toList()),
        ],
        const WebPackagesSection(),
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
    final attention = e.needsAttention ||
        e.phase == ExtensionPhase.needsPermission ||
        e.phase == ExtensionPhase.restartRequired;
    return AppDisclosure(
        key: ValueKey('${e.key}-details'),
        initiallyExpanded: attention,
        title: e.nameFor(context.l10n),
        leading: Icon(e.glyph,
            size: 18, color: attention ? AppStyle.warning : AppStyle.secondary),
        subtitle: attention
            ? Text('${e.stateLabelFor(context.l10n)} · ${e.detail}',
                style: AppStyle.caption.copyWith(color: AppStyle.warning))
            : null,
        trailing: e.configurable
            ? AppToggle(
                key: ValueKey('${e.key}-toggle'),
                label: context.l10n.enableFeature(e.nameFor(context.l10n)),
                value: e.enabled,
                onChanged: controller.refreshingPlugins
                    ? null
                    : (value) => controller.setExtensionEnabled(e, value))
            : null,
        child: Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
          Text(
              [
                if (e.version.isNotEmpty) e.version,
                e.stateLabelFor(context.l10n)
              ].join(' · '),
              style: AppStyle.caption),
          if (e.capabilities.isNotEmpty) ...[
            const SizedBox(height: 6),
            Text(
                e.capabilities
                    .map((capability) =>
                        _capabilityLabel(capability, context.l10n))
                    .join(' · '),
                style: AppStyle.caption),
          ],
          if (e.legacy)
            Text(context.l10n.legacyV1PermissionsCannotBeRevokedIndividually,
                style: AppStyle.caption),
          if (!e.firstParty && !e.legacy && e.configurable)
            TextButton(
                key: ValueKey('${e.key}-permission'),
                onPressed: controller.refreshingPlugins
                    ? null
                    : () => e.needsApproval
                        ? _approve(context, e)
                        : controller.revokeExtension(e),
                child: Text(e.needsApproval
                    ? context.l10n.reviewPermissions
                    : context.l10n.revokePermissions)),
        ]));
  }

  Future<void> _approve(BuildContext context, ExtensionStatusDto e) async {
    final accepted = await showDialog<bool>(
        context: context,
        builder: (context) => AlertDialog(
                title: Text(context.l10n.allowFeature(e.name)),
                content: Column(
                    mainAxisSize: MainAxisSize.min,
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text('${e.id} · ${e.version}', style: AppStyle.caption),
                      const SizedBox(height: 12),
                      Text(context.l10n.thisExtensionRequestsPermissionTo),
                      const SizedBox(height: 8),
                      for (final capability in e.capabilities)
                        Padding(
                            padding: const EdgeInsets.only(bottom: 6),
                            child: Text(
                                '• ${_capabilityLabel(capability, context.l10n)}')),
                      if (e.capabilities.isEmpty)
                        Text(context.l10n.noHostPermissionsRequested),
                      const SizedBox(height: 8),
                      Text(
                          context.l10n
                              .approvalAppliesToThisInspectedDefinitionRestart,
                          style: AppStyle.caption),
                    ]),
                actions: [
                  TextButton(
                      autofocus: true,
                      onPressed: () => Navigator.pop(context, false),
                      child: Text(context.l10n.cancel)),
                  FilledButton(
                      onPressed: () => Navigator.pop(context, true),
                      child: Text(context.l10n.allowForNextLaunch))
                ]));
    if (accepted == true && context.mounted) {
      await controller.approveExtension(e);
    }
  }
}

String _capabilityLabel(String capability, AppLocalizations l10n) =>
    switch (capability) {
      'ui.render' => l10n.showAPluginViewInAnywhere,
      'clipboard.write' => l10n.writeToTheClipboard,
      'url.open' => l10n.openWebAddresses,
      'file.open' => l10n.openFiles,
      'git.read' => l10n.readRepositoryInformation,
      'process.start' => l10n.runProjectTasks,
      'storage' => l10n.saveExtensionData,
      _ => capability,
    };
