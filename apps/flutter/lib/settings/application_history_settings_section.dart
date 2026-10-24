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
            if (widget.controller.failed)
              AppNotice(context.l10n.appHistorySettingsFailed),
            if (!widget.controller.loaded && widget.controller.busy)
              const LinearProgressIndicator(minHeight: 1),
            for (final source in widget.controller.sources) ...[
              _SourceRow(
                  key: ValueKey(source.id),
                  source: source,
                  controller: widget.controller),
              const SizedBox(height: 10),
            ],
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

  @override
  Widget build(BuildContext context) => AppCard(
      child: Padding(
          padding: const EdgeInsets.all(12),
          child:
              Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
            Text(widget.source.title, style: AppStyle.body),
            const SizedBox(height: 8),
            TextField(
                key: Key('history-path-${widget.source.id}'),
                controller: _path,
                enabled: !widget.controller.busy,
                style: AppStyle.body,
                decoration: InputDecoration(
                    labelText: context.l10n.appHistorySourcePath,
                    hintText: widget.source.fileHint),
                onSubmitted: (_) => unawaited(
                    widget.controller.configure(widget.source.id, _path.text))),
            const SizedBox(height: 8),
            Row(mainAxisAlignment: MainAxisAlignment.end, children: [
              if (widget.source.path.isNotEmpty)
                TextButton(
                    onPressed: widget.controller.busy
                        ? null
                        : () => unawaited(
                            widget.controller.configure(widget.source.id, '')),
                    child: Text(context.l10n.appHistoryDisable)),
              TextButton(
                  key: Key('history-save-${widget.source.id}'),
                  onPressed: widget.controller.busy
                      ? null
                      : () => unawaited(widget.controller
                          .configure(widget.source.id, _path.text)),
                  child: Text(widget.source.path.isEmpty
                      ? context.l10n.appHistoryEnable
                      : context.l10n.saveChanges)),
            ]),
          ])));
}
