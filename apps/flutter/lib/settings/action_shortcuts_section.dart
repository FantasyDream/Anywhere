import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import '../l10n/app_language.dart';
import '../l10n/localized_label.dart';
import '../src/rust/api.dart';
import '../ui/app_components.dart';
import '../ui/app_icons.dart';
import '../ui/app_style.dart';
import 'action_shortcut.dart';
import 'host_settings.dart';
import 'intent_shortcut_dialog.dart';
import 'settings_components.dart';
import 'settings_controller.dart';
import 'shortcut_recorder.dart';

/// Uses the settings page's existing save queue and close protection.
class ActionShortcutsSection extends StatefulWidget {
  const ActionShortcutsSection(
      {super.key,
      required this.controller,
      required this.save,
      required this.onEditingChanged});
  final SettingsController controller;
  final Future<bool> Function(HostSettings Function(HostSettings)) save;
  final ValueChanged<bool> onEditingChanged;
  @override
  State<ActionShortcutsSection> createState() => _ActionShortcutsSectionState();
}

class _ActionShortcutsSectionState extends State<ActionShortcutsSection> {
  bool _busy = false;
  final _rowKeys = <String, GlobalKey>{};
  String _bindingTitle(BuildContext context, ActionShortcut binding) {
    if (binding.isIntent) return binding.title; // User-authored name.
    final action = widget.controller.shortcutActions
        .where((a) => a.actionId == binding.actionId)
        .firstOrNull;
    return action == null
        ? binding.title
        : context.localizedLabel(action.title, action.localizedTitles);
  }

  Future<void> _edit({ActionShortcut? existing, bool intent = false}) async {
    final options = widget.controller.shortcutActions;
    intent = intent || (existing?.isIntent ?? false);
    if (options.isEmpty && existing == null) return;
    widget.onEditingChanged(true);
    ActionShortcut? binding;
    try {
      binding = await showDialog<ActionShortcut>(
          context: context,
          builder: (_) => intent
              ? IntentShortcutDialog(
                  actions: options,
                  applications: widget.controller.shortcutApplications,
                  settings: widget.controller.current,
                  existing: existing)
              : _ShortcutDialog(
                  options: options,
                  settings: widget.controller.current,
                  existing: existing));
    } finally {
      widget.onEditingChanged(false);
    }
    final selected = binding;
    if (!mounted || selected == null) return;
    await _save((current) => current.copyWith(actionShortcuts: [
          for (final b in current.actionShortcuts)
            if (b.actionId != selected.actionId) b,
          selected,
        ]));
    if (!mounted) return;
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!mounted) return;
      final row = _rowKeys[selected.actionId]?.currentContext;
      if (row != null) {
        Scrollable.ensureVisible(row,
            alignment: 0.2, duration: const Duration(milliseconds: 120));
      }
    });
  }

  Future<void> _save(HostSettings Function(HostSettings) update) async {
    setState(() => _busy = true);
    try {
      await widget.save(update);
    } finally {
      if (mounted) {
        _rowKeys.removeWhere((id, _) => !widget
            .controller.current.actionShortcuts
            .any((b) => b.actionId == id));
        setState(() => _busy = false);
      }
    }
  }

  @override
  Widget build(BuildContext context) =>
      Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
        AppSectionHeading(context.l10n.actionShortcuts, icon: AppIcons.bolt),
        Text(context.l10n.bindAKeyToAnActionOr,
            style: TextStyle(fontSize: 11, color: AppStyle.muted)),
        const SizedBox(height: 8),
        AppCard(
            child: Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: [
              for (final binding in widget.controller.current.actionShortcuts)
                Padding(
                    key: _rowKeys.putIfAbsent(binding.actionId, GlobalKey.new),
                    padding: const EdgeInsets.fromLTRB(12, 5, 4, 5),
                    child: Row(children: [
                      Expanded(
                          child: Column(
                              crossAxisAlignment: CrossAxisAlignment.start,
                              children: [
                            Text(_bindingTitle(context, binding),
                                maxLines: 1,
                                overflow: TextOverflow.ellipsis,
                                style: const TextStyle(
                                    fontSize: 13, color: AppStyle.primary)),
                            Text(
                                binding.global
                                    ? context.l10n.globalWorksFromOtherApps
                                    : context.l10n.whileAnywhereIsOpen,
                                style: const TextStyle(
                                    fontSize: 11, color: AppStyle.muted)),
                            if (binding.isIntent)
                              Text(
                                  context.l10n
                                      .contextRulesCount(binding.routes.length),
                                  style: const TextStyle(
                                      fontSize: 11, color: AppStyle.muted)),
                            if (!binding.isIntent &&
                                !widget.controller
                                    .shortcutSupported(binding.actionId))
                              Text(context.l10n.actionUnavailableShortcutPaused,
                                  style: TextStyle(
                                      fontSize: 11, color: AppStyle.warning)),
                            if (binding.isIntent &&
                                binding.routes.any((r) => !widget.controller
                                    .shortcutSupported(r.actionId)))
                              Text(
                                  binding.routes.every((r) => !widget.controller
                                          .shortcutSupported(r.actionId))
                                      ? context
                                          .l10n.actionsUnavailableShortcutPaused
                                      : context.l10n.someRulesAreUnavailable,
                                  style: const TextStyle(
                                      fontSize: 11, color: AppStyle.warning)),
                            if (!binding.isIntent &&
                                widget.controller
                                    .shortcutSupported(binding.actionId) &&
                                !widget.controller.shortcutActions
                                    .any((a) => a.actionId == binding.actionId))
                              Text(context.l10n.notDetectedInTheLastContext,
                                  style: TextStyle(
                                      fontSize: 11, color: AppStyle.muted)),
                          ])),
                      Tooltip(
                          message:
                              context.l10n.editActionShortcut(binding.title),
                          child: OutlinedButton.icon(
                              onPressed:
                                  _busy ? null : () => _edit(existing: binding),
                              icon: const Icon(AppIcons.edit, size: 13),
                              label: Text(binding.label,
                                  style: const TextStyle(fontSize: 11)))),
                      IconButton(
                          tooltip:
                              context.l10n.removeActionShortcut(binding.title),
                          iconSize: 16,
                          onPressed: _busy
                              ? null
                              : () => _save((current) => current.copyWith(
                                  actionShortcuts: current.actionShortcuts
                                      .where(
                                          (b) => b.actionId != binding.actionId)
                                      .toList())),
                          icon: const Icon(AppIcons.delete,
                              color: AppStyle.muted)),
                    ])),
              Padding(
                  padding: const EdgeInsets.all(12),
                  child: Wrap(spacing: 8, runSpacing: 8, children: [
                    OutlinedButton.icon(
                        onPressed:
                            _busy || widget.controller.shortcutActions.isEmpty
                                ? null
                                : () => _edit(),
                        icon: const Icon(AppIcons.add, size: 14),
                        label: Text(context.l10n.addActionShortcut)),
                    OutlinedButton.icon(
                        onPressed:
                            _busy || widget.controller.shortcutActions.isEmpty
                                ? null
                                : () => _edit(intent: true),
                        icon: const Icon(AppIcons.route, size: 14),
                        label: Text(context.l10n.addContextShortcut)),
                  ])),
            ])),
        const SizedBox(height: 6),
        Text(context.l10n.toolsSuchAsFileSearchAreAlways,
            style: TextStyle(fontSize: 11, color: AppStyle.muted)),
      ]);
}

