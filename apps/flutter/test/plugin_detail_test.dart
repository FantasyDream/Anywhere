import 'dart:async';

import 'package:anywhere/infrastructure/window_host.dart';
import 'package:anywhere/launcher/host_view_navigator.dart';
import 'package:anywhere/launcher/launcher_controller.dart';
import 'package:anywhere/launcher/launcher_screen.dart';
import 'package:anywhere/plugins/plugin_detail_page.dart';
import 'package:anywhere/src/rust/api.dart';
import 'package:anywhere/ui/app_style.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

import 'launcher_fixtures.dart';

const detail = PluginDetailDto(
    schemaVersion: 1,
    title: 'Context Detail',
    body: '<script> is plain text, {{context.value}} is data',
    rows: [PluginKeyValueDto(label: 'Object', value: '/project')]);
ActionResultDto result(int session, int revision,
        {PluginDetailDto? payload = detail}) =>
    ActionResultDto(
        sessionId: session,
        revision: revision,
        status: ActionStatusDto.succeeded,
        view: HostViewDto.pluginDetail,
        pluginDetail: payload,
        closeWindow: false);
const row = CandidateDto(
    id: 'detail:context',
    commandId: 'example_detail.inspect',
    kind: CandidateKindDto.action,
    title: 'Inspect Context',
    menuPath: [],
    contextKeys: [],
    homeEligible: true,
    primaryAction: true,
    iconKey: 'file',
    score: 100,
    groupId: 'plugin.example_detail',
    availability: AvailabilityDto.enabled);
