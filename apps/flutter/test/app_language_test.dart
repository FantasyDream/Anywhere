import 'dart:async';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:anywhere/l10n/app_language.dart';
import 'package:anywhere/settings/host_settings.dart';
import 'package:anywhere/ui/app_style.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  test('legacy preference and region fallback preserve unrelated settings', () {
    final old = HostSettings.defaults.toMap()..remove('uiLanguage');
    expect(HostSettings.fromMap(old).uiLanguage, 'system');
    final next = HostSettings.fromMap(old).copyWith(uiLanguage: 'zh');
    expect(HostSettings.fromMap(next.toMap()).uiLanguage, 'zh');
    expect(next.currentAppKeyCode, HostSettings.defaults.currentAppKeyCode);
    expect(next.actionShortcuts, HostSettings.defaults.actionShortcuts);
    expect(AppLanguage.resolve('system', [const Locale('zh', 'TW')]),
        const Locale('zh'));
    expect(AppLanguage.resolve('system', [const Locale('ja')]),
        const Locale('en'));
    expect(AppLanguage.resolve('en', [const Locale('zh')]), const Locale('en'));
    expect(HostSettings.fromMap({...old, 'uiLanguage': 'invalid'}).uiLanguage,
        'system');
  });

  testWidgets('language changes preserve the editor, selection and focus',
      (tester) async {
    final input = TextEditingController(text: '用户内容');
    final focus = FocusNode();
    AppLanguage.current.apply('en');
    addTearDown(() {
      input.dispose();
      focus.dispose();
      AppLanguage.current.apply('system');
    });
    await tester.pumpWidget(LocalizedApp(
        theme: AppStyle.theme,
        home: Scaffold(
            body: Builder(
                builder: (context) => Column(children: [
                      Text(context.l10n.settings),
                      Text(context.l10n.resultsCount(2)),
                      TextField(controller: input, focusNode: focus),
                    ])))));
    await tester.pumpAndSettle();
    await tester.tap(find.byType(TextField));
    input.selection = const TextSelection(baseOffset: 1, extentOffset: 3);
    AppLanguage.current.apply('zh');
    await tester.pumpAndSettle();
    expect(find.text('设置'), findsOneWidget);
    expect(find.text('2 个结果'), findsOneWidget);
    expect(input.text, '用户内容');
    expect(
        input.selection, const TextSelection(baseOffset: 1, extentOffset: 3));
    expect(focus.hasFocus, isTrue);
    expect(
        MaterialLocalizations.of(tester.element(find.byType(TextField)))
            .copyButtonLabel,
        '复制');
    AppLanguage.current.apply('en');
    await tester.pumpAndSettle();
    expect(find.text('Settings'), findsOneWidget);
    expect(find.text('2 results'), findsOneWidget);
    await tester.pumpWidget(const SizedBox());
  });

  testWidgets('a missing language reply does not strand startup',
      (tester) async {
    const channel = MethodChannel('test/locale-timeout');
    final messenger =
        TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger;
    final pending = Completer<String>();
    messenger.setMockMethodCallHandler(channel, (_) => pending.future);
    final language = AppLanguage(channel: channel);
    var completed = false;
    final attaching = language.attach().then((_) => completed = true);
    await tester.pump();
    await tester.pump(const Duration(seconds: 1));
    expect(completed, isTrue);
    expect(language.preference, 'system');
    language.dispose();
    pending.complete('zh');
    await attaching;
    messenger.setMockMethodCallHandler(channel, null);
  });

  testWidgets('a late initial read cannot overwrite a newer native language',
      (tester) async {
    const channel = MethodChannel('test/locale');
    final messenger =
        TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger;
    final initial = Completer<String>();
    messenger.setMockMethodCallHandler(channel, (_) => initial.future);
    final locale = AppLanguage(channel: channel);
    final attaching = locale.attach();
    await tester.pump();
    final delivered = Completer<void>();
    unawaited(messenger.handlePlatformMessage(
        channel.name,
        const StandardMethodCodec()
            .encodeMethodCall(const MethodCall('changed', 'zh')),
        (_) => delivered.complete()));
    await delivered.future;
    initial.complete('en');
    await attaching;
    expect(locale.preference, 'zh');
    locale.dispose();
    messenger.setMockMethodCallHandler(channel, null);
  });
}