class _ShortcutDialog extends StatefulWidget {
  const _ShortcutDialog(
      {required this.options, required this.settings, this.existing});
  final List<ShortcutActionDto> options;
  final HostSettings settings;
  final ActionShortcut? existing;
  @override
  State<_ShortcutDialog> createState() => _ShortcutDialogState();
}

class _ShortcutDialogState extends State<_ShortcutDialog> {
  final _focus = FocusNode();
  final _recorder = ShortcutRecorder();
  final _search = TextEditingController();
  ShortcutActionDto? _action;
  ActionShortcut? _binding;
  String? _error;
  bool _recording = false;
  bool _global = false;
  @override
  void initState() {
    super.initState();
    _binding = widget.existing;
    _global = widget.existing?.global ?? false;
    for (final action in widget.options) {
      if (action.actionId == widget.existing?.actionId) _action = action;
    }
    final saved = widget.existing;
    if (saved != null && _action == null) {
      // Display saved metadata only; Core still checks registration eligibility
      // and resolves fresh execution targets when invoked.
      _action = ShortcutActionDto(
          actionId: saved.actionId, title: saved.title, contextTypes: const []);
    }
  }

  @override
  void dispose() {
    _recorder.cancel();
    _focus.dispose();
    _search.dispose();
    super.dispose();
  }

  KeyEventResult _key(FocusNode node, KeyEvent event) {
    if (!_recording || event is! KeyDownEvent) return KeyEventResult.ignored;
    if (_recorder.nativePending) return KeyEventResult.handled;
    if (event.logicalKey == LogicalKeyboardKey.escape) {
      _recorder.cancel();
      setState(() {
        _recording = false;
        _error = null;
      });
      return KeyEventResult.handled;
    }
    final code = shortcutEventKeyCode(event);
    if (code == null) return KeyEventResult.handled;
    _acceptChord(code, pressedModifiers());
    return KeyEventResult.handled;
  }

