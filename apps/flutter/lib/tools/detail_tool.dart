import '../l10n/app_language.dart';
import 'dart:async';
import 'package:flutter/services.dart';
import '../infrastructure/desktop_environment.dart';
import 'package:flutter/material.dart';
import '../plugins/plugin_detail_page.dart';
import '../src/rust/api.dart';
import '../ui/app_style.dart';
import 'tool_page.dart';
import 'tool_page_shell.dart';

final detailPages = <ToolPageRegistration>[
  ToolPageRegistration(
      id: 'legacy.detail',
      view: HostViewDto.pluginDetail,
      open: (result) {
        final detail = result.pluginDetail;
        if (detail == null || detail.schemaVersion != 1) {
          throw StateError('Unsupported or missing plugin UI');
        }
        return LegacyDetailSession(detail);
      }),
  ToolPageRegistration(
      id: 'result.detail',
      open: (result) {
        final text = result.outputText;
        if (text == null) throw StateError('Result text is missing');
        return ResultDetailSession(text,
            failed: result.status == ActionStatusDto.failed);
      }),
];

class LegacyDetailSession extends ToolPageSession {
  LegacyDetailSession(this.detail);
  final PluginDetailDto detail;
  final scroll = ScrollController();
  @override
  String get id => 'legacy.detail';
  @override
  String get title => detail.title;
  @override
  PluginDetailDto get legacyDetail => detail;
  @override
  Widget build(BuildContext context,
          {required VoidCallback onBack,
          required int presentation,
          required bool visible,
          String? notice,
          Future<void> Function()? onCopy}) =>
      PluginDetailPage(
          detail: detail,
          onBack: onBack,
          controller: scroll,
          notice: notice,
          presentation: presentation,
          visible: visible);
  @override
  void dispose() {
    scroll.dispose();
    super.dispose();
  }
}

class ResultDetailSession extends ToolPageSession {
  ResultDetailSession(this.text, {required this.failed});
  final String text;
  final bool failed;
  final scroll = ScrollController();
  @override
  String get id => 'result.detail';
  @override
  String get title => failed ? 'Action failed' : 'Action result';
  @override
  Widget build(BuildContext context,
          {required VoidCallback onBack,
          required int presentation,
          required bool visible,
          String? notice,
          Future<void> Function()? onCopy}) =>
      ToolPageShell(
          title: failed ? context.l10n.actionFailed : context.l10n.actionResult,
          onBack: onBack,
          notice: notice,
          presentation: presentation,
          visible: visible,
          scrollController: scroll,
          onKeyEvent: (event) {
            if (canInteract &&
                onCopy != null &&
                event is KeyDownEvent &&
                DesktopEnvironment.current.commandPressed &&
                HardwareKeyboard.instance.isShiftPressed &&
                event.logicalKey == LogicalKeyboardKey.keyC) {
              unawaited(onCopy());
              return KeyEventResult.handled;
            }
            return KeyEventResult.ignored;
          },
          footer: Row(children: [
            const Spacer(),
            if (onCopy != null)
              TextButton(
                  onPressed: visible ? onCopy : null,
                  child: Text(context.l10n.shortcutHint(context.l10n.copy,
                      DesktopEnvironment.current.commandShiftLabel('C')))),
          ]),
          child: Scrollbar(
              controller: scroll,
              child: SingleChildScrollView(
                key: const Key('result-preview'),
                controller: scroll,
                padding: const EdgeInsets.all(24),
                child: SelectableText(text,
                    style: const TextStyle(
                        fontFamily: 'monospace',
                        fontSize: 12,
                        height: 1.65,
                        color: AppStyle.primary)),
              )));
  @override
  void dispose() {
    scroll.dispose();
    super.dispose();
  }
}
