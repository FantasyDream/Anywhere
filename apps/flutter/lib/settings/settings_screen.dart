import 'dart:async';
import 'package:flutter/material.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';
import 'settings_controller.dart';
import 'action_shortcuts_section.dart';
import '../launcher/launcher_style.dart';
import 'host_settings.dart';
import '../src/rust/api.dart';

class SettingsScreen extends StatefulWidget {
  const SettingsScreen(
      {super.key, required this.controller, required this.onClose});
  final SettingsController controller;
  final Future<bool> Function() onClose;

  @override
  State<SettingsScreen> createState() => _SettingsScreenState();
}

class _SettingsScreenState extends State<SettingsScreen> {
  late final TextEditingController _excluded;
  String? _recording;
  Future<bool>? _pending;
  bool _saving = false, _closing = false;
  bool _shortcutEditing = false;

  SettingsController get _controller => widget.controller;
  HostSettings get _settings => _controller.current;

  @override
  void initState() {
    super.initState();
    _excluded = TextEditingController(text: _settings.excludedApps.join('\n'));
    _controller.addListener(_onSettings);
    _controller.beforeClose = _prepareClose;
  }

  void _onSettings() {
    if (mounted) setState(() {});
  }

  @override
  void dispose() {
    if (_controller.beforeClose == _prepareClose) {
      _controller.beforeClose = null;
    }
    _controller.removeListener(_onSettings);
    _excluded.dispose();
    super.dispose();
  }

  Future<bool> _save(
    HostSettings Function(HostSettings current) update, {
    bool excluded = false,
  }) {
    Future<bool> run() async {
      try {
        final saved = await _controller.save(update(_controller.current));
        if (excluded && saved && mounted) {
          _excluded.text = _controller.current.excludedApps.join('\n');
        }
        return saved;
      } catch (_) {
        _controller.reportError('Settings could not be saved. Try again.');
        return false;
      }
    }

    final previous = _pending ?? Future<bool>.value(true);
    final next = previous.then((_) => run());
    _pending = next;
    setState(() => _saving = true);
    unawaited(next.then((_) {
      if (mounted && _pending == next) setState(() => _saving = false);
    }));
    return next;
  }

  Future<bool> _prepareClose() async {
    if (!mounted) return false;
    if (_shortcutEditing) {
      _controller.reportError(
          'Finish or cancel editing the action shortcut before closing.');
      return false;
    }
    setState(() => _closing = true);
    try {
      final saved = await (_pending ?? Future<bool>.value(true));
      if (!mounted) return false;
      final dirty = !listEquals(_excludedIds(), _settings.excludedApps);
      if (!dirty && saved) return true;
      final choice = await showDialog<String>(
        context: context,
        barrierDismissible: false,
        builder: (context) => AlertDialog(
          title: Text(dirty
              ? 'Save changes before closing?'
              : 'Some changes were not saved'),
          content: Text(dirty
              ? 'The hidden apps list has unapplied changes.'
              : (_controller.statusMessage ??
                  'Settings could not be saved. You can keep editing or close without saving.')),
          actions: [
            TextButton(
                onPressed: () => Navigator.pop(context, 'cancel'),
                child: const Text('Keep editing')),
            TextButton(
                onPressed: () => Navigator.pop(context, 'discard'),
                child: const Text('Close without saving')),
            if (dirty)
              FilledButton(
                  onPressed: () => Navigator.pop(context, 'save'),
                  child: const Text('Save changes')),
          ],
        ),
      );
      if (choice == 'discard') return true;
      if (choice != 'save') return false;
      return await _save(
          (current) => current.copyWith(excludedApps: _excludedIds()),
          excluded: true);
    } finally {
      if (mounted) setState(() => _closing = false);
    }
  }

