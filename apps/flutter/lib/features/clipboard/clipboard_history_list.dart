import '../../l10n/app_language.dart';
import 'package:flutter/material.dart';
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
      this.compact = false});
  final List<ClipboardItemDto> items;
  final int selectedIndex;
  final ScrollController scroll;
  final ValueChanged<ClipboardItemDto>? onSelect;
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
        return Padding(
            padding: EdgeInsets.symmetric(
                horizontal: compact ? 6 : 10, vertical: compact ? 1 : 3),
            child: AppSelectionSurface(
                selected: selected,
                onTap: onSelect == null ? null : () => onSelect!(item),
                child: Tooltip(
                    message: item.summary,
                    child: Semantics(
                        selected: selected,
                        child: Padding(
                            padding: const EdgeInsets.symmetric(horizontal: 12),
                            child: Row(children: [
                              Icon(clipboardKindIcon(item.kind),
                                  size: compact ? 18 : 22,
                                  color: AppStyle.secondary),
                              const SizedBox(width: 12),
                              Expanded(
                                  child: Column(
                                      crossAxisAlignment:
                                          CrossAxisAlignment.start,
                                      mainAxisAlignment:
                                          MainAxisAlignment.center,
                                      children: [
                                    Text(item.summary,
                                        maxLines: 1,
                                        overflow: TextOverflow.ellipsis,
                                        style: compact
                                            ? AppStyle.body
                                                .copyWith(fontSize: 12)
                                            : AppStyle.body),
                                    SizedBox(height: compact ? 2 : 4),
                                    Text(
                                        '${clipboardKindLabel(item.kind, context.l10n)} · ${clipboardCopyDate(context, item.copiedAtMs.toInt(), compact: true)}',
                                        maxLines: 1,
                                        overflow: TextOverflow.ellipsis,
                                        style: compact
                                            ? AppStyle.caption
                                                .copyWith(fontSize: 10)
                                            : AppStyle.caption),
                                  ])),
                              if (compact && index < 9) ...[
                                const SizedBox(width: 8),
                                Text(
                                    DesktopEnvironment.current
                                        .commandLabel('${index + 1}'),
                                    style: AppStyle.caption
                                        .copyWith(fontSize: 10)),
                              ],
                            ]))))));
      });
}
