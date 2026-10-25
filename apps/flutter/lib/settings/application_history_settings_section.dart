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
  late final TextEditingController _filter;
  @override
  void initState() {
    super.initState();
    _filter = TextEditingController(text: widget.controller.filter);
    // The settings controller also notifies the parent screen. Start loading
    // after mounting so that notification cannot rebuild an ancestor mid-build.
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (mounted) unawaited(widget.controller.load());
    });
  }

  @override
  void dispose() {
    _filter.dispose();
    super.dispose();
  }

  void _clearFilter() {
    _filter.clear();
    widget.controller.setFilter('', onlyEnabled: false);
  }

  Widget _filterButton(String label, bool enabledOnly) {
    final selected = widget.controller.enabledOnly == enabledOnly;
    return Semantics(
        selected: selected,
        child: TextButton(
            key: Key(
                enabledOnly ? 'history-filter-enabled' : 'history-filter-all'),
            style: TextButton.styleFrom(
                backgroundColor: selected ? AppStyle.selectedSurface : null),
            onPressed: () => widget.controller
                .setFilter(_filter.text, onlyEnabled: enabledOnly),
            child: Text(label)));
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
                controller: _filter,
                style: AppStyle.body,
                decoration: InputDecoration(
                    hintText: context.l10n.appHistoryFilterSources,
                    prefixIcon: const Icon(AppIcons.search, size: 18),
                    suffixIcon: widget.controller.filter.isEmpty
                        ? null
                        : IconButton(
                            key: const Key('history-clear-filter'),
                            tooltip: context.l10n.clearSearch,
                            onPressed: _clearFilter,
                            icon: const Icon(AppIcons.close, size: 16))),
                onChanged: widget.controller.setFilter),
            const SizedBox(height: 6),
            Wrap(spacing: 6, children: [
              _filterButton(context.l10n.appHistoryAllSources, false),
              _filterButton(context.l10n.enabled, true),
            ]),
            const SizedBox(height: 12),
            if (widget.controller.failed)
              AppNotice(context.l10n.appHistorySettingsFailed),
            if (!widget.controller.loaded && widget.controller.busy)
              const LinearProgressIndicator(minHeight: 1),
            for (final source in widget.controller.visibleSources) ...[
              _SourceRow(
                  key: ValueKey(source.id),
                  source: source,
                  controller: widget.controller),
              const SizedBox(height: 10),
            ],
            if (widget.controller.loaded &&
                widget.controller.visibleSources.isEmpty)
              Padding(
                  padding: const EdgeInsets.all(12),
                  child: Column(children: [
                    Text(context.l10n.noMatchingEntries, style: AppStyle.body),
                    TextButton(
                        onPressed: _clearFilter,
                        child: Text(context.l10n.appHistoryShowAllSources)),
                  ])),
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
  final _pathFocus = FocusNode(debugLabel: 'history-source-path');
  @override
  void initState() {
    super.initState();
    _path =
        TextEditingController(text: widget.controller.draftPath(widget.source));
  }

  @override
  void didUpdateWidget(_SourceRow old) {
    super.didUpdateWidget(old);
    final draft = widget.controller.draftPath(widget.source);
    if (_path.text != draft) _path.text = draft;
  }

  @override
  void dispose() {
    _path.dispose();
    _pathFocus.dispose();
    super.dispose();
  }

  Future<void> _configure(AppHistorySourceModeDto mode) async {
    if (mode == AppHistorySourceModeDto.custom && _path.text.trim().isEmpty) {
      return;
    }
    final restoreFocus = _pathFocus.hasFocus;
    await widget.controller.configure(widget.source.id,
        mode == AppHistorySourceModeDto.custom ? _path.text : '',
        mode: mode);
    if (mounted && restoreFocus) {
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (mounted && !widget.controller.busy) _pathFocus.requestFocus();
      });
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
      child: AppDisclosure(
          key: ValueKey('history-details-${widget.source.id}'),
          title: widget.source.title,
          initiallyExpanded: widget.controller.hasDraft(widget.source.id) ||
              widget.controller.failedSource == widget.source.id ||
              (widget.source.actionEnabled && widget.source.path.isEmpty),
          subtitle: Text(
              widget.controller.hasDraft(widget.source.id)
                  ? '${_status(context)} · ${context.l10n.appHistoryUnsavedPath}'
                  : _status(context),
              key: Key('history-mode-${widget.source.id}'),
              style: AppStyle.caption),
          trailing: AppToggle(
              key: Key('history-action-${widget.source.id}'),
              label:
                  '${widget.source.title} ${context.l10n.appHistoryActionEnabled}',
              value: widget.source.actionEnabled,
              onChanged: widget.controller.busy
                  ? null
                  : (enabled) => unawaited(widget.controller
                      .setActionEnabled(widget.source.id, enabled))),
          child:
              Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
            TextField(
                key: Key('history-path-${widget.source.id}'),
                controller: _path,
                focusNode: _pathFocus,
                enabled: !widget.controller.busy,
                textInputAction: TextInputAction.done,
                onEditingComplete: () {},
                style: AppStyle.body,
                decoration: InputDecoration(
                    labelText: context.l10n.appHistorySourcePath,
                    hintText: widget.source.fileHint),
                onChanged: (text) =>
                    widget.controller.editPath(widget.source, text),
                onSubmitted: (_) {
                  if (!widget.controller.busy) {
                    unawaited(_configure(AppHistorySourceModeDto.custom));
                  }
                }),
            const SizedBox(height: 8),
            Wrap(spacing: 8, runSpacing: 4, children: [
              TextButton(
                  key: Key('history-auto-${widget.source.id}'),
                  onPressed: widget.controller.busy
                      ? null
                      : () => unawaited(
                          _configure(AppHistorySourceModeDto.automatic)),
                  child: Text(
                      widget.source.mode == AppHistorySourceModeDto.automatic
                          ? context.l10n.appHistoryDetectAgain
                          : context.l10n.appHistoryUseAutomatic)),
              if (widget.controller.hasDraft(widget.source.id))
                TextButton(
                    key: Key('history-revert-${widget.source.id}'),
                    onPressed: widget.controller.busy
                        ? null
                        : () => widget.controller
                            .discardDrafts(source: widget.source.id),
                    child: Text(context.l10n.appHistoryRevertPath)),
              ValueListenableBuilder<TextEditingValue>(
                  valueListenable: _path,
                  builder: (context, value, _) => TextButton(
                      key: Key('history-save-${widget.source.id}'),
                      onPressed: widget.controller.busy ||
                              value.text.trim().isEmpty ||
                              (widget.source.mode ==
                                      AppHistorySourceModeDto.custom &&
                                  !widget.controller.hasDraft(widget.source.id))
                          ? null
                          : () => unawaited(
                              _configure(AppHistorySourceModeDto.custom)),
                      child: Text(context.l10n.appHistorySaveCustom))),
            ]),
          ])));
}
