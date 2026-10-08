import '../infrastructure/desktop_environment.dart';
import '../ui/app_icons.dart';
import 'dart:async';
import 'package:flutter/material.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';
import 'settings_controller.dart';
import 'settings_components.dart';
import 'action_shortcuts_section.dart';
import 'shortcut_recorder.dart';
import 'action_shortcut.dart';
import '../ui/app_style.dart';
import '../ui/app_components.dart';
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
  final _recorder = ShortcutRecorder();
  Future<bool>? _pending;
  bool _saving = false, _closing = false;
  bool _shortcutEditing = false;
  String _page = 'shortcuts';

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
    _recorder.cancel();
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
        _recorder.cancel();
        setState(() => _recording = null);
      } else {
        unawaited(widget.onClose());
      }
      return KeyEventResult.handled;
    }
    if (_recording == null) return KeyEventResult.ignored;
    if (_recorder.nativePending) return KeyEventResult.handled;
    final code = shortcutEventKeyCode(event);
    if (code == null) return KeyEventResult.ignored;
    _acceptChord(code, pressedModifiers());
    return KeyEventResult.handled;
  }

  bool _acceptChord(int code, int modifiers) {
    if (modifiers == 0) return false;
    final universal = _recording == 'universal';
    setState(() => _recording = null);
    _save((current) => universal
        ? current.copyWith(
            universalKeyCode: code,
            universalModifiers: modifiers,
          )
        : current.copyWith(
            currentAppKeyCode: code,
            currentAppModifiers: modifiers,
          ));
    return true;
  }

  List<String> _excludedIds() => [
        for (final line in _excluded.text.split(RegExp(r'[\n,]')))
          if (line.trim().isNotEmpty) line.trim(),
      ];

  Widget _sourceStatus(ContextSourceDto source) {
    final status = _controller.contextSourceStatuses
        .firstWhere((item) => item.source == source);
    return Tooltip(
      message: status.detail,
      child: Row(children: [
        Icon(
            status.attention > 0
                ? AppIcons.info
                : (status.capture?.status == ContextCaptureStatusDto.captured
                    ? AppIcons.success
                    : AppIcons.dot),
            size: 10,
            color: status.attention > 0 ? AppStyle.warning : AppStyle.muted),
        const SizedBox(width: 6),
        Expanded(
            child: Text(status.statusText,
                style: const TextStyle(fontSize: 10, color: AppStyle.muted))),
        if (status.settingsPane != null)
          TextButton(
            onPressed: () =>
                _controller.openPrivacySettings(status.settingsPane!),
            child: const Text('Open Settings', style: TextStyle(fontSize: 11)),
          ),
      ]),
    );
  }

  static const _pages = <(String, String, IconData)>[
    ('general', 'General', AppIcons.settings),
    ('shortcuts', 'Shortcuts', AppIcons.keyboard),
    ('context', 'Context', AppIcons.layers),
    ('search', 'Search', AppIcons.search),
    ('plugins', 'Plugins', AppIcons.plugin),
  ];

  void _selectPage(String id) {
    if (_page == id) return;
    _recorder.cancel();
    setState(() {
      _page = id;
      _recording = null;
    });
  }

  @override
  Widget build(BuildContext context) => Focus(
      autofocus: true,
      onKeyEvent: _onKey,
      child: AbsorbPointer(
          absorbing: _closing,
          child: Row(crossAxisAlignment: CrossAxisAlignment.stretch, children: [
            SizedBox(
                width: 164,
                child: ColoredBox(
                    color: AppStyle.sidebar,
                    child: Padding(
                        padding: const EdgeInsets.fromLTRB(10, 42, 10, 12),
                        child: Column(
                            key: const Key('settings-navigation'),
                            crossAxisAlignment: CrossAxisAlignment.stretch,
                            children: [
                              const Padding(
                                  padding: EdgeInsets.fromLTRB(8, 0, 0, 10),
                                  child: Text('Settings',
                                      style: AppStyle.caption)),
                              for (final page in _pages)
                                Padding(
                                    padding: const EdgeInsets.only(bottom: 3),
                                    child: AppSidebarItem(
                                        key: ValueKey('settings-${page.$1}'),
                                        label: page.$2,
                                        icon: page.$3,
                                        iconColor: switch (page.$1) {
                                          'general' => const Color(0xff707070),
                                          'shortcuts' =>
                                            const Color(0xff925acb),
                                          'context' => AppStyle.accentFill,
                                          'search' => const Color(0xff4c9b71),
                                          _ => const Color(0xffc6833e),
                                        },
                                        selected: _page == page.$1,
                                        onPressed: () => _selectPage(page.$1))),
                            ])))),
            const VerticalDivider(width: 1),
            Expanded(
                child: Column(
                    crossAxisAlignment: CrossAxisAlignment.stretch,
                    children: [
                  const SizedBox(height: 28),
                  AppToolbar(
                      title: _pages.firstWhere((p) => p.$1 == _page).$2,
                      actions: [
                        if (_saving) ...[
                          const SizedBox.square(
                              dimension: 12,
                              child:
                                  CircularProgressIndicator(strokeWidth: 1.5)),
                          const SizedBox(width: 6),
                          const Text('Saving…', style: AppStyle.caption),
                          const SizedBox(width: 8),
                        ],
                        OutlinedButton(
                            onPressed: widget.onClose,
                            child: const Text('Done')),
                      ]),
                  const Divider(),
                  Expanded(
                      child: Scrollbar(
                          child: ListView(
                              key: PageStorageKey('settings-content-$_page'),
                              primary: true,
                              padding:
                                  const EdgeInsets.fromLTRB(20, 18, 20, 20),
                              children: [
                        if (_controller.statusMessage != null) ...[
                          AppNotice(_controller.statusMessage!),
                          const SizedBox(height: 12),
                        ],
                        ..._pageContent(),
                      ]))),
                ])),
          ])));

  List<Widget> _pageContent() {
    final plugins = _controller.plugins;
    return switch (_page) {
      'shortcuts' => [
          _section('SHORTCUTS'),
          _card(
            child: Column(
              children: [
                _hotkeyRow('Current App', _settings.currentAppLabel, 'current'),
                const Divider(height: 1, color: AppStyle.divider),
                _hotkeyRow(
                    'Universal Search', _settings.universalLabel, 'universal'),
              ],
            ),
          ),
          const SizedBox(height: 6),
          const Text(
            'Choose a shortcut, then press the new key combination. Esc cancels.',
            style: TextStyle(fontSize: 11, color: AppStyle.muted),
          ),
          SizedBox(height: DesktopEnvironment.current.titleBarInset),
          ActionShortcutsSection(
              // Keep the editor owner alive when a status notice shifts the list.
              key: const ValueKey('action-shortcuts-section'),
              controller: _controller,
              onEditingChanged: (editing) {
                if (mounted) {
                  if (editing) _recorder.cancel();
                  setState(() {
                    _shortcutEditing = editing;
                    if (editing) _recording = null;
                  });
                }
              },
              save: (update) => _save(update)),
          SizedBox(height: DesktopEnvironment.current.titleBarInset),
        ],
      'general' => [
          _section('GENERAL'),
          _card(
              child: SettingsToggleRow(
                  title: 'Open at login',
                  description: 'Start quietly when you sign in',
                  icon: AppIcons.power,
                  value: _settings.launchAtLogin,
                  onChanged: (value) => _save(
                      (current) => current.copyWith(launchAtLogin: value)))),
        ],
      'context' => [
          _section('CONTEXT SOURCES'),
          const Text('Choose what Anywhere can use when you open it.',
              style: AppStyle.caption),
          const SizedBox(height: 10),
          _card(
              child: Column(children: [
            SettingsToggleRow(
                title: DesktopEnvironment.current.fileSelectionLabel,
                description: 'Selected files and folders',
                icon: AppIcons.folder,
                value: _settings.captureFinderSelection,
                status: _sourceStatus(ContextSourceDto.finderSelection),
                onChanged: (value) => _save((current) =>
                    current.copyWith(captureFinderSelection: value))),
            const Divider(),
            SettingsToggleRow(
                title: 'Selected text',
                description: 'Text selected in the current app',
                icon: AppIcons.quote,
                value: _settings.captureSelectedText,
                status: _sourceStatus(ContextSourceDto.selectedText),
                onChanged: (value) => _save(
                    (current) => current.copyWith(captureSelectedText: value))),
            const Divider(),
            SettingsToggleRow(
                title: 'Browser current page',
                description: DesktopEnvironment.current.browserDescription,
                icon: AppIcons.globe,
                value: _settings.captureBrowserCurrentPage,
                status: _sourceStatus(ContextSourceDto.browserCurrentPage),
                onChanged: (value) => _save((current) =>
                    current.copyWith(captureBrowserCurrentPage: value))),
            const Divider(),
            SettingsToggleRow(
                title: 'Clipboard text',
                description: 'Auto-fill new copies for 15 seconds',
                icon: AppIcons.clipboard,
                value: _settings.captureClipboard,
                status: _sourceStatus(ContextSourceDto.clipboard),
                onChanged: (value) => _save(
                    (current) => current.copyWith(captureClipboard: value))),
          ])),
          const SizedBox(height: 8),
          const Text(
              'Changes apply on the next wake. Status refers to the last capture.',
              style: AppStyle.caption),
        ],
      'search' => [
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
                          color: AppStyle.primary)),
                  const SizedBox(height: 3),
                  const Text(
                      'Enter one application ID per line to keep it out of search.',
                      style: TextStyle(fontSize: 11, color: AppStyle.muted)),
                  const SizedBox(height: 10),
                  TextField(
                    controller: _excluded,
                    minLines: 3,
                    maxLines: 5,
                    style: const TextStyle(
                        fontSize: 12,
                        fontFamily: 'monospace',
                        color: AppStyle.primary),
                    decoration: InputDecoration(
                      isDense: true,
                      hintText: DesktopEnvironment.current.applicationIdHint,
                      hintStyle: const TextStyle(color: AppStyle.muted),
                      filled: true,
                      fillColor: AppStyle.field,
                      border: OutlineInputBorder(
                          borderRadius:
                              BorderRadius.circular(AppStyle.controlRadius)),
                    ),
                    onSubmitted: (_) => _save(
                      (current) =>
                          current.copyWith(excludedApps: _excludedIds()),
                      excluded: true,
                    ),
                    onEditingComplete: () => _save(
                      (current) =>
                          current.copyWith(excludedApps: _excludedIds()),
                      excluded: true,
                    ),
                  ),
                  const SizedBox(height: 10),
                  Align(
                    alignment: Alignment.centerRight,
                    child: FilledButton(
                      onPressed: () => _save(
                        (current) =>
                            current.copyWith(excludedApps: _excludedIds()),
                        excluded: true,
                      ),
                      child: const Text('Save apps'),
                    ),
                  ),
                ],
              ),
            ),
          ),
          SizedBox(height: DesktopEnvironment.current.titleBarInset),
        ],
      'plugins' => [
          _section('PLUGINS'),
          _card(
            child: Padding(
              padding: const EdgeInsets.all(12),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  if (plugins.isEmpty)
                    Row(children: [
                      const Expanded(
                          child: Text('No plugins loaded.',
                              style: TextStyle(
                                  fontSize: 12, color: AppStyle.muted))),
                      _reloadPluginsButton(),
                    ])
                  else
                    for (final plugin in plugins)
                      Padding(
                        padding: const EdgeInsets.only(bottom: 6),
                        child: Row(
                          children: [
                            Icon(plugin.ok ? AppIcons.success : AppIcons.error,
                                size: 15,
                                color: plugin.ok
                                    ? AppStyle.accent
                                    : AppStyle.warning),
                            const SizedBox(width: 8),
                            Expanded(
                              child: Text('${plugin.id}  ${plugin.detail}',
                                  maxLines: 2,
                                  overflow: TextOverflow.ellipsis,
                                  style: TextStyle(
                                      fontSize: 12,
                                      color: plugin.ok
                                          ? AppStyle.secondary
                                          : AppStyle.warning)),
                            ),
                          ],
                        ),
                      ),
                  if (plugins.isNotEmpty)
                    Align(
                      alignment: Alignment.centerRight,
                      child: _reloadPluginsButton(),
                    ),
                ],
              ),
            ),
          ),
        ],
      _ => const [],
    };
  }

  Widget _section(String title) => AppSectionHeading(
      switch (title) {
        'SHORTCUTS' => 'Wake shortcuts',
        'GENERAL' => 'Startup',
        'CONTEXT SOURCES' => 'Context sources',
        'SEARCH' => 'Search results',
        'PLUGINS' => 'Extensions',
        _ => title,
      },
      icon: switch (title) {
        'SHORTCUTS' => AppIcons.keyboard,
        'GENERAL' => AppIcons.power,
        'CONTEXT SOURCES' => AppIcons.layers,
        'SEARCH' => AppIcons.search,
        _ => AppIcons.plugin,
      });

  Widget _card({required Widget child}) => AppCard(child: child);

  Widget _reloadPluginsButton() => OutlinedButton.icon(
      onPressed: _controller.refreshingPlugins
          ? null
          : () async {
              await _controller.refreshPlugins();
              if (mounted) setState(() {});
            },
      icon: const Icon(AppIcons.refresh, size: 14),
      label: const Text('Reload plugins'));

  Widget _hotkeyRow(String label, String value, String id) {
    final recording = _recording == id;
    return SizedBox(
      height: 42,
      child: Row(
        children: [
          const SizedBox(width: 12),
          Expanded(
            child: Text(
              label,
              style: const TextStyle(fontSize: 13, color: AppStyle.primary),
            ),
          ),
          OutlinedButton(
            onPressed: () {
              setState(() => _recording = id);
              _recorder.start(
                  onChord: _acceptChord,
                  onCancel: () {
                    if (mounted) setState(() => _recording = null);
                  },
                  onError: () {
                    if (mounted) {
                      setState(() => _recording = null);
                      _controller
                          .reportError('Shortcut recording failed. Try again.');
                    }
                  });
            },
            style: OutlinedButton.styleFrom(
              minimumSize: const Size(98, 26),
              side: BorderSide(
                  color: recording ? AppStyle.accent : AppStyle.divider),
              backgroundColor:
                  recording ? AppStyle.buttonSelection : Colors.transparent,
              shape: RoundedRectangleBorder(
                  borderRadius: BorderRadius.circular(8)),
            ),
            child: Text(
              recording ? 'Press a shortcut' : value,
              style: TextStyle(
                fontSize: 12,
                color: recording ? AppStyle.accent : AppStyle.secondary,
              ),
            ),
          ),
          const SizedBox(width: 12),
        ],
      ),
    );
  }
}