  bool _acceptChord(int code, int modifiers) {
    final action = _action!;
    final binding = ActionShortcut(
        actionId: action.actionId,
        title: action.title,
        keyCode: code,
        modifiers: modifiers,
        global: _global);
    final error = binding.validate(widget.settings, context.l10n);
    setState(() {
      _error = error;
      if (error == null) {
        _binding = binding;
        _recording = false;
      }
    });
    return error == null;
  }

  @override
  Widget build(BuildContext context) {
    final query = _search.text.toLowerCase();
    final actions = widget.options
        .where((a) =>
            '${a.title} ${a.contextTypes.map(contextTypeLabel).join(' ')} ${a.contextTypes.join(' ')}'
                .toLowerCase()
                .contains(query))
        .toList();
    return AlertDialog(
      scrollable: _action != null,
      title: Text(context.l10n.actionShortcut),
      content: Focus(
          focusNode: _focus,
          onKeyEvent: _key,
          child: SizedBox(
              width: 420,
              height: _action == null ? 350 : null,
              child: Column(
                  mainAxisSize: MainAxisSize.min,
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    if (_action == null) ...[
                      TextField(
                          controller: _search,
                          autofocus: true,
                          decoration: InputDecoration(
                              hintText: context.l10n.findAnAction,
                              prefixIcon: Icon(AppIcons.search, size: 17),
                              isDense: true),
                          onChanged: (_) => setState(() {})),
                      const SizedBox(height: 8),
                      Expanded(
                          child: ListView(children: [
                        for (final action in actions)
                          ListTile(
                              dense: true,
                              title: Text(action.title),
                              subtitle: Text(
                                  action.contextTypes
                                      .map(contextTypeLabel)
                                      .join(' · '),
                                  style: AppStyle.caption),
                              onTap: () {
                                setState(() {
                                  _action = action;
                                  _binding = null;
                                });
                                _focus.requestFocus();
                              }),
                      ])),
                    ] else ...[
                      Text(
                          context.localizedLabel(
                              _action!.title, _action!.localizedTitles),
                          style: const TextStyle(fontWeight: FontWeight.w600)),
                      Text(
                          _action!.contextTypes
                              .map(contextTypeLabel)
                              .join(' · '),
                          style: const TextStyle(
                              fontSize: 11, color: AppStyle.muted)),
                      if (_action!.contextTypes.isEmpty)
                        Text(context.l10n.notDetectedInTheLastContextThe,
                            style:
                                TextStyle(fontSize: 11, color: AppStyle.muted)),
                      const SizedBox(height: 16),
                      OutlinedButton(
                          onPressed: () {
                            setState(() {
                              _recording = true;
                              _error = null;
                            });
                            _recorder.start(
                                onChord: _acceptChord,
                                onCancel: () {
                                  if (mounted) {
                                    setState(() {
                                      _recording = false;
                                      _error = null;
                                    });
                                  }
                                },
                                onError: () {
                                  if (mounted) {
                                    setState(() {
                                      _recording = false;
                                      _error = context
                                          .l10n.shortcutRecordingFailedTryAgain;
                                    });
                                  }
                                });
                            _focus.requestFocus();
                          },
                          child: Text(_recording
                              ? context.l10n.pressAShortcut
                              : (_binding?.label ??
                                  context.l10n.recordShortcut))),
                      const SizedBox(height: 8),
                      Text(context.l10n.useCommandOptionOrControlWithA,
                          style:
                              TextStyle(fontSize: 12, color: AppStyle.muted)),
                      SettingsToggleRow(
                          title: context.l10n.globalShortcut,
                          description:
                              context.l10n.worksFromOtherAppsUsesFreshContext,
                          value: _global,
                          onChanged: _recording
                              ? null
                              : (value) => setState(() {
                                    _global = value;
                                    _binding = _binding?.withGlobal(value);
                                  })),
                      if (_error != null)
                        Padding(
                            padding: const EdgeInsets.only(top: 10),
                            child: Text(_error!,
                                style: const TextStyle(
                                    fontSize: 12, color: AppStyle.warning))),
                      if (widget.existing == null)
                        TextButton(
                            onPressed: () {
                              _recorder.cancel();
                              setState(() {
                                _recording = false;
                                _action = null;
                                _binding = null;
                                _error = null;
                              });
                            },
                            child: Text(context.l10n.chooseAnotherAction)),
                    ],
                  ]))),
      actions: [
        TextButton(
            onPressed: () => Navigator.pop(context),
            child: Text(context.l10n.cancel)),
        FilledButton(
            onPressed: _binding == null ||
                    _action == null ||
                    _recording ||
                    _error != null
                ? null
                : () => Navigator.pop(context, _binding),
            child: Text(context.l10n.saveShortcut)),
      ],
    );
  }
}
