import 'package:flutter/material.dart';

import '../src/rust/api.dart';
import '../tools/tool_page_shell.dart';
import '../ui/app_style.dart';

/// Host owns layout, keyboard navigation and theme. Plugin data is plain text;
/// it cannot inject widgets, links, callbacks or executable markup.
class PluginDetailPage extends StatefulWidget {
  const PluginDetailPage({
    super.key,
    required this.detail,
    required this.onBack,
    this.notice,
    this.controller,
    this.presentation = 0,
    this.visible = true,
  });
  final PluginDetailDto detail;
  final VoidCallback onBack;
  final String? notice;
  final ScrollController? controller;
  final int presentation;
  final bool visible;

  @override
  State<PluginDetailPage> createState() => _PluginDetailPageState();
}

class _PluginDetailPageState extends State<PluginDetailPage> {
  late final _scroll = widget.controller ?? ScrollController();
  @override
  void dispose() {
    if (widget.controller == null) _scroll.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final detail = widget.detail;
    return ToolPageShell(
      title: detail.title,
      onBack: widget.onBack,
      notice: widget.notice,
      scrollController: _scroll,
      presentation: widget.presentation,
      visible: widget.visible,
      child: Scrollbar(
          controller: _scroll,
          child: SingleChildScrollView(
            key: const Key('plugin-detail-scroll'),
            controller: _scroll,
            padding: const EdgeInsets.all(24),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
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
                          ])),
                if (detail.rows.isNotEmpty &&
                    (detail.body?.isNotEmpty ?? false))
                  const Divider(height: 24),
                if (detail.body != null)
                  SelectableText(detail.body!,
                      style: AppStyle.body.copyWith(height: 1.6)),
              ],
            ),
          )),
    );
  }
}
