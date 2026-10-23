import 'package:anywhere/infrastructure/window_host.dart';
import 'package:anywhere/l10n/app_language.dart';
import 'package:anywhere/launcher/context_source_status.dart';
import 'package:anywhere/settings/action_shortcut.dart';
import 'package:anywhere/settings/extension_presentation.dart';
import 'package:anywhere/settings/host_settings.dart';
import 'package:anywhere/settings/settings_controller.dart';
import 'package:anywhere/src/rust/api.dart';
import 'package:anywhere/ui/file_type_appearance.dart';
import 'package:anywhere/web_plugins/permission_labels.dart';
import 'package:anywhere/web_plugins/web_host_panels.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  final english = lookupAppLocalizations(const Locale('en'));
  final chinese = lookupAppLocalizations(const Locale('zh'));

  test(
      'validation language changes only presentation and preserves native diagnostic data',
      () async {
    const invalid = ActionShortcut(
        actionId: 'test.action',
        title: 'User title',
        keyCode: 999,
        modifiers: 0);
    expect(invalid.chordErrorFor(chinese), chinese.letterWithModifier);
    expect(invalid.chordError, english.letterWithModifier);
    expect(invalid.actionId, 'test.action');
    final settings = SettingsController(
        window:
            WindowHost(const MethodChannel('test/localization-validation')));
    addTearDown(settings.dispose);
    expect(
        await settings.save(
            HostSettings.defaults.copyWith(fileSearchRoots: ['relative/path'])),
        isFalse);
    expect(
        settings.statusMessageFor(chinese), chinese.absoluteSearchDirectories);
    expect(
        settings.statusMessageFor(english), english.absoluteSearchDirectories);
    settings.reportError('native.failure: unchanged diagnostic');
    expect(settings.statusMessageFor(chinese),
        'native.failure: unchanged diagnostic');
  });

  test(
      'capture and file types localize from typed values without changing payloads',
      () {
    const status = ContextSourceStatus(ContextSourceDto.selectedText, null);
    expect(status.withLocalizations(chinese).statusText, '上次唤醒时未采集');
    expect(status.withLocalizations(english).statusText,
        'Not captured in the last wake');
    expect(FileTypeAppearance.configuration.labelFor(chinese), '配置文件');
    expect(FileTypeAppearance.configuration.labelFor(english),
        'Configuration file');
    expect(chinese.fileFoundCount(47), '找到 47 项');
    expect(english.searchIssues(1), '1 issue');
    expect(english.searchIssues(7), '7 issues');
  });

  test(
      'permission descriptions follow verified capability types in both languages',
      () {
    const read = WebPermissionDto(
        kind: WebPermissionKindDto.clipboardReadText, contextTypes: []);
    const context = WebPermissionDto(
        kind: WebPermissionKindDto.contextRead,
        contextTypes: ['core.file', 'vendor.private-type']);
    expect(webPermissionLabel(read, english), contains('Read clipboard text'));
    expect(webPermissionLabel(read, chinese), contains('每次操作均需确认'));
    expect(webPermissionLabel(context, chinese), contains('文件'));
    expect(
        webPermissionLabel(context, chinese), contains('vendor.private-type'));
    expect(context.contextTypes, ['core.file', 'vendor.private-type']);
  });

  test('module presentation uses typed state and preserves external names', () {
    ExtensionStatusDto module({
      bool restart = false,
      bool firstParty = false,
      String detail = 'Restart Anywhere: external diagnostic',
    }) =>
        ExtensionStatusDto(
            key: 'test',
            id: 'clipboard',
            name: 'External Clipboard',
            version: '1.0.0',
            firstParty: firstParty,
            legacy: false,
            enabled: true,
            active: false,
            needsApproval: false,
            configurable: true,
            restartRequired: restart,
            capabilities: const [],
            approvalToken: '',
            detail: detail);
    expect(module().phase, ExtensionPhase.failed,
        reason: 'diagnostic text cannot choose the module state');
    final waiting = module(restart: true, detail: '未启动');
    expect(waiting.phase, ExtensionPhase.restartRequired);
    expect(waiting.stateLabelFor(chinese), chinese.restartRequired);
    expect(waiting.stateLabelFor(english), english.restartRequired);
    expect(waiting.nameFor(chinese), 'External Clipboard');
    expect(module(firstParty: true).nameFor(chinese), chinese.clipboardHistory);
    expect(module(firstParty: true).nameFor(english), english.clipboardHistory);
  });

  testWidgets('file icon accessibility follows the current display language',
      (tester) async {
    final semantics = tester.ensureSemantics();
    final language = AppLanguage.current;
    final previous = language.preference;
    addTearDown(() => language.apply(previous));
    try {
      language.apply('zh');
      await tester.pumpWidget(const LocalizedApp(
          home: Scaffold(
              body:
                  FileTypeIcon(appearance: FileTypeAppearance.configuration))));
      await tester.pumpAndSettle();
      expect(find.bySemanticsLabel('配置文件'), findsOneWidget);
      language.apply('en');
      await tester.pumpAndSettle();
      expect(find.bySemanticsLabel('Configuration file'), findsOneWidget);
      expect(find.bySemanticsLabel('配置文件'), findsNothing);
    } finally {
      semantics.dispose();
    }
  });

  testWidgets(
      'host consent follows a live language switch and retains the external preview',
      (tester) async {
    final language = AppLanguage.current;
    final previous = language.preference;
    addTearDown(() => language.apply(previous));
    const payload = '{"用户":"Ada"}';
    final selected = <bool>[];
    language.apply('en');
    await tester.pumpWidget(LocalizedApp(
        home: Builder(
            builder: (context) => Scaffold(
                    body: Stack(children: [
                  WebConsentPanel(
                      title: context.l10n.readClipboardText,
                      toolTitle: 'Example Tool',
                      preview: payload,
                      allowSelected: false,
                      onAnswer: selected.add),
                ])))));
    await tester.pumpAndSettle();
    expect(find.text('Read clipboard text'), findsOneWidget);
    language.apply('zh');
    await tester.pumpAndSettle();
    expect(find.text('读取剪贴板文本'), findsOneWidget);
    expect(find.text(payload), findsOneWidget);
    expect(find.textContaining('Example Tool'), findsOneWidget);
    expect(selected, isEmpty,
        reason: 'language changes cannot approve an operation');
    await tester.tap(find.text(chinese.allowOnceAction));
    expect(selected, [true]);
  });
}