LauncherStateDto state(int session, int revision, String input) =>
    launcherSnapshot(
        sessionId: session,
        revision: revision,
        snapshotSeq: 1,
        input: input,
        contextComplete: true,
        candidates: [row],
        diagnostics: []);

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  const channel = MethodChannel('test/plugin-detail');
  final messenger =
      TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger;
  setUp(() => messenger.setMockMethodCallHandler(channel, (_) async => null));
  tearDown(() => messenger.setMockMethodCallHandler(channel, null));

  LauncherController controller({Execute? execute, int Function()? now}) {
    var session = 0;
    LauncherStateDto? held;
    return LauncherController(
        window: WindowHost(channel),
        nowMillis: now,
        backend: LauncherBackend(
            beginActivation: () => ++session,
            activate: (id,
                    {promptAccessibility = false,
                    capturedEnvironment = false}) async =>
                held = state(id, 0, ''),
            updateInput: (id, text, revision,
                    {scopeId, inputMode = InputModeDto.environment}) async =>
                held = state(id, revision, text),
            execute: execute ?? (id, revision, _) async => result(id, revision),
            suspend: (_) async {},
            resume: (_) async => held,
            deactivate: (_) async {}));
  }

  test('payload routes require matching data', () async {
    final navigation = HostViewNavigator();
    PluginDetailDto? opened;
    navigation.registerPayload(
        HostViewDto.pluginDetail, (value) async => opened = value.pluginDetail);
    await expectLater(
        navigation.open(HostViewDto.pluginDetail), throwsStateError);
    await navigation.open(HostViewDto.pluginDetail, result: result(1, 1));
    expect(opened, detail);
  });

  test('detail returns to original query and survives only the 15s lifetime',
      () async {
    var now = 0;
    final host = controller(now: () => now);
    addTearDown(host.dispose);
    await host.activate(mode: WakeMode.universal);
    await host.update('object');
    await host.executeSelected();
    expect(host.view.pluginDetail, detail);
    final selected = host.view.selectedId;
    await host.dismiss(hostAlreadyHidden: true);
    now = 14000;
    await host.activate();
    expect(host.view.pluginDetail, detail);
    expect(host.view.input, 'object');
    await host.escape();
    expect(host.view.pluginDetail, isNull);
    expect(host.view.input, 'object');
    expect(host.view.selectedId, selected);
    await host.executeSelected();
    await host.dismiss(hostAlreadyHidden: true);
    now += 15000;
    await host.activate();
    expect(host.view.pluginDetail, isNull);
    expect(host.view.input, isEmpty);
  });

  test('late results cannot reopen a dismissed detail', () async {
    final pending = Completer<ActionResultDto>();
    final host = controller(execute: (_, __, ___) => pending.future);
    addTearDown(host.dispose);
    await host.activate(mode: WakeMode.universal);
    await host.update('object');
    final session = host.view.snapshot!.sessionId;
    final revision = host.view.snapshot!.revision;
    final opening = host.executeSelected();
    expect(host.view.executing, isTrue);
    await host.dismiss(retain: false);
    pending.complete(result(session, revision));
    await opening;
    expect(host.view.pluginDetail, isNull);
    expect(host.view.visible, isFalse);
  });

  test('missing payload reports an error and leaves the query usable',
      () async {
    final host = controller(
        execute: (id, revision, _) async =>
            result(id, revision, payload: null));
    addTearDown(host.dispose);
    await host.activate(mode: WakeMode.universal);
    await host.update('query');
    await host.executeSelected();
    expect(host.view.pluginDetail, isNull);
    expect(host.view.input, 'query');
    expect(host.view.executing, isFalse);
    expect(host.view.notice, contains('missing plugin UI'));
  });

  testWidgets('completed views keep follow-up warnings visible',
      (tester) async {
    final host = controller(
        execute: (id, revision, _) async => ActionResultDto(
              sessionId: id,
              revision: revision,
              status: ActionStatusDto.succeeded,
              view: HostViewDto.pluginDetail,
              pluginDetail: detail,
              warning: 'Usage could not be saved',
              closeWindow: false,
            ));
    addTearDown(host.dispose);
    await tester.pumpWidget(MaterialApp(
        theme: AppStyle.theme, home: LauncherScreen(controller: host)));
    await host.activate(mode: WakeMode.universal);
    await host.update('object');
    await host.executeSelected();
    await tester.pump();
    expect(find.text('Usage could not be saved'), findsOneWidget);
    expect(find.byType(PluginDetailPage), findsOneWidget);
    expect(tester.takeException(), isNull);
  });

  testWidgets('host layout renders plain text and arrow keys scroll',
      (tester) async {
    var back = 0;
    final long = PluginDetailDto(
        schemaVersion: 1,
        title: detail.title,
        body: List.filled(80, 'A line of readable text').join('\n'),
        rows: detail.rows);
    await tester.pumpWidget(MaterialApp(
        theme: AppStyle.theme,
        home: Scaffold(
            body: Center(
                child: SizedBox(
                    width: 648,
                    height: 480,
                    child: PluginDetailPage(
                        detail: long, onBack: () => back++))))));
    await tester.pump();
    final scroll = tester.widget<SingleChildScrollView>(
        find.byKey(const Key('plugin-detail-scroll')));
    expect(scroll.controller!.offset, 0);
    await tester.sendKeyEvent(LogicalKeyboardKey.arrowDown);
    expect(scroll.controller!.offset, 40);
    await tester.sendKeyEvent(LogicalKeyboardKey.tab);
    expect(scroll.controller!.offset, 40);
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    expect(back, 1);
    expect(tester.takeException(), isNull);
  });

  testWidgets('Esc restores the shared search field and keyboard input',
      (tester) async {
    final host = controller();
    addTearDown(host.dispose);
    await tester.pumpWidget(MaterialApp(
        theme: AppStyle.theme, home: LauncherScreen(controller: host)));
    await host.activate(mode: WakeMode.universal);
    await host.update('object');
    await host.executeSelected();
    await tester.pump();
    await tester.pump();
    expect(find.byType(PluginDetailPage), findsOneWidget);
    expect(find.byType(TextField), findsNothing);
    expect(find.text(detail.body!), findsOneWidget);
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pump();
    expect(find.byType(PluginDetailPage), findsNothing);
    final field = tester.widget<TextField>(find.byType(TextField));
    expect(field.focusNode!.hasFocus, isTrue);
    await tester.enterText(find.byType(TextField), 'next object');
    await tester.pump();
    expect(host.view.input, 'next object');
    expect(tester.takeException(), isNull);
  });
}
