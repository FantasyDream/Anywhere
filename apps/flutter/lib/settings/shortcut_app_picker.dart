import 'dart:math';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import '../infrastructure/bridge_api.dart';
import '../l10n/app_language.dart';
import '../l10n/localized_label.dart';
import '../ui/app_components.dart';
import '../ui/app_icons.dart';
import '../ui/app_style.dart';

/// A null app means Any app; a null dialog result means Cancel.
class ShortcutApplicationChoice {
  const ShortcutApplicationChoice(this.app);
  final ShortcutApplicationDto? app;
}

class ShortcutAppPicker extends StatefulWidget {
  const ShortcutAppPicker(
      {super.key, required this.applications, this.selectedId});
  final List<ShortcutApplicationDto> applications;
  final String? selectedId;
  @override
  State<ShortcutAppPicker> createState() => _ShortcutAppPickerState();
}

class _ShortcutAppPickerState extends State<ShortcutAppPicker> {
  final _search = TextEditingController();
  final _scroll = ScrollController();
  int _selected = 0;
  String? _matchedQuery;
  late List<ShortcutApplicationDto> _matches;

  @override
  void initState() {
    super.initState();
    _filterApplications();
  }

  @override
  void didUpdateWidget(ShortcutAppPicker oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (identical(oldWidget.applications, widget.applications)) return;
    final selectedId = _selected == 0 ? null : _matches[_selected - 1].bundleId;
    _matchedQuery = null;
    _filterApplications();
    final index = _matches.indexWhere((app) => app.bundleId == selectedId);
    _selected = index < 0 ? 0 : index + 1;
  }

  double get _rowHeight =>
      max(44, MediaQuery.textScalerOf(context).scale(30) + 12);

  void _filterApplications() {
    final query = _search.text.trim().toLowerCase();
    if (_matchedQuery == query) return;
    _matchedQuery = query;
    // Navigation and focus changes reuse the matches. Empty search shares the
    // catalogue instead of copying all entries on every arrow-key rebuild.
    _matches = query.isEmpty
        ? widget.applications
        : List.unmodifiable(widget.applications.where((app) =>
            '${app.title} ${app.localizedTitles.values.join(' ')} ${app.bundleId}'
                .toLowerCase()
                .contains(query)));
  }

  @override
  void dispose() {
    _search.dispose();
    _scroll.dispose();
    super.dispose();
  }

  void _choose(ShortcutApplicationDto? app) =>
      Navigator.pop(context, ShortcutApplicationChoice(app));

  KeyEventResult _key(FocusNode node, KeyEvent event) {
    final keyboard = HardwareKeyboard.instance;
    if ((event is! KeyDownEvent && event is! KeyRepeatEvent) ||
        !_search.value.composing.isCollapsed ||
        keyboard.isMetaPressed ||
        keyboard.isControlPressed ||
        keyboard.isAltPressed ||
        keyboard.isShiftPressed) {
      return KeyEventResult.ignored;
    }
    final direction = event.logicalKey == LogicalKeyboardKey.arrowDown
        ? 1
        : event.logicalKey == LogicalKeyboardKey.arrowUp
            ? -1
            : 0;
    if (direction == 0) return KeyEventResult.ignored;
    setState(
        () => _selected = (_selected + direction).clamp(0, _matches.length));
    if (_scroll.hasClients) {
      _scroll.jumpTo(
          (_selected * _rowHeight).clamp(0, _scroll.position.maxScrollExtent));
    }
    return KeyEventResult.handled;
  }

  @override
  Widget build(BuildContext context) {
    final matches = _matches;
    return AlertDialog(
        title: Text(context.l10n.chooseApp),
        content: SizedBox(
            width: 420,
            height: min(320, MediaQuery.sizeOf(context).height - 180),
            child: Focus(
                onKeyEvent: _key,
                child: Column(children: [
                  TextField(
                      controller: _search,
                      autofocus: true,
                      decoration: InputDecoration(
                          hintText: context.l10n.findAnApp,
                          prefixIcon: Icon(AppIcons.search, size: 17),
                          isDense: true),
                      onChanged: (_) => setState(() {
                            _filterApplications();
                            _selected =
                                _matches.isEmpty || _matchedQuery!.isEmpty
                                    ? 0
                                    : 1;
                            _scroll.jumpTo(0);
                          }),
                      onSubmitted: (_) => _choose(
                          _selected == 0 ? null : _matches[_selected - 1])),
                  const SizedBox(height: 8),
                  Expanded(
                      child: ListView.builder(
                          controller: _scroll,
                          itemExtent: _rowHeight,
                          itemCount: matches.length + 1,
                          itemBuilder: (_, index) {
                            final app = index == 0 ? null : matches[index - 1];
                            return ListTile(
                                key: ValueKey(app?.bundleId ?? 'any-app'),
                                dense: true,
                                shape: RoundedRectangleBorder(
                                    borderRadius: BorderRadius.circular(
                                        AppStyle.controlRadius)),
                                leading: AppIconPlate(
                                    app == null
                                        ? AppIcons.apps
                                        : AppIcons.desktop,
                                    size: 28),
                                selected: index == _selected,
                                title: Text(
                                    app == null
                                        ? context.l10n.anyApp
                                        : context.localizedLabel(
                                            app.title, app.localizedTitles),
                                    maxLines: 1,
                                    overflow: TextOverflow.ellipsis),
                                subtitle: app == null
                                    ? null
                                    : Text(app.bundleId,
                                        maxLines: 1,
                                        overflow: TextOverflow.ellipsis,
                                        style: TextStyle(
                                            fontSize: 10,
                                            color: index == _selected
                                                ? AppStyle.primary
                                                : AppStyle.muted)),
                                trailing: app?.bundleId == widget.selectedId
                                    ? const Icon(AppIcons.check, size: 16)
                                    : null,
                                onTap: () => _choose(app));
                          })),
                  if (matches.isEmpty && _search.text.trim().isNotEmpty)
                    Text(context.l10n.noMatchingApps,
                        style: TextStyle(fontSize: 11, color: AppStyle.muted)),
                ]))),
        actions: [
          TextButton(
              onPressed: () => Navigator.pop(context),
              child: Text(context.l10n.cancel))
        ]);
  }
}
