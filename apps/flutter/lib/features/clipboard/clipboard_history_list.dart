import '../../l10n/app_language.dart';
import 'package:flutter/material.dart';
import 'package:flutter/gestures.dart';
import '../../infrastructure/desktop_environment.dart';
import '../../src/rust/api.dart';
import '../../ui/app_style.dart';
import '../../ui/app_components.dart';
import 'clipboard_history_presentation.dart';

/// Metadata-only rows. Full payloads and thumbnails are loaded only on selection.
class ClipboardHistoryList extends StatelessWidget {
  const ClipboardHistoryList(
      {super.key,
      required this.items,
      required this.selectedIndex,
      required this.scroll,
      required this.onSelect,
      this.onActivate,
      this.onActions,
      this.selectedCommands,
      this.selectedStatus,
      this.compact = false});
  final List<ClipboardItemDto> items;
  final int selectedIndex;
  final ScrollController scroll;
  final ValueChanged<ClipboardItemDto>? onSelect;
  final ValueChanged<ClipboardItemDto>? onActivate;
  final ValueChanged<ClipboardItemDto>? onActions;
  final Widget? selectedCommands;
  final String? selectedStatus;
  final bool compact;

  @override
  Widget build(BuildContext context) => ListView.builder(
      controller: scroll,
      itemExtent: compact ? clipboardShelfRowHeight : clipboardHistoryRowHeight,
      padding: EdgeInsets.zero,
      itemCount: items.length,
      itemBuilder: (context, index) {
        final item = items[index];
        final selected = index == selectedIndex;
        final commands = selected ? selectedCommands : null;
        final object = GestureDetector(
            // Only the object recognizes double-clicks. The command buttons
            // remain immediate and do not join this primary gesture arena.
            behavior: HitTestBehavior.opaque,
            onDoubleTap: onActivate == null ? null : () => onActivate!(item),
            child: Tooltip(
                message: item.summary,
                child: Row(children: [
                  Icon(clipboardKindIcon(item.kind),
                      size: compact ? 18 : 22,
                      color: clipboardKindTint(item.kind)),
                  const SizedBox(width: 12),
                  Expanded(child: _labels(context, item, selected)),
                ])));
        final row = Padding(
            padding: EdgeInsets.symmetric(
                horizontal: compact ? 6 : 10, vertical: compact ? 1 : 3),
            child: AppSelectionSurface(
                selected: selected,
                canRequestFocus: !compact,
                child: Padding(
                    padding: const EdgeInsets.symmetric(horizontal: 12),
                    child: Row(children: [
                      Expanded(child: object),
                      if (commands != null) ...[
                        const SizedBox(width: 4),
                        commands,
                      ] else if (compact && index < 9) ...[
                        const SizedBox(width: 8),
                        Text(
                            DesktopEnvironment.current
                                .commandLabel('${index + 1}'),
                            style: AppStyle.caption.copyWith(fontSize: 10)),
                      ],
                    ]))));
        return Listener(
            // Select before Return or a command, without waiting for a double
            // click. Dragging and scrolling can select but never copy content.
            onPointerDown: onSelect == null
                ? null
                : (event) {
                    if (event.buttons == kPrimaryMouseButton) onSelect!(item);
                  },
            child: GestureDetector(
                onSecondaryTap:
                    onActions == null ? null : () => onActions!(item),
                child: Semantics(
                    selected: selected,
                    button: true,
                    enabled: onSelect != null,
                    onTap: onSelect == null ? null : () => onSelect!(item),
                    child: row)));
      });

  Widget _labels(BuildContext context, ClipboardItemDto item, bool selected) =>
      Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          mainAxisAlignment: MainAxisAlignment.center,
          children: [
            Text(item.summary,
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: compact
                    ? AppStyle.body.copyWith(fontSize: 12)
                    : AppStyle.body),
            SizedBox(height: compact ? 2 : 4),
            if (selected && selectedStatus != null)
              Semantics(
                  liveRegion: true,
                  child: Text(selectedStatus!,
                      key: const Key('clipboard-row-status'),
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: AppStyle.caption.copyWith(
                          fontSize: compact ? 10 : 11,
                          color: AppStyle.success)))
            else
              Text(
                  '${clipboardKindLabel(item.kind, context.l10n)} · ${clipboardCopyDate(context, item.copiedAtMs.toInt(), compact: true)}',
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: compact
                      ? AppStyle.caption.copyWith(fontSize: 10)
                      : AppStyle.caption),
          ]);
}
