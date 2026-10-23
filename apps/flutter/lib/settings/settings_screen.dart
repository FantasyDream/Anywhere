import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import '../infrastructure/desktop_environment.dart';
import '../l10n/app_language.dart';
import '../src/rust/api.dart';
import '../ui/app_components.dart';
import '../ui/app_icons.dart';
import '../ui/app_style.dart';
import 'action_shortcut.dart';
import 'action_shortcuts_section.dart';
import 'clipboard_settings_section.dart';
import 'features_section.dart';
import 'file_search_settings.dart';
import 'host_settings.dart';
import 'settings_components.dart';
import 'settings_controller.dart';
import 'shortcut_recorder.dart';

class SettingsScreen extends StatefulWidget {
  const SettingsScreen(
      {super.key, required this.controller, required this.onClose});
  final SettingsController controller;
  final Future<bool> Function() onClose;

  @override
  State<SettingsScreen> createState() => _SettingsScreenState();
}

class _SettingsScreenState extends State<SettingsScreen> {
  late final TextEditingController _excluded, _fileRoots, _fileExclusions;
  String? _recording;
  final _recorder = ShortcutRecorder();
  Future<bool>? _pending;
  bool _saving = false, _closing = false;
  bool _shortcutEditing = false;
  String _page = 'shortcuts';

  SettingsController get _controller => widget.controller;
  HostSettings get _settings => _controller.current;
  String? get _notice =>
      _controller.statusMessageFor(context.l10n) ??
      (_page == 'shortcuts'
          ? _controller.shortcutCatalogMessageFor(context.l10n)
          : null);

  @override
  void initState() {
    super.initState();
    _excluded = TextEditingController(text: _settings.excludedApps.join('\n'));
    _fileRoots =
        TextEditingController(text: _settings.fileSearchRoots.join('\n'));
    _fileExclusions = TextEditingController(
        text: _settings.fileSearchExcludedNames.join('\n'));
    _controller.addListener(_onSettings);
    _controller.beforeClose = _prepareClose;
  }

  void _onSettings() {
    if (mounted) setState(() {});
  }

  @override
  void dispose() {
    _controller.clipboard.setVisible(false);
    if (_controller.beforeClose == _prepareClose) {
      _controller.beforeClose = null;
    }
    _controller.removeListener(_onSettings);
    _recorder.cancel();
    _excluded.dispose();
    _fileRoots.dispose();
    _fileExclusions.dispose();
    super.dispose();
  }

