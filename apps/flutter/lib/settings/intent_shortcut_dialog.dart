import '../ui/app_icons.dart';
import 'dart:math';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import '../ui/app_style.dart';
import '../ui/app_components.dart';
import '../src/rust/api.dart';
import 'action_shortcut.dart';
import 'host_settings.dart';
import 'shortcut_recorder.dart';
import 'shortcut_app_picker.dart';

/// An intent has one chord and explicit rules. Rule order has no meaning.
class IntentShortcutDialog extends StatefulWidget {
  const IntentShortcutDialog(
      {super.key,
      required this.actions,
      required this.applications,
      required this.settings,
      this.existing});
  final List<ShortcutActionDto> actions;
  final List<ShortcutApplicationDto> applications;
  final HostSettings settings;
  final ActionShortcut? existing;
  @override
  State<IntentShortcutDialog> createState() => _IntentShortcutDialogState();
}

class _IntentShortcutDialogState extends State<IntentShortcutDialog> {
  final _focus = FocusNode();
  final _recorder = ShortcutRecorder();
  late final TextEditingController _name;
  late final String _id;
  late List<ShortcutRoute> _rules;
  int? _code, _modifiers;
  bool _global = true, _recording = false;
  String? _error;

  @override
  void initState() {
    super.initState();
    final saved = widget.existing;
    _name = TextEditingController(text: saved?.title ?? '');
    final random = Random.secure();
    _id = saved?.actionId ??
        'intent:${List.generate(16, (_) => random.nextInt(256).toRadixString(16).padLeft(2, '0')).join()}';
    _rules = [...?saved?.routes];
    _code = saved?.keyCode;
    _modifiers = saved?.modifiers;
    _global = saved?.global ?? true;
  }

  @override
  void dispose() {
    _recorder.cancel();
    _focus.dispose();
    _name.dispose();
    super.dispose();
  }

