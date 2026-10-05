import 'dart:async';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import '../launcher/launcher_style.dart';
import '../launcher/widgets/candidate_row.dart';
import '../src/rust/api.dart';
import 'action_shortcut.dart';
import 'host_settings.dart';
import 'settings_controller.dart';

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
  Future<void> _edit([ActionShortcut? existing]) async {
    final options = widget.controller.shortcutActions;
    if (options.isEmpty) return;
    if (existing != null &&
        !options.any((a) => a.actionId == existing.actionId)) {
      return;
    }
    widget.onEditingChanged(true);
    ActionShortcut? binding;
    try {
      binding = await showDialog<ActionShortcut>(
          context: context,
          builder: (_) => _ShortcutDialog(
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
  }

  Future<void> _save(HostSettings Function(HostSettings) update) async {
    setState(() => _busy = true);
    try {
      await widget.save(update);
    } finally {
      if (mounted) setState(() => _busy = false);
    }
  }

  @override
  Widget build(BuildContext context) =>
      Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
        const Text('ACTION SHORTCUTS',
            style: TextStyle(
                fontSize: 11,
                fontWeight: FontWeight.w600,
                letterSpacing: 1,
                color: LauncherStyle.secondary)),
        const SizedBox(height: 8),
        const Text(
            'Run an action while Anywhere is open. Uses the current context; multiple objects let you choose.',
            style: TextStyle(fontSize: 11, color: LauncherStyle.muted)),
        const SizedBox(height: 8),
        Material(
            color: const Color(0x24ffffff),
            borderRadius: BorderRadius.circular(12),
            child: Column(children: [
              for (final binding in widget.controller.current.actionShortcuts)
                Padding(
                    padding: const EdgeInsets.fromLTRB(12, 5, 4, 5),
                    child: Row(children: [
                      Expanded(
                          child: Column(
                              crossAxisAlignment: CrossAxisAlignment.start,
                              children: [
                            Text(binding.title,
                                maxLines: 1,
                                overflow: TextOverflow.ellipsis,
                                style: const TextStyle(
                                    fontSize: 13,
                                    color: LauncherStyle.primary)),
                            if (!widget.controller.shortcutActions
                                .any((a) => a.actionId == binding.actionId))
                              const Text('Not detected in the last context',
                                  style: TextStyle(
                                      fontSize: 11,
                                      color: LauncherStyle.muted)),
                          ])),
                      TextButton(
                          onPressed: _busy ||
                                  !widget.controller.shortcutActions.any(
                                      (a) => a.actionId == binding.actionId)
                              ? null
                              : () => _edit(binding),
                          child: LauncherKeycap(binding.label)),
                      IconButton(
                          tooltip: 'Remove shortcut for ${binding.title}',
                          iconSize: 16,
                          onPressed: _busy
                              ? null
                              : () => _save((current) => current.copyWith(
                                  actionShortcuts: current.actionShortcuts
                                      .where(
                                          (b) => b.actionId != binding.actionId)
                                      .toList())),
                          icon: const Icon(Icons.close_rounded,
                              color: LauncherStyle.muted)),
                    ])),
              Align(
                  alignment: Alignment.centerLeft,
                  child: TextButton.icon(
                      onPressed:
                          _busy || widget.controller.shortcutActions.isEmpty
                              ? null
                              : () => _edit(),
                      icon: const Icon(Icons.add_rounded, size: 16),
                      label: const Text('Add action shortcut'))),
            ])),
        const SizedBox(height: 6),
        const Text(
            'Project scripts appear after opening Anywhere in that project. App menu shortcuts remain displayed by the app and cannot be saved here.',
            style: TextStyle(fontSize: 11, color: LauncherStyle.muted)),
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
  final _search = TextEditingController();
  ShortcutActionDto? _action;
  ActionShortcut? _binding;
  String? _error;
  bool _recording = false;
  @override
  void initState() {
    super.initState();
    _binding = widget.existing;
    for (final action in widget.options) {
      if (action.actionId == widget.existing?.actionId) _action = action;
    }
  }

  @override
  void dispose() {
    _focus.dispose();
    _search.dispose();
    super.dispose();
  }

  KeyEventResult _key(FocusNode node, KeyEvent event) {
    if (!_recording || event is! KeyDownEvent) return KeyEventResult.ignored;
    if (event.logicalKey == LogicalKeyboardKey.escape) {
      setState(() => _recording = false);
      return KeyEventResult.handled;
    }
    final code = carbonKeyCode(event.logicalKey);
    if (code == null) return KeyEventResult.handled;
    final action = _action!;
    final binding = ActionShortcut(
        actionId: action.actionId,
        title: action.title,
        keyCode: code,
        modifiers: pressedModifiers());
    final error = binding.validate(widget.settings);
    setState(() {
      _error = error;
      if (error == null) {
        _binding = binding;
        _recording = false;
      }
    });
    return KeyEventResult.handled;
  }

  @override
  Widget build(BuildContext context) {
    final query = _search.text.toLowerCase();
    final actions = widget.options
        .where((a) => '${a.title} ${a.contextTypes.join(' ')}'
            .toLowerCase()
            .contains(query))
        .toList();
    return AlertDialog(
      title: const Text('Action shortcut'),
      content: Focus(
          focusNode: _focus,
          onKeyEvent: _key,
          child: SizedBox(
              width: 420,
              height: 300,
              child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    if (_action == null) ...[
                      TextField(
                          controller: _search,
                          autofocus: true,
                          decoration: const InputDecoration(
                              hintText: 'Find an action…', isDense: true),
                          onChanged: (_) => setState(() {})),
                      const SizedBox(height: 8),
                      Expanded(
                          child: ListView(children: [
                        for (final action in actions)
                          ListTile(
                              dense: true,
                              title: Text(action.title),
                              subtitle: Text(action.contextTypes.join(' · ')),
                              onTap: () {
                                setState(() {
                                  _action = action;
                                  _binding = null;
                                });
                                _focus.requestFocus();
                              }),
                      ])),
                    ] else ...[
                      Text(_action!.title,
                          style: const TextStyle(fontWeight: FontWeight.w600)),
                      Text(_action!.contextTypes.join(' · '),
                          style: const TextStyle(
                              fontSize: 11, color: LauncherStyle.muted)),
                      const SizedBox(height: 16),
                      OutlinedButton(
                          onPressed: () {
                            setState(() {
                              _recording = true;
                              _error = null;
                            });
                            _focus.requestFocus();
                          },
                          child: Text(_recording
                              ? 'Press a shortcut…'
                              : (_binding?.label ?? 'Record shortcut'))),
                      const SizedBox(height: 8),
                      const Text(
                          'Use Command, Option or Control with a letter. Esc cancels recording.',
                          style: TextStyle(
                              fontSize: 12, color: LauncherStyle.muted)),
                      if (_error != null)
                        Padding(
                            padding: const EdgeInsets.only(top: 10),
                            child: Text(_error!,
                                style: const TextStyle(
                                    fontSize: 12,
                                    color: LauncherStyle.warning))),
                      const Spacer(),
                      if (widget.existing == null)
                        TextButton(
                            onPressed: () => setState(() {
                                  _action = null;
                                  _binding = null;
                                  _error = null;
                                }),
                            child: const Text('Choose another action')),
                    ],
                  ]))),
      actions: [
        TextButton(
            onPressed: () => Navigator.pop(context),
            child: const Text('Cancel')),
        FilledButton(
            onPressed: _binding == null ||
                    _action == null ||
                    _recording ||
                    _error != null
                ? null
                : () => Navigator.pop(context, _binding),
            child: const Text('Save shortcut')),
      ],
    );
  }
}