  KeyEventResult _onKey(FocusNode node, KeyEvent event) {
    if (event is! KeyDownEvent) {
      return KeyEventResult.ignored;
    }
    if (event.logicalKey == LogicalKeyboardKey.escape) {
      if (_recording != null) {
        setState(() => _recording = null);
      } else {
        unawaited(widget.onClose());
      }
      return KeyEventResult.handled;
    }
    if (_recording == null) return KeyEventResult.ignored;
    final code = carbonKeyCode(event.logicalKey);
    if (code == null) return KeyEventResult.ignored;
    var modifiers = 0;
    final keys = HardwareKeyboard.instance.logicalKeysPressed;
    if (keys.contains(LogicalKeyboardKey.metaLeft) ||
        keys.contains(LogicalKeyboardKey.metaRight)) {
      modifiers |= HostSettings.cmdKey;
    }
    if (keys.contains(LogicalKeyboardKey.altLeft) ||
        keys.contains(LogicalKeyboardKey.altRight)) {
      modifiers |= HostSettings.optionKey;
    }
    if (keys.contains(LogicalKeyboardKey.shiftLeft) ||
        keys.contains(LogicalKeyboardKey.shiftRight)) {
      modifiers |= HostSettings.shiftKey;
    }
    if (keys.contains(LogicalKeyboardKey.controlLeft) ||
        keys.contains(LogicalKeyboardKey.controlRight)) {
      modifiers |= HostSettings.controlKey;
    }
    if (modifiers == 0) return KeyEventResult.handled;
    final universal = _recording == 'universal';
    _recording = null;
    _save((current) => universal
        ? current.copyWith(
            universalKeyCode: code,
            universalModifiers: modifiers,
          )
        : current.copyWith(
            currentAppKeyCode: code,
            currentAppModifiers: modifiers,
          ));
    return KeyEventResult.handled;
  }

  List<String> _excludedIds() => [
        for (final line in _excluded.text.split(RegExp(r'[\n,]')))
          if (line.trim().isNotEmpty) line.trim(),
      ];

