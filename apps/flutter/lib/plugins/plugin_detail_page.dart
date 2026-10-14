import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import '../src/rust/api.dart';
import '../ui/app_style.dart';
import '../ui/app_components.dart';

/// Host owns layout, keyboard navigation and theme. Plugin data is plain text;
/// it cannot inject widgets, links, callbacks or executable markup.
class PluginDetailPage extends StatefulWidget {
  const PluginDetailPage({
    super.key,
    required this.detail,
    required this.onBack,
    this.notice,
  });
  final PluginDetailDto detail;
  final VoidCallback onBack;
  final String? notice;

  @override
  State<PluginDetailPage> createState() => _PluginDetailPageState();
}

class _PluginDetailPageState extends State<PluginDetailPage> {
  final _scroll = ScrollController();
  final _focus = FocusNode(debugLabel: 'plugin-detail');

  @override
  void dispose() {
    _scroll.dispose();
    _focus.dispose();
    super.dispose();
  }

  KeyEventResult _onKey(FocusNode node, KeyEvent event) {
    if (event is! KeyDownEvent && event is! KeyRepeatEvent) {
      return KeyEventResult.ignored;
    }
    if (event.logicalKey == LogicalKeyboardKey.escape) {
      widget.onBack();
      return KeyEventResult.handled;
    }
    if (event.logicalKey == LogicalKeyboardKey.tab) {
      return KeyEventResult.handled;
    }
    // Text selection keeps its modifier keys. Bare arrows scroll the reader.
    if (HardwareKeyboard.instance.isShiftPressed ||
        HardwareKeyboard.instance.isControlPressed ||
        HardwareKeyboard.instance.isAltPressed ||
        HardwareKeyboard.instance.isMetaPressed) {
      return KeyEventResult.ignored;
    }
    final distance = switch (event.logicalKey) {
      LogicalKeyboardKey.arrowDown => 40.0,
      LogicalKeyboardKey.arrowUp => -40.0,
      LogicalKeyboardKey.pageDown => 240.0,
      LogicalKeyboardKey.pageUp => -240.0,
      _ => null,
    };
    if (distance == null) return KeyEventResult.ignored;
    if (_scroll.hasClients) {
      _scroll.jumpTo((_scroll.offset + distance)
          .clamp(0, _scroll.position.maxScrollExtent));
    }
    return KeyEventResult.handled;
  }

  @override
  Widget build(BuildContext context) {
    final detail = widget.detail;
    return Focus(
      autofocus: true,
      focusNode: _focus,
      onKeyEvent: _onKey,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Padding(
            padding: const EdgeInsets.fromLTRB(16, 16, 20, 16),
            child: Row(children: [
              IconButton(
                tooltip: 'Back · Esc',
                onPressed: widget.onBack,
                icon: const Icon(Icons.chevron_left_rounded, size: 26),
              ),
              const SizedBox(width: 8),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    const Text('Detail', style: AppStyle.caption),
                    const SizedBox(height: 4),
                    Text(detail.title,
                        maxLines: 2,
                        overflow: TextOverflow.ellipsis,
                        style: AppStyle.heading.copyWith(fontSize: 22)),
                  ],
                ),
              ),
            ]),
          ),
          const Divider(height: 1, indent: 20, endIndent: 20),
          Expanded(
            child: Scrollbar(
              controller: _scroll,
              child: SingleChildScrollView(
                key: const Key('plugin-detail-scroll'),
                controller: _scroll,
                padding: const EdgeInsets.all(24),
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  children: [
                    if (widget.notice != null)
                      Padding(
                        padding: const EdgeInsets.only(bottom: 16),
                        child: AppNotice(widget.notice!),
                      ),
                    for (final row in detail.rows)
                      Padding(
                        padding: const EdgeInsets.only(bottom: 16),
                        child: Row(
                          crossAxisAlignment: CrossAxisAlignment.start,
                          children: [
                            SizedBox(
                                width: 140,
                                child:
                                    Text(row.label, style: AppStyle.caption)),
                            const SizedBox(width: 16),
                            Expanded(
                                child: SelectableText(row.value,
                                    style: AppStyle.body)),
                          ],
                        ),
                      ),
                    if (detail.rows.isNotEmpty &&
                        (detail.body?.isNotEmpty ?? false)) ...[
                      const Divider(height: 24),
                    ],
                    if (detail.body != null)
                      SelectableText(detail.body!,
                          style: AppStyle.body.copyWith(height: 1.6)),
                  ],
                ),
              ),
            ),
          ),
          const Divider(height: 1),
          Padding(
            padding: const EdgeInsets.fromLTRB(24, 10, 24, 12),
            child: Row(children: [
              const Expanded(child: Text('↑↓ Scroll', style: AppStyle.caption)),
              TextButton(onPressed: widget.onBack, child: const Text('Back')),
              const SizedBox(width: 8),
              const Text('esc', style: AppStyle.caption),
            ]),
          ),
        ],
      ),
    );
  }
}
