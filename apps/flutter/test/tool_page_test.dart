import 'dart:async';

import 'package:anywhere/infrastructure/window_host.dart';
import 'package:anywhere/launcher/launcher_controller.dart';
import 'package:anywhere/launcher/launcher_screen.dart';
import 'package:anywhere/launcher/launcher_view_state.dart';
import 'package:anywhere/src/rust/api.dart';
import 'package:anywhere/tools/detail_tool.dart';
import 'package:anywhere/tools/tool_page.dart';
import 'package:anywhere/tools/tool_page_shell.dart';
import 'package:anywhere/ui/app_components.dart';
import 'package:anywhere/ui/app_style.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

import 'launcher_fixtures.dart';

class EditorSession extends ToolPageSession {
  final visibility = <bool>[];
  @override
  void setVisible(bool visible) {
    visibility.add(visible);
  }

  final input = TextEditingController(text: 'Initial');
  final focus = FocusNode();
  @override
  String get id => 'test.editor';
  @override
  String get title => 'Native editor';
  @override
  Widget build(BuildContext context,
          {required VoidCallback onBack,
          required int presentation,
          required bool visible,
          String? notice,
          Future<void> Function()? onCopy}) =>
      ToolPageShell(
          title: title,
          onBack: onBack,
          editorFocus: focus,
          presentation: presentation,
          visible: visible,
          child: TextField(
              key: const Key('tool-editor'),
              controller: input,
              focusNode: focus,
              enabled: canInteract,
              maxLines: null));
  @override
  void dispose() {
    input.dispose();
    focus.dispose();
    super.dispose();
  }
}

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  const channel = MethodChannel('test/tool-pages');
  setUp(() => TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
      .setMockMethodCallHandler(channel, (_) async => null));
  tearDown(() => TestDefaultBinaryMessengerBinding
      .instance.defaultBinaryMessenger
      .setMockMethodCallHandler(channel, null));
  final pages = [
    ToolPageRegistration(
        id: 'test.editor',
        view: HostViewDto.pluginDetail,
        open: (_) => EditorSession())
  ];
  test(
      'registration rejects invalid / duplicate identities and ambiguous routes',
      () {
    expect(() => ToolPageRegistry([...pages, ...pages]), throwsArgumentError);
    expect(
        () => ToolPageRegistry(
            [ToolPageRegistration(id: 'bad', open: (_) => EditorSession())]),
        throwsArgumentError);
    final ambiguous = ToolPageRegistry([
      ...pages,
      ToolPageRegistration(
          id: 'other.editor',
          view: HostViewDto.pluginDetail,
          open: (_) => EditorSession())
    ]);
    expect(
        () => ambiguous.open(const ActionResultDto(
            sessionId: 1,
            revision: 1,
            status: ActionStatusDto.succeeded,
            view: HostViewDto.pluginDetail,
            closeWindow: false)),
        throwsStateError);
  });
  LauncherController controller(int Function() now,
      {Completer<ActionResultDto>? objectResult}) {
    var session = 0;
    LauncherStateDto? held;
    const candidate = CandidateDto(
        id: 'test-editor-action',
        commandId: 'test.editor',
        kind: CandidateKindDto.action,
        title: 'Editor',
        score: 100,
        availability: AvailabilityDto.enabled,
        groupId: 'test',
        iconKey: 'code',
        menuPath: [],
        contextKeys: [],
        primaryAction: false);
    return LauncherController(
        window: WindowHost(channel),
        nowMillis: now,
        toolPages: ToolPageRegistry(
            [...pages, ...detailPages.where((p) => p.id == 'result.detail')]),
        backend: LauncherBackend(
          beginActivation: () => ++session,
          activate: (id,
                  {promptAccessibility = false,
                  capturedEnvironment = false}) async =>
              held = launcherSnapshot(
                  sessionId: id,
                  revision: 0,
                  snapshotSeq: 1,
                  input: '',
                  contextComplete: true,
                  diagnostics: [],
                  candidates: [candidate]),
          updateInput: (id, text, revision,
                  {scopeId, inputMode = InputModeDto.environment}) async =>
              held = launcherSnapshot(
                  sessionId: id,
                  revision: revision,
                  snapshotSeq: 1,
                  contextComplete: true,
                  diagnostics: [],
                  input: text,
                  candidates: [candidate]),
          execute: (id, revision, candidateId) async =>
              candidateId == 'test-object-result'
                  ? objectResult != null
                      ? await objectResult.future
                      : ActionResultDto(
                          sessionId: id,
                          revision: revision,
                          status: ActionStatusDto.succeeded,
                          outputText: 'Object output',
                          closeWindow: false)
                  : ActionResultDto(
                      sessionId: id,
                      revision: revision,
                      status: ActionStatusDto.succeeded,
                      view: HostViewDto.pluginDetail,
                      closeWindow: false),
          deactivate: (_) async {},
          suspend: (_) async {},
          resume: (_) async => held,
        ));
  }

  testWidgets(
      'nested action results retain the parent and revoke a popped page host',
      (tester) async {
    final host = controller(() => 0);
    addTearDown(host.dispose);
    await tester
        .pumpWidget(MaterialApp(home: LauncherScreen(controller: host)));
    await host.activate(mode: WakeMode.universal);
    await host.update('editor');
    await host.executeSelected();
    await tester.pumpAndSettle();
    final parent = host.view.toolPage! as EditorSession;
    parent.input.text = 'retained object query';
    const action = CandidateDto(
        id: 'test-object-result',
        kind: CandidateKindDto.action,
        title: 'Object action',
        score: 1,
        availability: AvailabilityDto.enabled,
        groupId: 'test',
        iconKey: 'copy',
        menuPath: [],
        contextKeys: [],
        primaryAction: false);
    expect(await parent.host!.execute(action), isTrue);
    await tester.pumpAndSettle();
    final child = host.view.toolPage!;
    final childHost = child.host!;
    expect(child.id, 'result.detail');
    expect(parent.disposed, isFalse);
    expect(parent.canInteract, isFalse);
    host.back();
    await tester.pumpAndSettle();
    expect(child.disposed, isTrue);
    expect(host.view.toolPage, same(parent));
    expect(parent.input.text, 'retained object query');
    expect(parent.focus.hasFocus, isTrue);
    expect(await childHost.execute(action), isFalse);
    host.back();
    await tester.pumpAndSettle();
    expect(parent.disposed, isTrue);
    expect(host.view.input, 'editor');
    expect(tester.takeException(), isNull);
  });

  testWidgets('leaving a page rejects its late action result', (tester) async {
    final result = Completer<ActionResultDto>();
    final host = controller(() => 0, objectResult: result);
    addTearDown(host.dispose);
    await tester
        .pumpWidget(MaterialApp(home: LauncherScreen(controller: host)));
    await host.activate(mode: WakeMode.universal);
    await host.update('editor');
    await host.executeSelected();
    await tester.pumpAndSettle();
    final parent = host.view.toolPage!;
    final port = parent.host!;
    const action = CandidateDto(
        id: 'test-object-result',
        kind: CandidateKindDto.action,
        title: 'Object action',
        score: 1,
        availability: AvailabilityDto.enabled,
        groupId: 'test',
        iconKey: 'copy',
        menuPath: [],
        contextKeys: [],
        primaryAction: false);
    final running = port.execute(action);
    await tester.pump();
    host.back();
    await tester.pump();
    result.complete(ActionResultDto(
        sessionId: port.sessionId,
        revision: port.revision,
        status: ActionStatusDto.succeeded,
        outputText: 'Late output',
        closeWindow: false));
    expect(await running, isFalse);
    await tester.pumpAndSettle();
    expect(parent.disposed, isTrue);
    expect(host.view.toolPage, isNull);
    expect(host.view.input, 'editor');
    expect(find.text('Late output'), findsNothing);
  });

  testWidgets('native editor focus, IME, retention and return use shared rules',
      (tester) async {
    var now = 0;
    final host = controller(() => now);
    addTearDown(host.dispose);
    await tester
        .pumpWidget(MaterialApp(home: LauncherScreen(controller: host)));
    await host.activate(mode: WakeMode.universal);
    await host.update('editor');
    await host.executeSelected();
    await tester.pump();
    await tester.pump();
    final page = host.view.toolPage! as EditorSession;
    expect(page.focus.hasFocus, isTrue);
    await tester.enterText(find.byKey(const Key('tool-editor')), 'Edited');
    page.input.value = const TextEditingValue(
        text: 'Edited',
        selection: TextSelection.collapsed(offset: 6),
        composing: TextRange(start: 0, end: 6));
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pump();
    expect(host.view.toolPage, same(page));
    page.input.value = page.input.value.copyWith(composing: TextRange.empty);
    await tester.sendKeyEvent(LogicalKeyboardKey.tab);
    expect(page.focus.hasFocus, isTrue);
    await host.dismiss(hostAlreadyHidden: true);
    expect(page.visibility.last, isFalse,
        reason: 'suspend the page before another Flutter frame');
    await tester.pump();
    expect(page.canInteract, isFalse);
    now = 14000;
    await host.activate();
    expect(page.visibility.last, isTrue);
    await tester.pump();
    await tester.pump();
    expect(host.view.toolPage, same(page));
    expect(page.input.text, 'Edited');
    expect(page.focus.hasFocus, isTrue);
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pump();
    expect(page.disposed, isTrue);
    expect(host.view.input, 'editor');
    final field = tester.widget<TextField>(find.byType(TextField));
    expect(field.focusNode!.hasFocus, isTrue);
    expect(tester.takeException(), isNull);
  });
  testWidgets(
      'presentation reconnects the input client and preserves composition',
      (tester) async {
    final focus = FocusNode();
    final input = TextEditingController(text: 'draft');
    Widget page(int presentation) => MaterialApp(
        home: Material(
            child: ToolPageShell(
                title: 'Editor',
                onBack: () {},
                editorFocus: focus,
                presentation: presentation,
                child: TextField(controller: input, focusNode: focus))));
    await tester.pumpWidget(page(0));
    await tester.pumpAndSettle();
    expect(focus.hasPrimaryFocus, isTrue);
    tester.testTextInput.log.clear();

    // No simulated platform close: the stale client still looks attached to
    // Flutter when Host finishes presenting the page.
    await tester.pumpWidget(page(1));
    await tester.pumpAndSettle();
    final methods =
        tester.testTextInput.log.map((call) => call.method).toList();
    expect(methods, contains('TextInput.clearClient'));
    expect(methods.indexOf('TextInput.setClient'),
        greaterThan(methods.indexOf('TextInput.clearClient')));
    expect(focus.hasPrimaryFocus, isTrue);
    await tester.enterText(find.byType(TextField), 'continued');
    expect(input.text, 'continued');

    input.value = const TextEditingValue(
        text: 'continued',
        selection: TextSelection.collapsed(offset: 9),
        composing: TextRange(start: 0, end: 9));
    tester.testTextInput.log.clear();
    await tester.pumpWidget(page(2));
    await tester.pumpAndSettle();
    expect(focus.hasPrimaryFocus, isTrue);
    expect(input.value.composing, const TextRange(start: 0, end: 9));
    expect(tester.testTextInput.log.map((call) => call.method),
        isNot(contains('TextInput.clearClient')));
    await tester.pumpWidget(const SizedBox());
    focus.dispose();
    input.dispose();
  });
  test('expired retention disposes the native editor session', () async {
    var now = 0;
    final host = controller(() => now);
    addTearDown(host.dispose);
    await host.activate(mode: WakeMode.universal);
    await host.update('editor');
    await host.executeSelected();
    final page = host.view.toolPage!;
    await host.dismiss(hostAlreadyHidden: true);
    now = 15000;
    await host.activate();
    expect(page.disposed, isTrue);
    expect(host.view.toolPage, isNull);
  });
  testWidgets('workspace frame has one Escape target and keeps content usable',
      (tester) async {
    await tester.binding.setSurfaceSize(const Size(920, 600));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    var back = 0;
    Widget page(bool visible) => MaterialApp(
        theme: AppStyle.theme,
        home: Material(
            child: ToolPageShell(
          title: 'Workspace with a very long title that must stay on one line',
          subtitle: 'Workspace',
          visible: visible,
          onBack: () => back++,
          footer: const Row(children: [
            Expanded(child: Text('Local processing')),
            Text('Actions · ⌘K')
          ]),
          child: const ColoredBox(
              key: Key('workspace-body'), color: Colors.transparent),
        )));
    await tester.pumpWidget(page(true));
    await tester.pumpAndSettle();
    expect(tester.getSize(find.byType(AppPageHeader)).height,
        LauncherViewState.inputHeight);
    expect(tester.getSize(find.byType(AppPageFooter)).height,
        DesignTokens.pageFooterHeight);
    expect(tester.getSize(find.byKey(const Key('workspace-body'))).height,
        greaterThan(450));
    expect(find.text('Back'), findsNothing);
    expect(find.text('esc'), findsOneWidget);
    await tester.tap(find.text('esc'));
    expect(back, 1);
    await tester.binding.setSurfaceSize(const Size(648, 480));
    await tester.pumpAndSettle();
    expect(tester.takeException(), isNull);
    await tester.pumpWidget(page(false));
    await tester.pumpAndSettle();
    await tester.tap(find.text('esc'));
    expect(back, 1);
  });
}