  Widget _sourceStatus(ContextSourceDto source) {
    final status = _controller.contextSourceStatuses
        .firstWhere((item) => item.source == source);
    return Padding(
      padding: const EdgeInsets.fromLTRB(12, 0, 12, 8),
      child: Tooltip(
        message: status.detail,
        child: Row(children: [
          Icon(status.attention > 0 ? Icons.info_outline_rounded : Icons.circle,
              size: 12,
              color: status.attention > 0
                  ? LauncherStyle.warning
                  : LauncherStyle.muted),
          const SizedBox(width: 6),
          Expanded(
              child: Text(status.statusText,
                  style: const TextStyle(
                      fontSize: 11, color: LauncherStyle.muted))),
          if (status.settingsPane != null)
            TextButton(
              onPressed: () =>
                  _controller.openPrivacySettings(status.settingsPane!),
              child:
                  const Text('Open Settings', style: TextStyle(fontSize: 11)),
            ),
        ]),
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    final plugins = _controller.plugins;
    return Focus(
      autofocus: true,
      onKeyEvent: _onKey,
      child: AbsorbPointer(
        absorbing: _closing,
        child: Padding(
          padding: const EdgeInsets.fromLTRB(20, 46, 20, 12),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Row(
                children: [
                  Container(
                    width: 34,
                    height: 34,
                    decoration: BoxDecoration(
                      color: const Color(0x337e98f2),
                      borderRadius: BorderRadius.circular(11),
                      border: Border.all(color: const Color(0x559eb4ff)),
                    ),
                    child: const Icon(Icons.tune_rounded,
                        size: 18, color: LauncherStyle.primary),
                  ),
                  const SizedBox(width: 11),
                  const Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text('Settings',
                          style: TextStyle(
                              fontSize: 16,
                              fontWeight: FontWeight.w600,
                              color: LauncherStyle.primary)),
                      Text('Make Anywhere work your way',
                          style: TextStyle(
                              fontSize: 11, color: LauncherStyle.muted)),
                    ],
                  ),
                  const Spacer(),
                  if (_saving) ...[
                    const Text('Saving…',
                        style: TextStyle(
                            fontSize: 11, color: LauncherStyle.muted)),
                    const SizedBox(width: 8),
                  ],
                  TextButton(
                    onPressed: widget.onClose,
                    style: TextButton.styleFrom(
                        foregroundColor: LauncherStyle.primary),
                    child: const Text('Done'),
                  ),
                ],
              ),
              if (_controller.statusMessage != null) ...[
                const SizedBox(height: 8),
                Container(
                  width: double.infinity,
                  padding: const EdgeInsets.all(10),
                  decoration: BoxDecoration(
                    color: const Color(0x2ce5b779),
                    borderRadius: BorderRadius.circular(9),
                  ),
                  child: Text(
                    _controller.statusMessage!,
                    style: const TextStyle(
                        fontSize: 12, color: LauncherStyle.warning),
                  ),
                ),
              ],
              const SizedBox(height: 18),
              Expanded(
                child: ListView(
                  padding: EdgeInsets.zero,
                  children: [
                    _section('SHORTCUTS'),
                    _card(
                      child: Column(
                        children: [
                          _hotkeyRow('Current App', _settings.currentAppLabel,
                              'current'),
                          const Divider(
                              height: 1, color: LauncherStyle.divider),
                          _hotkeyRow('Universal Search',
                              _settings.universalLabel, 'universal'),
                        ],
                      ),
                    ),
                    const SizedBox(height: 6),
                    const Text(
                      'Choose a shortcut, then press the new key combination. Esc cancels.',
                      style:
                          TextStyle(fontSize: 11, color: LauncherStyle.muted),
                    ),
                    const SizedBox(height: 18),
                    ActionShortcutsSection(
                        controller: _controller,
                        onEditingChanged: (editing) {
                          if (mounted) _shortcutEditing = editing;
                        },
                        save: (update) => _save(update)),
                    const SizedBox(height: 18),
                    _section('GENERAL'),
                    _card(
                      child: SwitchListTile.adaptive(
                        contentPadding:
                            const EdgeInsets.symmetric(horizontal: 12),
                        dense: true,
                        title: const Text('Open at login',
                            style: TextStyle(
                                fontSize: 13, color: LauncherStyle.primary)),
                        subtitle: const Text('Start quietly when you sign in',
                            style: TextStyle(
                                fontSize: 11, color: LauncherStyle.muted)),
                        value: _settings.launchAtLogin,
                        onChanged: (value) => _save(
                          (current) => current.copyWith(launchAtLogin: value),
                        ),
                      ),
                    ),
                    const SizedBox(height: 18),
                    _section('CONTEXT SOURCES'),
                    _card(
                      child: Column(
                        children: [
                          SwitchListTile.adaptive(
                            contentPadding:
                                const EdgeInsets.symmetric(horizontal: 12),
                            dense: true,
                            title: const Text('Finder selection',
                                style: TextStyle(
                                    fontSize: 13,
                                    color: LauncherStyle.primary)),
                            subtitle: const Text(
                                'Use selected files and folders',
                                style: TextStyle(
                                    fontSize: 11, color: LauncherStyle.muted)),
                            value: _settings.captureFinderSelection,
                            onChanged: (value) => _save((current) => current
                                .copyWith(captureFinderSelection: value)),
                          ),
                          _sourceStatus(ContextSourceDto.finderSelection),
                          const Divider(
                              height: 1, color: LauncherStyle.divider),
                          SwitchListTile.adaptive(
                            contentPadding:
                                const EdgeInsets.symmetric(horizontal: 12),
                            dense: true,
                            title: const Text('Selected text',
                                style: TextStyle(
                                    fontSize: 13,
                                    color: LauncherStyle.primary)),
                            subtitle: const Text(
                                'Use text selected in the current app',
                                style: TextStyle(
                                    fontSize: 11, color: LauncherStyle.muted)),
                            value: _settings.captureSelectedText,
                            onChanged: (value) => _save((current) =>
                                current.copyWith(captureSelectedText: value)),
                          ),
                          _sourceStatus(ContextSourceDto.selectedText),
                          const Divider(
                              height: 1, color: LauncherStyle.divider),
                          SwitchListTile.adaptive(
                            contentPadding:
                                const EdgeInsets.symmetric(horizontal: 12),
                            dense: true,
                            title: const Text('Browser current page',
                                style: TextStyle(
                                    fontSize: 13,
                                    color: LauncherStyle.primary)),
                            subtitle: const Text(
                                'Use the active Safari, Chrome, or Edge tab’s web address',
                                style: TextStyle(
                                    fontSize: 11, color: LauncherStyle.muted)),
                            value: _settings.captureBrowserCurrentPage,
                            onChanged: (value) => _save((current) => current
                                .copyWith(captureBrowserCurrentPage: value)),
                          ),
                          _sourceStatus(ContextSourceDto.browserCurrentPage),
                          const Divider(
                              height: 1, color: LauncherStyle.divider),
                          SwitchListTile.adaptive(
                            contentPadding:
                                const EdgeInsets.symmetric(horizontal: 12),
                            dense: true,
                            title: const Text('Clipboard text',
                                style: TextStyle(
                                    fontSize: 13,
                                    color: LauncherStyle.primary)),
                            subtitle: const Text(
                                'Read once when Anywhere opens',
                                style: TextStyle(
                                    fontSize: 11, color: LauncherStyle.muted)),
                            value: _settings.captureClipboard,
                            onChanged: (value) => _save((current) =>
                                current.copyWith(captureClipboard: value)),
                          ),
                          _sourceStatus(ContextSourceDto.clipboard),
                        ],
                      ),
                    ),
                    const SizedBox(height: 6),
                    const Text(
                      'Changes apply on the next wake. Status shows the last captured snapshot.',
                      style:
                          TextStyle(fontSize: 11, color: LauncherStyle.muted),
                    ),
                    const SizedBox(height: 18),
                    _section('SEARCH'),
                    _card(
                      child: Padding(
                        padding: const EdgeInsets.all(12),
                        child: Column(
                          crossAxisAlignment: CrossAxisAlignment.start,
                          children: [
                            const Text('Hidden apps',
                                style: TextStyle(
                                    fontSize: 13,
                                    fontWeight: FontWeight.w600,
                                    color: LauncherStyle.primary)),
                            const SizedBox(height: 3),
                            const Text(
                                'Enter one app bundle ID per line to keep it out of search.',
                                style: TextStyle(
                                    fontSize: 11, color: LauncherStyle.muted)),
                            const SizedBox(height: 10),
                            TextField(
                              controller: _excluded,
                              minLines: 3,
                              maxLines: 5,
                              style: const TextStyle(
                                  fontSize: 12,
                                  fontFamily: 'monospace',
                                  color: LauncherStyle.primary),
                              decoration: InputDecoration(
                                isDense: true,
                                hintText: 'com.apple.Safari',
                                hintStyle:
                                    const TextStyle(color: LauncherStyle.muted),
                                filled: true,
                                fillColor: const Color(0x3010141d),
                                border: OutlineInputBorder(
                                    borderRadius: BorderRadius.circular(9)),
                              ),
                              onSubmitted: (_) => _save(
                                (current) => current.copyWith(
                                    excludedApps: _excludedIds()),
                                excluded: true,
                              ),
                              onEditingComplete: () => _save(
                                (current) => current.copyWith(
                                    excludedApps: _excludedIds()),
                                excluded: true,
                              ),
                            ),
                            Align(
                              alignment: Alignment.centerRight,
                              child: TextButton.icon(
                                onPressed: () => _save(
                                  (current) => current.copyWith(
                                      excludedApps: _excludedIds()),
                                  excluded: true,
                                ),
                                icon: const Icon(Icons.check_rounded, size: 15),
                                label: const Text('Save apps'),
                              ),
                            ),
                          ],
                        ),
                      ),
                    ),
                    const SizedBox(height: 18),
                    _section('PLUGINS'),
                    _card(
                      child: Padding(
                        padding: const EdgeInsets.fromLTRB(12, 10, 12, 4),
                        child: Column(
                          crossAxisAlignment: CrossAxisAlignment.start,
                          children: [
                            if (plugins.isEmpty)
                              const Text('No plugins loaded.',
                                  style: TextStyle(
                                      fontSize: 12, color: LauncherStyle.muted))
                            else
                              for (final plugin in plugins)
                                Padding(
                                  padding: const EdgeInsets.only(bottom: 6),
                                  child: Row(
                                    children: [
                                      Icon(
                                          plugin.ok
                                              ? Icons
                                                  .check_circle_outline_rounded
                                              : Icons.error_outline_rounded,
                                          size: 15,
                                          color: plugin.ok
                                              ? LauncherStyle.accent
                                              : LauncherStyle.warning),
                                      const SizedBox(width: 8),
                                      Expanded(
                                        child: Text(
                                            '${plugin.id}  ${plugin.detail}',
                                            maxLines: 2,
                                            overflow: TextOverflow.ellipsis,
                                            style: TextStyle(
                                                fontSize: 12,
                                                color: plugin.ok
                                                    ? LauncherStyle.secondary
                                                    : LauncherStyle.warning)),
                                      ),
                                    ],
                                  ),
                                ),
                            Align(
                              alignment: Alignment.centerRight,
                              child: TextButton.icon(
                                onPressed: () async {
                                  await _controller.refreshPlugins();
                                  if (mounted) setState(() {});
                                },
                                icon:
                                    const Icon(Icons.refresh_rounded, size: 15),
                                label: const Text('Reload plugins'),
                              ),
                            ),
                          ],
                        ),
                      ),
                    ),
                  ],
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }

  Widget _section(String title) => Padding(
        padding: const EdgeInsets.only(left: 3, bottom: 8),
        child: Text(
          title,
          style: const TextStyle(
            fontSize: 11,
            fontWeight: FontWeight.w600,
            letterSpacing: 1.0,
            color: LauncherStyle.secondary,
          ),
        ),
      );

  Widget _card({required Widget child}) => SizedBox(
        width: double.infinity,
        child: Material(
          color: const Color(0x24ffffff),
          shape: RoundedRectangleBorder(
            borderRadius: BorderRadius.circular(12),
            side: const BorderSide(color: Color(0x2effffff), width: 0.8),
          ),
          clipBehavior: Clip.antiAlias,
          child: child,
        ),
      );

  Widget _hotkeyRow(String label, String value, String id) {
    final recording = _recording == id;
    return SizedBox(
      height: 48,
      child: Row(
        children: [
          const SizedBox(width: 12),
          Expanded(
            child: Text(
              label,
              style:
                  const TextStyle(fontSize: 13, color: LauncherStyle.primary),
            ),
          ),
          OutlinedButton(
            onPressed: () => setState(() => _recording = id),
            style: OutlinedButton.styleFrom(
              minimumSize: const Size(125, 30),
              side: BorderSide(
                  color:
                      recording ? LauncherStyle.accent : LauncherStyle.divider),
              backgroundColor:
                  recording ? const Color(0x337e98f2) : const Color(0x2210141d),
              shape: RoundedRectangleBorder(
                  borderRadius: BorderRadius.circular(8)),
            ),
            child: Text(
              recording ? 'Press a shortcut' : value,
              style: TextStyle(
                fontSize: 12,
                color:
                    recording ? LauncherStyle.accent : LauncherStyle.secondary,
              ),
            ),
          ),
          const SizedBox(width: 12),
        ],
      ),
    );
  }
}
