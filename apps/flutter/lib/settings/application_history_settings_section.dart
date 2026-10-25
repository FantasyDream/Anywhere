import 'dart:async';

import 'package:flutter/material.dart';

import '../l10n/app_language.dart';
import '../src/rust/api.dart';
import '../ui/app_components.dart';
import '../ui/app_icons.dart';
import '../ui/app_style.dart';
import 'application_history_settings_controller.dart';

class AppHistorySettingsSection extends StatefulWidget {
  const AppHistorySettingsSection({super.key, required this.controller});
  final AppHistorySettingsController controller;
  @override
  State<AppHistorySettingsSection> createState() =>
      _AppHistorySettingsSectionState();
}

class _AppHistorySettingsSectionState extends State<AppHistorySettingsSection> {
  String _filter = '';
  @override
  void initState() {
    super.initState();
    unawaited(widget.controller.load());
  }

  @override
  Widget build(BuildContext context) => AnimatedBuilder(
      animation: widget.controller,
      builder: (context, _) =>
          Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
            AppSectionHeading(context.l10n.appHistorySources,
                icon: AppIcons.history),
            Text(context.l10n.appHistorySourceDescription,
                style: AppStyle.caption),
            const SizedBox(height: 12),
            TextField(
                key: const Key('history-source-filter'),
                style: AppStyle.body,
                decoration: InputDecoration(
                    hintText: context.l10n.appHistoryFilterSources,
                    prefixIcon: const Icon(AppIcons.search, size: 18)),
                onChanged: (value) =>
                    setState(() => _filter = value.trim().toLowerCase())),
            const SizedBox(height: 12),
            if (widget.controller.failed)
              AppNotice(context.l10n.appHistorySettingsFailed),
            if (!widget.controller.loaded && widget.controller.busy)
              const LinearProgressIndicator(minHeight: 1),
            for (final source in widget.controller.sources.where((source) =>
                '${source.title} ${source.id}'
                    .toLowerCase()
                    .contains(_filter))) ...[
              _SourceRow(
                  key: ValueKey(source.id),
                  source: source,
                  controller: widget.controller),
              const SizedBox(height: 10),
            ],
            AppCard(
                child: AppDisclosure(
                    key: const ValueKey('history-custom-adapters'),
                    title: context.l10n.appHistoryCustomAdapters,
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        Text(context.l10n.appHistoryCustomAdaptersHelp,
                            style: AppStyle.caption),
                        const SizedBox(height: 8),
                        SelectableText(widget.controller.customDirectory,
                            style: AppStyle.caption),
                        Align(
                            alignment: Alignment.centerRight,
                            child: TextButton(
                                key: const Key('history-custom-directory'),
                                onPressed: widget.controller.busy
                                    ? null
                                    : () => unawaited(widget.controller
                                        .openCustomDirectory()),
                                child:
                                    Text(context.l10n.appHistoryOpenAdapters))),
                      ],
                    ))),
            if (widget.controller.failed)
              TextButton(
                  onPressed: () => unawaited(widget.controller.load()),
                  child: Text(context.l10n.refresh)),
          ]));
}

class _SourceRow extends StatefulWidget {
  const _SourceRow({super.key, required this.source, required this.controller});
  final AppHistorySourceDto source;
  final AppHistorySettingsController controller;
  @override
  State<_SourceRow> createState() => _SourceRowState();
}

class _SourceRowState extends State<_SourceRow> {
  late final TextEditingController _path;
  @override
  void initState() {
    super.initState();
    _path = TextEditingController(text: widget.source.path);
  }

  @override
  void didUpdateWidget(_SourceRow old) {
    super.didUpdateWidget(old);
    if (_path.text == old.source.path) _path.text = widget.source.path;
  }

  @override
  void dispose() {
    _path.dispose();
    super.dispose();
  }

  Future<void> _configure(AppHistorySourceModeDto mode) async {
    if (mode == AppHistorySourceModeDto.custom && _path.text.trim().isEmpty) {
      return;
    }
    final saved = await widget.controller.configure(widget.source.id,
        mode == AppHistorySourceModeDto.custom ? _path.text : '',
        mode: mode);
    if (mounted && saved) {
      _path.text = widget.controller.sources
          .firstWhere((source) => source.id == widget.source.id)
          .path;
    }
  }

  String _status(BuildContext context) => !widget.source.actionEnabled
      ? context.l10n.appHistorySourceOff
      : switch (widget.source.mode) {
          AppHistorySourceModeDto.automatic => widget.source.path.isEmpty
              ? context.l10n.appHistoryNotDetected
              : context.l10n.appHistoryAutomatic,
          AppHistorySourceModeDto.custom => context.l10n.appHistoryCustom,
          AppHistorySourceModeDto.disabled => context.l10n.appHistorySourceOff,
        };

  @override
  Widget build(BuildContext context) => AppCard(
      child: Padding(
          padding: const EdgeInsets.all(12),
          child:
              Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
            Row(children: [
              Expanded(child: Text(widget.source.title, style: AppStyle.body)),
              Text(context.l10n.appHistoryActionEnabled,
                  style: AppStyle.caption),
              const SizedBox(width: 6),
              AppToggle(
                  key: Key('history-action-${widget.source.id}'),
                  label:
                      '${widget.source.title} ${context.l10n.appHistoryActionEnabled}',
                  value: widget.source.actionEnabled,
                  onChanged: widget.controller.busy
                      ? null
                      : (enabled) => unawaited(widget.controller
                          .setActionEnabled(widget.source.id, enabled))),
            ]),
            const SizedBox(height: 4),
            Text(_status(context),
                key: Key('history-mode-${widget.source.id}'),
                style: AppStyle.caption),
            const SizedBox(height: 8),
            TextField(
                key: Key('history-path-${widget.source.id}'),
                controller: _path,
                enabled: !widget.controller.busy && widget.source.actionEnabled,
                style: AppStyle.body,
                decoration: InputDecoration(
                    labelText: context.l10n.appHistorySourcePath,
                    hintText: widget.source.fileHint),
                onSubmitted: (_) =>
                    unawaited(_configure(AppHistorySourceModeDto.custom))),
            const SizedBox(height: 8),
            Row(children: [
              const Spacer(),
              TextButton(
                  key: Key('history-auto-${widget.source.id}'),
                  onPressed:
                      widget.controller.busy || !widget.source.actionEnabled
                          ? null
                          : () => unawaited(
                              _configure(AppHistorySourceModeDto.automatic)),
                  child: Text(
                      widget.source.mode == AppHistorySourceModeDto.automatic
                          ? context.l10n.appHistoryDetectAgain
                          : context.l10n.appHistoryUseAutomatic)),
              const SizedBox(width: 8),
              ValueListenableBuilder<TextEditingValue>(
                  valueListenable: _path,
                  builder: (context, value, _) => TextButton(
                      key: Key('history-save-${widget.source.id}'),
                      onPressed: widget.controller.busy ||
                              !widget.source.actionEnabled ||
                              value.text.trim().isEmpty
                          ? null
                          : () => unawaited(
                              _configure(AppHistorySourceModeDto.custom)),
                      child: Text(context.l10n.appHistorySaveCustom))),
            ]),
          ])));
}
