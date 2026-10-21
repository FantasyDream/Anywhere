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

class _WebPackagesSectionState extends State<WebPackagesSection> {
  List<api.WebPackageDto> _packages = [];
  String? _notice;
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
        setState(() => _notice = 'Web tools are unavailable on this platform.');
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
                  title: Text(
                      '${review.updating ? 'Update' : 'Install'} ${review.name}?'),
                  content: SizedBox(
                      width: 440,
                      child: SingleChildScrollView(
                          child: Column(
                              mainAxisSize: MainAxisSize.min,
                              crossAxisAlignment: CrossAxisAlignment.start,
                              children: [
                            Text('${review.id} · ${review.version}'),
                            const SizedBox(height: 12),
                            const Text('Requested access',
                                style: AppStyle.body),
                            for (final permission in review.permissions)
                              Padding(
                                  padding: const EdgeInsets.only(top: 8),
                                  child: Text('• $permission')),
                            if (review.permissions.isEmpty)
                              const Text('No Host permissions.'),
                            const SizedBox(height: 16),
                            const Text(
                                'Code runs in a local WebView. Network access and navigation are blocked. This is not an OS process sandbox.',
                                style: AppStyle.caption),
                            if (review.updating) ...[
                              const SizedBox(height: 12),
                              const Text(
                                  'This replaces the installed code, closes its active page, and keeps its local settings. A previously approved version can be restored.',
                                  style: AppStyle.caption)
                            ],
                            const SizedBox(height: 14),
                            SelectableText('SHA-256: ${review.digest}',
                                style: AppStyle.caption),
                          ]))),
                  actions: [
                    TextButton(
                        onPressed: () => Navigator.pop(context, false),
                        child: const Text('Cancel')),
                    FilledButton(
                        onPressed: () => Navigator.pop(context, true),
                        child: const Text('Approve and install'))
                  ]));
      if (approved == true && mounted) {
        final packages = await widget.backend.approve(review.token);
        if (mounted) {
          setState(() {
            _packages = packages;
            _notice = 'Installed. Search for the tool by name to open it.';
          });
        }
      }
    } catch (_) {
      if (mounted) {
        setState(() => _notice =
            'The package could not be installed. The existing version is unchanged.');
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
        setState(() => _notice =
            'The operation could not finish. Check the current tool status and try again.');
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
          const Expanded(
              child: AppSectionHeading('Web tools', icon: AppIcons.desktop)),
          TextButton.icon(
              onPressed: _busy ? null : _install,
              icon: const Icon(AppIcons.add, size: 16),
              label: const Text('Import…'))
        ]),
        const Text(
            'Local development packages on macOS. Windows support and public distribution are still under validation.',
            style: AppStyle.caption),
        const SizedBox(height: 10),
        if (_notice != null)
          Padding(
              padding: const EdgeInsets.only(bottom: 10),
              child: Text(_notice!, style: AppStyle.caption)),
        AppCard(
            child: Column(children: [
          if (_packages.isEmpty)
            const Padding(
                padding: EdgeInsets.all(14),
                child:
                    Text('No Web tools installed.', style: AppStyle.caption)),
          for (final package in _packages)
            Padding(
                padding:
                    const EdgeInsets.symmetric(horizontal: 12, vertical: 10),
                child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Row(children: [
                        Expanded(
                            child: Text(package.name, style: AppStyle.body)),
                        const SizedBox(width: 10),
                        AppToggle(
                            label: 'Enable ${package.name}',
                            value: package.enabled,
                            onChanged: _busy
                                ? null
                                : (enabled) => _change(
                                    package, enabled ? 'enable' : 'disable')),
                      ]),
                      Text('${package.version} · ${package.id}',
                          style: AppStyle.caption),
                      const SizedBox(height: 6),
                      Wrap(spacing: 8, runSpacing: 6, children: [
                        if (package.rollback)
                          TextButton.icon(
                              onPressed: _busy
                                  ? null
                                  : () => _change(package, 'rollback'),
                              icon: const Icon(AppIcons.refresh, size: 14),
                              label: const Text('Restore approved version')),
                        TextButton.icon(
                            onPressed: _busy
                                ? null
                                : () => _change(package, 'uninstall'),
                            icon: const Icon(AppIcons.close, size: 14),
                            label: const Text('Uninstall')),
                      ]),
                    ])),
        ])),
      ]);
}
