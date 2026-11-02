import 'package:flutter/material.dart';

import '../l10n/app_language.dart';
import '../l10n/localized_label.dart';
import '../src/rust/api.dart';
import '../ui/app_icons.dart';
import '../ui/app_style.dart';
import 'action_shortcut.dart';

/// Shared action choice for direct shortcuts and context rules. The dialog owns
/// the query so returning from the binding form preserves the search draft.
class ShortcutActionPicker extends StatefulWidget {
  const ShortcutActionPicker(
      {super.key,
      required this.actions,
      required this.query,
      required this.onSelected});
  final List<ShortcutActionDto> actions;
  final TextEditingController query;
  final ValueChanged<ShortcutActionDto> onSelected;

  @override
  State<ShortcutActionPicker> createState() => _ShortcutActionPickerState();
}

class _ShortcutActionPickerState extends State<ShortcutActionPicker> {
  final _scroll = ScrollController();
  List<ShortcutActionDto> _matches = const [];
  String? _matchedQuery;
  String? _matchedLocale;

  @override
  void initState() {
    super.initState();
    widget.query.addListener(_queryChanged);
  }

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    if (_matchedLocale != context.l10n.localeName) _filter();
  }

  @override
  void didUpdateWidget(ShortcutActionPicker oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.query != widget.query) {
      oldWidget.query.removeListener(_queryChanged);
      widget.query.addListener(_queryChanged);
    }
    if (oldWidget.query != widget.query ||
        !identical(oldWidget.actions, widget.actions)) {
      _filter();
    }
  }

  void _queryChanged() {
    if (widget.query.text.toLowerCase() == _matchedQuery) return;
    setState(_filter);
  }

  void _filter() {
    _matchedLocale = context.l10n.localeName;
    final needle = _matchedQuery = widget.query.text.toLowerCase();
    _matches = needle.isEmpty
        ? widget.actions
        : widget.actions.where((action) {
            final text = '${action.title} '
                '${action.localizedTitles.values.join(' ')} '
                '${action.contextTypes.map((type) => contextTypeLabel(type, context.l10n)).join(' ')} '
                '${action.contextTypes.join(' ')}';
            return text.toLowerCase().contains(needle);
          }).toList();
    if (_scroll.hasClients) _scroll.jumpTo(0);
  }

  @override
  void dispose() {
    widget.query.removeListener(_queryChanged);
    _scroll.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => Column(children: [
        TextField(
            controller: widget.query,
            autofocus: true,
            decoration: InputDecoration(
                hintText: context.l10n.findAnAction,
                prefixIcon: const Icon(AppIcons.search, size: 17),
                isDense: true)),
        const SizedBox(height: 8),
        Expanded(
            child: ListView.builder(
                controller: _scroll,
                itemCount: _matches.length,
                itemBuilder: (context, index) {
                  final action = _matches[index];
                  return ListTile(
                      dense: true,
                      title: Text(context.localizedLabel(
                          action.title, action.localizedTitles)),
                      subtitle: Text(
                          action.contextTypes
                              .map((type) =>
                                  contextTypeLabel(type, context.l10n))
                              .join(' · '),
                          style: AppStyle.caption),
                      onTap: () => widget.onSelected(action));
                })),
      ]);
}
