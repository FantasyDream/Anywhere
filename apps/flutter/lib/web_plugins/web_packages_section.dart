import '../l10n/app_language.dart';
import 'permission_labels.dart';
import 'package:flutter/material.dart';
import '../src/rust/api.dart' as api;
import '../infrastructure/web_package_backend.dart';
import '../ui/app_components.dart';
import '../ui/app_style.dart';
import '../ui/app_icons.dart';

/// Host-owned inspection and exact-content approval. Package code never runs here.
class WebPackagesSection extends StatefulWidget {
  const WebPackagesSection(
      {super.key, this.backend = const WebPackageBackend()});
  final WebPackageBackend backend;
  @override
  State<WebPackagesSection> createState() => _WebPackagesSectionState();
}

enum _PackageNotice { unavailable, installed, installFailed, operationFailed }

class _WebPackagesSectionState extends State<WebPackagesSection> {
  List<api.WebPackageDto> _packages = [];
  _PackageNotice? _notice;
  bool _busy = false;
  int _generation = 0;
  @override
  void initState() {
    super.initState();
    _refresh();
  }

  Future<void> _refresh() async {
    final generation = ++_generation;
    try {
      final packages = await widget.backend.list();
      if (mounted && generation == _generation) {
        setState(() => _packages = packages);
      }
    } catch (_) {
      if (mounted && generation == _generation) {
        setState(() => _notice = _PackageNotice.unavailable);
      }
    }
  }

  Future<void> _install() async {
    if (_busy) return;
    ++_generation; // An initial refresh must not overwrite a completed mutation.
    var inspected = false;
    setState(() {
      _busy = true;
      _notice = null;
    });
    try {
      final chosen = await widget.backend.choose();
      if (chosen == null || chosen.trim().isEmpty || !mounted) return;
      final review = await widget.backend.inspect(chosen.trim());
      inspected = true;
      if (!mounted) return;
      final approved = await showDialog<bool>(
          context: context,
          barrierDismissible: false,
          builder: (context) => AlertDialog(
                  title: Text(review.updating
                      ? context.l10n.updateToolReview(review.name)
                      : context.l10n.installToolReview(review.name)),
                  content: SizedBox(
                      width: 440,
                      child: SingleChildScrollView(
                          child: Column(
                              mainAxisSize: MainAxisSize.min,
                              crossAxisAlignment: CrossAxisAlignment.start,
                              children: [
                            Text('${review.id} · ${review.version}'),
                            const SizedBox(height: 12),
                            Text(context.l10n.requestedAccess,
                                style: AppStyle.body),
                            for (final permission in review.permissions)
                              Padding(
                                  padding: const EdgeInsets.only(top: 8),
                                  child: Text(
                                      '• ${webPermissionLabel(permission, context.l10n)}')),
                            if (review.permissions.isEmpty)
                              Text(context.l10n.noHostPermissions),
                            const SizedBox(height: 16),
                            Text(context.l10n.codeRunsInALocalWebViewNetwork,
                                style: AppStyle.caption),
                            if (review.updating) ...[
                              const SizedBox(height: 12),
                              Text(
                                  context.l10n
                                      .thisReplacesTheInstalledCodeClosesIts,
                                  style: AppStyle.caption)
                            ],
                            const SizedBox(height: 14),
                            SelectableText('SHA-256: ${review.digest}',
                                style: AppStyle.caption),
                          ]))),
                  actions: [
                    TextButton(
                        onPressed: () => Navigator.pop(context, false),
                        child: Text(context.l10n.cancel)),
                    FilledButton(
                        onPressed: () => Navigator.pop(context, true),
                        child: Text(context.l10n.approveAndInstall))
                  ]));
      if (approved == true && mounted) {
        final packages = await widget.backend.approve(review.token);
        if (mounted) {
          setState(() {
            _packages = packages;
            _notice = _PackageNotice.installed;
          });
        }
      }
    } catch (_) {
      if (mounted) {
        setState(() => _notice = _PackageNotice.installFailed);
      }
    } finally {
      if (inspected) {
        try {
          await widget.backend.cancelReview();
        } catch (_) {
          // An expired review or failed transport must not strand the settings UI.
        }
      }
      if (mounted) setState(() => _busy = false);
    }
  }

  Future<void> _change(api.WebPackageDto package, String operation) async {
    if (_busy) return;
    ++_generation;
    setState(() => _busy = true);
    try {
      final next = await widget.backend.change(package.id, operation);
      if (mounted) {
        setState(() {
          _packages = next;
          _notice = null;
        });
      }
    } catch (_) {
      // A removal can commit before private-data cleanup reports an I/O error.
      // Read authoritative status rather than showing a stale installed row.
      try {
        final packages = await widget.backend.list();
        if (mounted) setState(() => _packages = packages);
      } catch (_) {
        // Keep the last known rows when even status recovery is unavailable.
      }
      if (mounted) {
        setState(() => _notice = _PackageNotice.operationFailed);
      }
    } finally {
      if (mounted) setState(() => _busy = false);
    }
  }

  @override
  Widget build(BuildContext context) =>
      Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
        const SizedBox(height: 22),
        Row(children: [
          Expanded(
              child: AppSectionHeading(context.l10n.webTools,
                  icon: AppIcons.desktop)),
          TextButton.icon(
              onPressed: _busy ? null : _install,
              icon: const Icon(AppIcons.add, size: 16),
              label: Text(context.l10n.import))
        ]),
        const SizedBox(height: 10),
        if (_notice != null)
          Padding(
              padding: const EdgeInsets.only(bottom: 10),
              child: Text(
                  switch (_notice!) {
                    _PackageNotice.unavailable =>
                      context.l10n.webToolsAreUnavailableOnThisPlatform,
                    _PackageNotice.installed =>
                      context.l10n.installedSearchForTheToolByName,
                    _PackageNotice.installFailed =>
                      context.l10n.thePackageCouldNotBeInstalledThe,
                    _PackageNotice.operationFailed =>
                      context.l10n.theOperationCouldNotFinishCheckThe,
                  },
                  style: AppStyle.caption)),
        AppCard(
            child: Column(children: [
          if (_packages.isEmpty)
            Padding(
                padding: EdgeInsets.all(14),
                child: Text(context.l10n.noWebToolsInstalled,
                    style: AppStyle.caption)),
          for (final package in _packages)
            AppDisclosure(
                key: ValueKey('web-package-${package.id}'),
                title: package.name,
                leading: const Icon(AppIcons.plugin,
                    size: 18, color: AppStyle.secondary),
                trailing: AppToggle(
                    label: context.l10n.enableFeature(package.name),
                    value: package.enabled,
                    onChanged: _busy
                        ? null
                        : (enabled) =>
                            _change(package, enabled ? 'enable' : 'disable')),
                child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      SelectableText('${package.version} · ${package.id}',
                          style: AppStyle.caption),
                      const SizedBox(height: 6),
                      Wrap(spacing: 8, runSpacing: 6, children: [
                        if (package.rollback)
                          TextButton.icon(
                              onPressed: _busy
                                  ? null
                                  : () => _change(package, 'rollback'),
                              icon: const Icon(AppIcons.refresh, size: 14),
                              label: Text(context.l10n.restoreApprovedVersion)),
                        TextButton.icon(
                            onPressed: _busy
                                ? null
                                : () => _change(package, 'uninstall'),
                            icon: const Icon(AppIcons.delete, size: 16),
                            label: Text(context.l10n.uninstall)),
                      ]),
                    ])),
        ])),
      ]);
}
