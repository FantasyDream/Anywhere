import 'package:flutter/material.dart';
import '../src/rust/api.dart';
import '../tools/tool_page.dart';
import 'web_tool_surface.dart';

final webToolPages = <ToolPageRegistration>[
  ToolPageRegistration(
      id: 'web.document',
      view: HostViewDto.webPlugin,
      open: (result) {
        final launch = result.webPlugin;
        if (launch == null) throw StateError('Web invocation is missing');
        return WebToolSession(launch.invocationId);
      }),
];

class WebToolSession extends ToolPageSession {
  WebToolSession(this.invocation);
  final String invocation;
  @override
  String get id => 'web.document';
  @override
  String get title => 'Web tool';
  @override
  double get width => 920;
  @override
  double get height => 600;
  @override
  bool get embedsPlatformView => true;
  @override
  Widget build(BuildContext context,
          {required VoidCallback onBack,
          required int presentation,
          required bool visible,
          String? notice,
          Future<void> Function()? onCopy}) =>
      WebToolSurface(
          key: ValueKey(invocation),
          invocation: invocation,
          onBack: onBack,
          visible: visible,
          presentation: presentation,
          notice: notice);
}