  ActionShortcut _draft({int? code, int? modifiers}) => ActionShortcut(
      actionId: _id,
      title: _name.text.trim(),
      keyCode: code ?? _code ?? -1,
      modifiers: modifiers ?? _modifiers ?? 0,
      global: _global,
      routes: _rules);

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
    final error =
        _draft(code: code, modifiers: modifiers).validateChord(widget.settings);
    setState(() {
      _error = error;
      if (error == null) {
        _code = code;
        _modifiers = modifiers;
        _recording = false;
      }
    });
    return error == null;
  }

  Future<void> _editRule([int? index]) async {
    _recorder.cancel();
    setState(() {
      _recording = false;
      _error = null;
    });
    final rule = await showDialog<ShortcutRoute>(
        context: context,
        builder: (_) => _RuleDialog(
                actions: widget.actions,
                applications: widget.applications,
                existing: index == null ? null : _rules[index],
                otherRules: [
                  for (var i = 0; i < _rules.length; i++)
                    if (i != index) _rules[i]
                ]));
    if (!mounted || rule == null) return;
    setState(() {
      if (index == null) {
        _rules.add(rule);
      } else {
        _rules[index] = rule;
      }
      _error = null;
    });
  }

  void _save() {
    final draft = _draft();
    final error = draft.validate(widget.settings);
    if (_rules.isEmpty) {
      setState(() => _error = 'Add at least one rule.');
      return;
    }
    if (error != null) {
      setState(() => _error = error);
      return;
    }
    Navigator.pop(context, draft);
  }

  @override
  Widget build(BuildContext context) => Dialog(
      insetPadding: const EdgeInsets.symmetric(horizontal: 24, vertical: 12),
      child: Padding(
          padding: const EdgeInsets.all(16),
          child: Focus(
              focusNode: _focus,
              onKeyEvent: _key,
              child: SizedBox(
                  width: 440,
                  height: min(
                      MediaQuery.textScalerOf(context)
                          .scale(_rules.isEmpty ? 315 : 360),
                      MediaQuery.sizeOf(context).height - 80),
                  child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        const Text('Context shortcut',
                            style: TextStyle(
                                fontSize: 15, fontWeight: FontWeight.w600)),
                        const SizedBox(height: 12),
                        const Text('Shortcut name', style: AppStyle.caption),
                        const SizedBox(height: 4),
                        TextField(
                            key: const ValueKey('context-shortcut-name'),
                            controller: _name,
                            autofocus: widget.existing == null,
                            decoration: const InputDecoration(
                                hintText: 'e.g. Open terminal', isDense: true)),
                        const SizedBox(height: 10),
                        Row(children: [
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
                                          _error =
                                              'Shortcut recording failed. Try again.';
                                        });
                                      }
                                    });
                                _focus.requestFocus();
                              },
                              child: Text(_recording
                                  ? 'Press a shortcut…'
                                  : (_code == null
                                      ? 'Record shortcut'
                                      : _draft().label))),
                          const Spacer(),
                          const Text('Global', style: TextStyle(fontSize: 12)),
                          AppToggle(
                              label: 'Global shortcut',
                              value: _global,
                              onChanged: _recording
                                  ? null
                                  : (v) => setState(() => _global = v)),
                        ]),
                        const SizedBox(height: 6),
                        const Text(
                            'More specific conditions take priority. Equal matches let you choose. An unavailable matching action will not fall back.',
                            style:
                                TextStyle(fontSize: 11, color: AppStyle.muted)),
                        const SizedBox(height: 6),
                        Expanded(
                            child: ListView(children: [
                          if (_rules.isEmpty)
                            const Padding(
                                padding: EdgeInsets.symmetric(vertical: 8),
                                child: AppNotice(
                                    'No rules yet. Add an action and choose the app or context where it should run.',
                                    kind: NoticeKind.info)),
                          for (var i = 0; i < _rules.length; i++)
                            ListTile(
                                dense: true,
                                contentPadding: EdgeInsets.zero,
                                title: Text(_rules[i].title,
                                    maxLines: 1,
                                    overflow: TextOverflow.ellipsis),
                                subtitle: Text(_rules[i].conditionLabel,
                                    maxLines: 2, style: AppStyle.caption),
                                onTap: () => _editRule(i),
                                trailing: IconButton(
                                    tooltip: 'Remove rule ${i + 1}',
                                    icon: const Icon(AppIcons.close, size: 16),
                                    onPressed: () => setState(() {
                                          _rules.removeAt(i);
                                          _error = null;
                                        }))),
                          Align(
                              alignment: Alignment.centerLeft,
                              child: OutlinedButton.icon(
                                  onPressed: _rules.length >= 16
                                      ? null
                                      : () => _editRule(),
                                  icon: const Icon(AppIcons.add, size: 16),
                                  label: const Text('Add rule'))),
                        ])),
                        if (_error != null)
                          Text(_error!,
                              style: const TextStyle(
                                  fontSize: 11, color: AppStyle.warning)),
                        SizedBox(
                            width: double.infinity,
                            child: Wrap(
                                alignment: WrapAlignment.end,
                                spacing: 8,
                                runSpacing: 6,
                                children: [
                                  TextButton(
                                      onPressed: () => Navigator.pop(context),
                                      child: const Text('Cancel')),
                                  FilledButton(
                                      onPressed: _recording ? null : _save,
                                      child:
                                          const Text('Save context shortcut')),
                                ])),
                      ])))));
}

class _RuleDialog extends StatefulWidget {
  const _RuleDialog(
      {required this.actions,
      required this.applications,
      required this.otherRules,
      this.existing});
  final List<ShortcutActionDto> actions;
  final List<ShortcutApplicationDto> applications;
  final List<ShortcutRoute> otherRules;
  final ShortcutRoute? existing;
  @override
  State<_RuleDialog> createState() => _RuleDialogState();
}

class _RuleDialogState extends State<_RuleDialog> {
  final _search = TextEditingController();
  ShortcutActionDto? _action;
  String? _app, _context, _error;
  @override
  void initState() {
    super.initState();
    final saved = widget.existing;
    if (saved != null) {
      _action = widget.actions
              .where((a) => a.actionId == saved.actionId)
              .firstOrNull ??
          ShortcutActionDto(
              actionId: saved.actionId,
              title: saved.title,
              contextTypes: [
                if (saved.contextType != null) saved.contextType!
              ]);
      _app = saved.appBundleId;
      _context = saved.contextType;
    }
  }