  Future<bool> _save(
    HostSettings Function(HostSettings current) update, {
    bool excluded = false,
    bool fileSearch = false,
  }) {
    // Save the draft the user submitted, and keep any newer edits made while a
    // previous save or Host response is pending.
    final submittedApps = _excluded.text;
    final submittedRoots = _fileRoots.text;
    final submittedExclusions = _fileExclusions.text;
    Future<bool> run() async {
      try {
        var next = update(_controller.current);
        if (excluded) {
          next = next.copyWith(excludedApps: _parseLines(submittedApps));
        }
        if (fileSearch) {
          next = next.copyWith(
              fileSearchRoots: _parseLines(submittedRoots),
              fileSearchExcludedNames: _parseLines(submittedExclusions));
        }
        final saved = await _controller.save(next);
        if (excluded && saved && mounted && _excluded.text == submittedApps) {
          _excluded.text = _controller.current.excludedApps.join('\n');
        }
        if (fileSearch && saved && mounted) {
          if (_fileRoots.text == submittedRoots) {
            _fileRoots.text = _settings.fileSearchRoots.join('\n');
          }
          if (_fileExclusions.text == submittedExclusions) {
            _fileExclusions.text = _settings.fileSearchExcludedNames.join('\n');
          }
        }
        return saved;
      } catch (_) {
        if (!mounted) return false;
        _controller.reportError(context.l10n.settingsSaveFailed);
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

  List<String> _lines(TextEditingController field) => _parseLines(field.text);
  List<String> _parseLines(String text) => text
      .split('\n')
      .map((s) => s.trim())
      .where((s) => s.isNotEmpty)
      .toSet()
      .toList();

  bool get _hasSearchDrafts =>
      !listEquals(_excludedIds(), _settings.excludedApps) ||
      !listEquals(_lines(_fileRoots), _settings.fileSearchRoots) ||
      !listEquals(_lines(_fileExclusions), _settings.fileSearchExcludedNames);

  Future<bool> _prepareClose() async {
    if (!mounted) return false;
    if (_shortcutEditing) {
      _controller
          .reportError(context.l10n.finishOrCancelEditingTheActionShortcut);
      return false;
    }
    setState(() => _closing = true);
    try {
      final saved = await (_pending ?? Future<bool>.value(true));
      if (!mounted) return false;
      final dirty = _hasSearchDrafts;
      if (!dirty && saved) return true;
      final choice = await showDialog<String>(
        context: context,
        barrierDismissible: false,
        builder: (context) => AlertDialog(
          title: Text(dirty
              ? context.l10n.saveBeforeClose
              : context.l10n.changesNotSaved),
          content: Text(dirty
              ? context.l10n.searchDrafts
              : (_controller.statusMessageFor(context.l10n) ??
                  context.l10n.settingsCouldNotBeSavedYouCan)),
          actions: [
            TextButton(
                onPressed: () => Navigator.pop(context, 'cancel'),
                child: Text(context.l10n.keepEditing)),
            TextButton(
                onPressed: () => Navigator.pop(context, 'discard'),
                child: Text(context.l10n.closeWithoutSaving)),
            if (dirty)
              FilledButton(
                  onPressed: () => Navigator.pop(context, 'save'),
                  child: Text(context.l10n.saveChanges)),
          ],
        ),
      );
      if (choice == 'discard') return true;
      if (choice != 'save') return false;
      final applied = await _save(
          (current) => current.copyWith(
              excludedApps: _excludedIds(),
              fileSearchRoots: _lines(_fileRoots),
              fileSearchExcludedNames: _lines(_fileExclusions)),
          excluded: true,
          fileSearch: true);
      if (applied && mounted && _hasSearchDrafts) {
        _controller.reportError(context.l10n.newEditsWereMadeWhileSavingSave);
        return false;
      }
      return applied;
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

  Widget? _sourceStatus(ContextSourceDto source,
      {bool includeHealthy = false}) {
    final status = _controller.contextSourceStatuses
        .firstWhere((item) => item.source == source)
        .withLocalizations(context.l10n);
    if (!includeHealthy && status.attention == 0) return null;
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
            child:
                Text(context.l10n.openSettings, style: TextStyle(fontSize: 11)),
          ),
      ]),
    );
  }

  static const _pages = <(String, IconData)>[
    ('general', AppIcons.settings),
    ('shortcuts', AppIcons.keyboard),
    ('context', AppIcons.context),
    ('search', AppIcons.search),
    ('clipboard', AppIcons.clipboard),
    ('plugins', AppIcons.features),
  ];

  String _pageTitle(String id) => switch (id) {
        'general' => context.l10n.general,
        'shortcuts' => context.l10n.shortcuts,
        'context' => context.l10n.context,
        'search' => context.l10n.search,
        'clipboard' => context.l10n.clipboard,
        _ => context.l10n.features,
      };

  void _selectPage(String id) {
    if (_page == id) return;
    _controller.clipboard.setVisible(id == 'clipboard');
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
                              Padding(
                                  padding:
                                      const EdgeInsets.fromLTRB(8, 0, 0, 10),
                                  child: Text(context.l10n.settings,
                                      style: AppStyle.caption)),
                              for (final page in _pages)
                                Padding(
                                    padding: const EdgeInsets.only(bottom: 3),
                                    child: AppSidebarItem(
                                        key: ValueKey('settings-${page.$1}'),
                                        label: _pageTitle(page.$1),
                                        icon: page.$2,
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
                  AppToolbar(title: _pageTitle(_page), actions: [
                    if (_saving) ...[
                      const SizedBox.square(
                          dimension: 12,
                          child: CircularProgressIndicator(strokeWidth: 1.5)),
                      const SizedBox(width: 6),
                      Text(context.l10n.saving, style: AppStyle.caption),
                      const SizedBox(width: 8),
                    ],
                    OutlinedButton(
                        onPressed: widget.onClose,
                        child: Text(context.l10n.done)),
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
                        if (_notice case final message?) ...[
                          AppNotice(message),
                          const SizedBox(height: 12),
                        ],
                        ..._pageContent(),
                      ]))),
                ])),
          ])));

  List<Widget> _pageContent() {
    return switch (_page) {
      'shortcuts' => [
          _card(
            child: Column(
              children: [
                _hotkeyRow(context.l10n.currentApp, _settings.currentAppLabel,
                    'current'),
                const Divider(height: 1, color: AppStyle.divider),
                _hotkeyRow(context.l10n.universalSearch,
                    _settings.universalLabel, 'universal'),
              ],
            ),
          ),
          const SizedBox(height: 6),
          Text(
            context.l10n.shortcutHelp,
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
          _card(
              child: Padding(
                  padding: const EdgeInsets.all(12),
                  child: Row(children: [
                    Expanded(
                        child: Column(
                            crossAxisAlignment: CrossAxisAlignment.start,
                            children: [
                          Text(context.l10n.language, style: AppStyle.body),
                          const SizedBox(height: 3),
                          Text(context.l10n.languageDescription,
                              style: AppStyle.caption),
                        ])),
                    const SizedBox(width: 12),
                    DropdownButton<String>(
                      key: const Key('settings-language'),
                      icon: const Padding(
                          padding: EdgeInsets.only(left: 8),
                          child: Icon(AppIcons.chevronDown, size: 14)),
                      value: _settings.uiLanguage,
                      items: [
                        DropdownMenuItem(
                            value: 'system',
                            child: Text(context.l10n.languageSystem)),
                        DropdownMenuItem(
                            value: 'zh',
                            child: Text(context.l10n.languageChinese)),
                        DropdownMenuItem(
                            value: 'en',
                            child: Text(context.l10n.languageEnglish)),
                      ],
                      onChanged: _saving
                          ? null
                          : (language) {
                              if (language != null) {
                                _save((current) =>
                                    current.copyWith(uiLanguage: language));
                              }
                            },
                    ),
                  ]))),
          const SizedBox(height: 12),
          _card(
              child: SettingsToggleRow(
                  title: context.l10n.openAtLogin,
                  description: context.l10n.openAtLoginDescription,
                  icon: AppIcons.power,
                  value: _settings.launchAtLogin,
                  onChanged: (value) => _save(
                      (current) => current.copyWith(launchAtLogin: value)))),
        ],
      'context' => [
          _section(context.l10n.contextSources, AppIcons.context),
          Text(context.l10n.contextDescription, style: AppStyle.caption),
          const SizedBox(height: 10),
          _card(
              child: Column(children: [
            SettingsToggleRow(
                title: DesktopEnvironment.current.usesMetaCommand
                    ? context.l10n.finderSelection
                    : context.l10n.fileSelection,
                description: context.l10n.selectedFilesDescription,
                icon: AppIcons.folder,
                value: _settings.captureFinderSelection,
                status: _sourceStatus(ContextSourceDto.finderSelection),
                onChanged: (value) => _save((current) =>
                    current.copyWith(captureFinderSelection: value))),
            const Divider(),
            SettingsToggleRow(
                title: context.l10n.selectedText,
                description: context.l10n.selectedTextDescription,
                icon: AppIcons.quote,
                value: _settings.captureSelectedText,
                status: _sourceStatus(ContextSourceDto.selectedText),
                onChanged: (value) => _save(
                    (current) => current.copyWith(captureSelectedText: value))),
            const Divider(),
            SettingsToggleRow(
                title: context.l10n.browserCurrentPage,
                description: DesktopEnvironment.current.usesMetaCommand
                    ? context.l10n.browserPageDescription
                    : context.l10n.supportedBrowserDescription,
                icon: AppIcons.globe,
                value: _settings.captureBrowserCurrentPage,
                status: _sourceStatus(ContextSourceDto.browserCurrentPage),
                onChanged: (value) => _save((current) =>
                    current.copyWith(captureBrowserCurrentPage: value))),
          ])),
          const SizedBox(height: 8),
          AppDisclosure(
              title: context.l10n.sourceStatus,
              child: Column(
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  children: [
                    Text(context.l10n.changesApplyOnTheNextWakeStatus,
                        style: AppStyle.caption),
                    const SizedBox(height: 8),
                    for (final source in [
                      ContextSourceDto.finderSelection,
                      ContextSourceDto.selectedText,
                      ContextSourceDto.browserCurrentPage
                    ])
                      Padding(
                          padding: const EdgeInsets.only(bottom: 8),
                          child: Column(
                              crossAxisAlignment: CrossAxisAlignment.stretch,
                              children: [
                                Text(
                                    switch (source) {
                                      ContextSourceDto.finderSelection =>
                                        context.l10n.fileSelection,
                                      ContextSourceDto.selectedText =>
                                        context.l10n.selectedText,
                                      _ => context.l10n.browserCurrentPage
                                    },
                                    style: AppStyle.caption),
                                _sourceStatus(source, includeHealthy: true)!,
                              ])),
                  ])),
        ],
      'search' => [
          _card(
              child: FileSearchSettings(
                  enabled: _settings.fileSearchEnabled,
                  includeHidden: _settings.fileSearchIncludeHidden,
                  onHidden: (value) => _save((current) =>
                      current.copyWith(fileSearchIncludeHidden: value)),
                  roots: _fileRoots,
                  exclusions: _fileExclusions,
                  onAuthorize: !_settings.fileSearchEnabled ||
                          _saving ||
                          _controller.authorizingFiles
                      ? null
                      : _controller.authorizeFileSearch,
                  onDiskAccess: () =>
                      _controller.openPrivacySettings('fullDiskAccess'),
                  onToggle: (value) => _save(
                      (current) => current.copyWith(fileSearchEnabled: value)),
                  onSave: _saving
                      ? null
                      : () => _save(
                          (current) => current.copyWith(
                              fileSearchRoots: _lines(_fileRoots),
                              fileSearchExcludedNames: _lines(_fileExclusions)),
                          fileSearch: true))),
          const SizedBox(height: 14),
          _card(
            child: AppDisclosure(
              key: const Key('hidden-apps-disclosure'),
              title: context.l10n.hiddenApps,
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(context.l10n.enterOneApplicationIDPerLineTo,
                      style: TextStyle(fontSize: 11, color: AppStyle.muted)),
                  const SizedBox(height: 10),
                  TextField(
                    key: const Key('hidden-apps'),
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
                      child: Text(context.l10n.saveApps),
                    ),
                  ),
                ],
              ),
            ),
          ),
          SizedBox(height: DesktopEnvironment.current.titleBarInset),
        ],
      'clipboard' => [
          ClipboardSettingsSection(
              controller: _controller.clipboard,
              autoFill: _settings.captureClipboard,
              autoFillStatus: _sourceStatus(ContextSourceDto.clipboard),
              onAutoFill: _saving || _controller.clipboard.busy
                  ? null
                  : (value) => _save(
                      (current) => current.copyWith(captureClipboard: value)))
        ],
      'plugins' => [FeaturesSection(controller: _controller)],
      _ => const [],
    };
  }

  Widget _section(String title, IconData icon) =>
      AppSectionHeading(title, icon: icon);

  Widget _card({required Widget child}) => AppCard(child: child);

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
                      _controller.reportError(
                          context.l10n.shortcutRecordingFailedTryAgain);
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
              recording ? context.l10n.pressShortcutKey : value,
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
