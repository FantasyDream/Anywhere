import 'package:flutter/material.dart';
import '../../l10n/app_language.dart';
import '../../ui/app_components.dart';
import '../../ui/app_style.dart';
import '../../ui/app_icons.dart';

enum ShelfAction {
  copy(AppIcons.copy),
  copyAndReturn(AppIcons.external),
  preview(AppIcons.preview),
  delete(AppIcons.delete),
  refresh(AppIcons.refresh);

  const ShelfAction(this.glyph);
  final IconData glyph;
}

/// Inline controls share the shelf's keyboard owner; no popup route takes focus
/// away from search or escapes the bounds of a small native window.
class ShelfActionList extends StatefulWidget {
  const ShelfActionList(
      {super.key,
      required this.labels,
      required this.shortcuts,
      required this.enabled,
      required this.selected,
      required this.onInvoke,
      this.target});
  final List<String> labels, shortcuts;
  final List<bool> enabled;
  final int selected;
  final ValueChanged<int> onInvoke;
  final String? target;

  @override
  State<ShelfActionList> createState() => _ShelfActionListState();
}

class _ShelfActionListState extends State<ShelfActionList> {
  final _scroll = ScrollController();
  static const _rowHeight = 40.0;

  @override
  void didUpdateWidget(ShelfActionList old) {
    super.didUpdateWidget(old);
    if (old.selected == widget.selected) return;
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!mounted || !_scroll.hasClients) return;
      final top = widget.selected * _rowHeight;
      final bottom = top + _rowHeight;
      final viewport = _scroll.position.viewportDimension;
      if (top < _scroll.offset || bottom > _scroll.offset + viewport) {
        _scroll.jumpTo((top < _scroll.offset ? top : bottom - viewport)
            .clamp(0, _scroll.position.maxScrollExtent));
      }
    });
  }

  @override
  void dispose() {
    _scroll.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => Padding(
      padding: const EdgeInsets.fromLTRB(8, 8, 8, 4),
      child: Column(crossAxisAlignment: CrossAxisAlignment.stretch, children: [
        Padding(
            padding: const EdgeInsets.fromLTRB(8, 0, 8, 6),
            child: Text(widget.target ?? context.l10n.actions,
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: AppStyle.caption)),
        Expanded(
            child: ListView.builder(
          key: const Key('shelf-actions-list'),
          controller: _scroll,
          itemExtent: _rowHeight,
          itemCount: widget.labels.length,
          itemBuilder: (context, index) => Padding(
              padding: const EdgeInsets.symmetric(vertical: 1),
              child: Semantics(
                  button: true,
                  selected: widget.selected == index,
                  child: AppSelectionSurface(
                      selected: widget.selected == index,
                      canRequestFocus: false,
                      onTap: !widget.enabled[index]
                          ? null
                          : () => widget.onInvoke(index),
                      child: Opacity(
                          opacity: widget.enabled[index] ? 1 : .45,
                          child: Padding(
                              padding:
                                  const EdgeInsets.symmetric(horizontal: 10),
                              child: Row(children: [
                                Icon(ShelfAction.values[index].glyph,
                                    size: 16,
                                    color: index == ShelfAction.delete.index
                                        ? AppStyle.danger
                                        : AppStyle.secondary),
                                const SizedBox(width: 10),
                                Expanded(
                                    child: Text(widget.labels[index],
                                        maxLines: 1,
                                        overflow: TextOverflow.ellipsis,
                                        style: AppStyle.body
                                            .copyWith(fontSize: 12))),
                                const SizedBox(width: 8),
                                Text(widget.shortcuts[index],
                                    style: AppStyle.caption
                                        .copyWith(fontSize: 10)),
                              ])))))),
        )),
      ]));
}

class ShelfDeletePrompt extends StatelessWidget {
  const ShelfDeletePrompt(
      {super.key,
      required this.summary,
      required this.confirmSelected,
      required this.onCancel,
      required this.onConfirm});
  final String summary;
  final bool confirmSelected;
  final VoidCallback onCancel, onConfirm;

  @override
  Widget build(BuildContext context) => Center(
      child: SingleChildScrollView(
          child: Padding(
              padding: const EdgeInsets.all(16),
              child: Column(mainAxisSize: MainAxisSize.min, children: [
                Text(context.l10n.shelfDeleteTitle, style: AppStyle.body),
                const SizedBox(height: 8),
                Text(summary,
                    maxLines: 2,
                    overflow: TextOverflow.ellipsis,
                    style: AppStyle.caption,
                    textAlign: TextAlign.center),
                const SizedBox(height: 8),
                Text(context.l10n.shelfDeleteDescription,
                    style: AppStyle.caption, textAlign: TextAlign.center),
                const SizedBox(height: 14),
                Row(children: [
                  Expanded(
                      child: _choice(
                          context.l10n.cancel, !confirmSelected, onCancel)),
                  const SizedBox(width: 8),
                  Expanded(
                      child: _choice(context.l10n.deleteEntry, confirmSelected,
                          onConfirm)),
                ]),
              ]))));

  Widget _choice(String label, bool selected, VoidCallback action) => Semantics(
      button: true,
      selected: selected,
      child: AppSelectionSurface(
          selected: selected,
          canRequestFocus: false,
          onTap: action,
          child: Padding(
              padding: const EdgeInsets.all(10),
              child: Text(label,
                  textAlign: TextAlign.center,
                  style: AppStyle.body.copyWith(fontSize: 12)))));
}