  @override
  void dispose() {
    _search.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final apps = [...widget.applications];
    if (_app != null && !apps.any((a) => a.bundleId == _app)) {
      apps.add(ShortcutApplicationDto(
          bundleId: _app!, title: widget.existing?.appName ?? 'Saved app'));
    }
    final types = <String>{
      // The host capability is an Action target, not captured ambient Context.
      ...?_action?.contextTypes.where((type) => type != 'core.launcher'),
      if (_context != null) _context!
    };
    return AlertDialog(
        scrollable: _action != null,
        title: const Text('Shortcut rule'),
        content: SizedBox(
            width: 420,
            height: _action == null ? 270 : null,
            child: Column(
                mainAxisSize: MainAxisSize.min,
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  if (_action == null) ...[
                    TextField(
                        controller: _search,
                        autofocus: true,
                        decoration: const InputDecoration(
                            hintText: 'Find an action…',
                            prefixIcon: Icon(AppIcons.search, size: 17),
                            isDense: true),
                        onChanged: (_) => setState(() {})),
                    Expanded(
                        child: ListView(children: [
                      for (final a in widget.actions.where((a) =>
                          '${a.title} ${a.contextTypes.join(' ')}'
                              .toLowerCase()
                              .contains(_search.text.toLowerCase())))
                        ListTile(
                            dense: true,
                            title: Text(a.title),
                            subtitle: Text(
                                a.contextTypes
                                    .map(contextTypeLabel)
                                    .join(' · '),
                                style: AppStyle.caption),
                            onTap: () => setState(() {
                                  _action = a;
                                  _context = null;
                                  _error = null;
                                })),
                    ])),
                  ] else ...[
                    Text(_action!.title,
                        style: const TextStyle(fontWeight: FontWeight.w600)),
                    const SizedBox(height: 10),
                    const Text('In app',
                        style: TextStyle(fontSize: 11, color: AppStyle.muted)),
                    const SizedBox(height: 4),
                    OutlinedButton(
                        key: const ValueKey('rule-app'),
                        onPressed: () async {
                          final choice =
                              await showDialog<ShortcutApplicationChoice>(
                                  context: context,
                                  builder: (_) => ShortcutAppPicker(
                                      applications: apps, selectedId: _app));
                          if (!mounted || choice == null) return;
                          setState(() {
                            _app = choice.app?.bundleId;
                            _error = null;
                          });
                        },
                        child: Row(children: [
                          Expanded(
                              child: Text(
                                  _app == null
                                      ? 'Any app'
                                      : apps
                                          .firstWhere((a) => a.bundleId == _app)
                                          .title,
                                  maxLines: 1,
                                  overflow: TextOverflow.ellipsis)),
                          const Icon(AppIcons.search, size: 16),
                        ])),
                    const SizedBox(height: 12),
                    const Text('With context',
                        style: TextStyle(fontSize: 11, color: AppStyle.muted)),
                    const SizedBox(height: 4),
                    DropdownButtonFormField<String>(
                        key: const ValueKey('rule-context'),
                        decoration: const InputDecoration(isDense: true),
                        style: const TextStyle(
                            fontSize: 12, height: 1.2, color: AppStyle.primary),
                        isExpanded: true,
                        initialValue: _context ?? '',
                        items: [
                          const DropdownMenuItem(
                              value: '', child: Text('Any compatible context')),
                          for (final ty in types)
                            DropdownMenuItem(
                                value: ty, child: Text(contextTypeLabel(ty)))
                        ],
                        onChanged: (v) => setState(() {
                              _context = v == '' ? null : v;
                              _error = null;
                            })),
                    if (_error != null)
                      Text(_error!,
                          style: const TextStyle(
                              fontSize: 11, color: AppStyle.warning)),
                    const SizedBox(height: 10),
                    TextButton(
                        onPressed: () => setState(() {
                              _action = null;
                              _context = null;
                              _error = null;
                            }),
                        child: const Text('Choose another action')),
                  ],
                ])),
        actions: [
          TextButton(
              onPressed: () => Navigator.pop(context),
              child: const Text('Cancel')),
          FilledButton(
              onPressed: _action == null
                  ? null
                  : () {
                      if (widget.otherRules.any((r) =>
                          r.appBundleId == _app && r.contextType == _context)) {
                        setState(() =>
                            _error = 'A rule already uses these conditions.');
                        return;
                      }
                      Navigator.pop(
                          context,
                          ShortcutRoute(
                              actionId: _action!.actionId,
                              title: _action!.title,
                              appBundleId: _app,
                              appName: apps
                                  .where((a) => a.bundleId == _app)
                                  .firstOrNull
                                  ?.title,
                              contextType: _context));
                    },
              child: const Text('Save rule')),
        ]);
  }
}
