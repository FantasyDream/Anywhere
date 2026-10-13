import 'dart:async';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:anywhere/infrastructure/window_host.dart';
import 'package:anywhere/launcher/launcher_controller.dart';
import 'package:anywhere/launcher/launcher_screen.dart';
import 'package:anywhere/launcher/launcher_view_state.dart';
import 'package:anywhere/src/rust/api.dart';
import 'launcher_fixtures.dart';

ContextCaptureDto clipboard({int? age = 0}) => ContextCaptureDto(
    source: ContextSourceDto.clipboard,
    status: ContextCaptureStatusDto.captured,
    capturedAtMs: 1000,
    text: 'https://example.org/copied',
    clipboardAgeMs: age == null ? null : BigInt.from(age));
const selected = ContextCaptureDto(
    source: ContextSourceDto.selectedText,
    status: ContextCaptureStatusDto.captured,
    capturedAtMs: 1000,
    text: 'https://example.org/selected');
const noSelection = ContextCaptureDto(
    source: ContextSourceDto.selectedText,
    status: ContextCaptureStatusDto.empty,
    capturedAtMs: 1000);
CandidateDto action(String id, String group) => CandidateDto(
    id: id,
    kind: CandidateKindDto.action,
    title: id,
    menuPath: const [],
    contextKeys: const [],
    homeEligible: true,
    primaryAction: true,
    iconKey: 'file',
    score: 100,
    groupId: group,
    availability: AvailabilityDto.enabled);
LauncherStateDto snapshot(int revision, String input, InputModeDto mode,
        List<ContextCaptureDto> captures, {int sequence = 1}) =>
    launcherSnapshot(
        sessionId: 1,
        revision: revision,
        snapshotSeq: sequence,
        input: input,
        contextComplete: true,
        searchSources: const [],
        resultActions: const [],
        diagnostics: [],
        contextCaptures: captures,
        candidates: mode == InputModeDto.content
            ? (input.isEmpty ? [] : [action('Open URL', 'web')])
            : mode == InputModeDto.environment
                ? [
                    action('App menu', 'app.editor'),
                    action('Copy Path', 'folder')
                  ]
                : []);

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  const channel = MethodChannel('test/activation-prefill');
  final messenger =
      TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger;
  setUp(() => messenger.setMockMethodCallHandler(channel, (_) async => null));
  tearDown(() => messenger.setMockMethodCallHandler(channel, null));
  LauncherController host(List<ContextCaptureDto> captures,
          {WaitForSnapshot? wait,
          List<InputModeDto>? requests,
          bool clipboardEligible = true}) =>
      LauncherController(
          window: WindowHost(channel),
          nowMillis: () => 1000,
          backend: LauncherBackend(
              chooseActivationInput: (captures, nowMs,
                      {required selectedEnabled,
                      required clipboardEnabled,
                      required allowPendingSelection}) =>
                  fixtureInputSuggestion(captures, nowMs,
                      selectedEnabled: selectedEnabled,
                      clipboardEnabled: clipboardEnabled && clipboardEligible,
                      allowPendingSelection: allowPendingSelection),
              beginActivation: () => 1,
              activate: (_,
                      {promptAccessibility = false,
                      capturedEnvironment = false}) async =>
                  snapshot(0, '', InputModeDto.environment, captures),
              updateInput: (_, input, revision,
                  {scopeId, inputMode = InputModeDto.environment}) async {
                requests?.add(inputMode);
                return snapshot(revision, input, inputMode, captures);
              },
              execute: (_, __, ___) async => throw StateError('unused'),
              deactivate: (_) async {},
              waitForSnapshot: wait));

  test(
      'prefill uses Global Search then Escape returns Current App then an empty Global Search',
      () async {
    final requests = <InputModeDto>[];
    final controller = host([clipboard(), selected], requests: requests);
    await controller.activate();
    expect(controller.view.input, selected.text);
    expect(controller.view.inputMode, InputModeDto.content);
    expect(controller.view.scopeId, isNull);
    expect(controller.view.scopes.map((s) => s.id), ['web']);
    await controller.escape();
    expect(controller.view.input, '');
    expect(controller.view.scopeId, 'app.editor');
    expect(controller.view.visibleCandidates.map((c) => c.title),
        ['App menu', 'Copy Path']);
    await controller.escape();
    expect(controller.view.inputMode, InputModeDto.search);
    expect(controller.view.input, '');
    expect(controller.view.scopeId, isNull);
    expect(controller.view.visibleEntries, isEmpty);
    expect(requests,
        [InputModeDto.content, InputModeDto.environment, InputModeDto.search]);
    await controller.escape();
    expect(controller.view.phase, LauncherPhase.hidden);
    expect(
        controller.settings.contextSourceStatuses
            .every((s) => s.capture?.text == null),
        isTrue);
    controller.dispose();
  });

  for (final age in <int?>[15001, null]) {
    test('expired or unknown clipboard keeps Current App: age=$age', () async {
      final controller =
          host([noSelection, clipboard(age: age)], clipboardEligible: false);
      await controller.activate();
      expect(controller.view.input, '');
      expect(controller.view.inputMode, InputModeDto.environment);
      expect(controller.view.scopeId, 'app.editor');
      controller.dispose();
    });
  }
  test('recent clipboard fills when selection is empty', () async {
    final controller = host([noSelection, clipboard(age: 12000)]);
    await controller.activate();
    expect(controller.view.contentSource, ContextSourceDto.clipboard);
    expect(controller.view.input, clipboard().text);
    controller.dispose();
  });
  test('explicit Universal wake stays blank despite captured content',
      () async {
    final controller = host([selected, clipboard()]);
    await controller.activate(mode: WakeMode.universal);
    expect(controller.view.inputMode, InputModeDto.search);
    expect(controller.view.input, '');
    expect(controller.view.visibleEntries, isEmpty);
    controller.dispose();
  });
  test('late source does not overwrite a query the user entered', () async {
    final arrival = Completer<SnapshotWaitDto>();
    var calls = 0;
    final controller = host(const [],
        wait: (_, __) => calls++ == 0
            ? arrival.future
            : Completer<SnapshotWaitDto>().future);
    await controller.activate();
    await controller.update('my query');
    final revision = controller.view.snapshot!.revision;
    arrival.complete(SnapshotWaitDto(
        kind: SnapshotWaitKindDto.updated,
        state: snapshot(
            revision, 'my query', InputModeDto.environment, [selected],
            sequence: 2)));
    await Future<void>.delayed(Duration.zero);
    expect(controller.view.input, 'my query');
    expect(controller.view.inputMode, InputModeDto.environment);
    controller.dispose();
  });
  test('late selection fills an untouched wake after environment activation',
      () async {
    final arrival = Completer<SnapshotWaitDto>();
    var calls = 0;
    final controller = host(const [],
        wait: (_, __) => calls++ == 0
            ? arrival.future
            : Completer<SnapshotWaitDto>().future);
    await controller.activate();
    expect(controller.view.inputMode, InputModeDto.environment);
    arrival.complete(SnapshotWaitDto(
        kind: SnapshotWaitKindDto.updated,
        state: snapshot(0, '', InputModeDto.environment, [selected],
            sequence: 2)));
    await Future<void>.delayed(Duration.zero);
    expect(controller.view.input, selected.text);
    expect(controller.view.inputMode, InputModeDto.content);
    expect(controller.view.scopeId, isNull);
    controller.dispose();
  });
  testWidgets(
      'captured input has custom UI while original Scope bar is retained',
      (tester) async {
    final controller = host([selected, clipboard()]);
    await tester
        .pumpWidget(MaterialApp(home: LauncherScreen(controller: controller)));
    await controller.activate();
    await tester.pumpAndSettle();
    expect(find.byKey(const Key('captured-content-label')), findsOneWidget);
    expect(find.byKey(const Key('context-mode-bar')), findsNothing);
    expect(find.byKey(const Key('scope-bar')), findsOneWidget);
    expect(tester.widget<TextField>(find.byType(TextField)).controller!.text,
        selected.text);
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pumpAndSettle();
    expect(find.byKey(const Key('captured-content-label')), findsNothing);
    expect(find.text('Copy Path'), findsOneWidget);
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pumpAndSettle();
    expect(find.byKey(const Key('action-list')), findsNothing);
    expect(
        tester.widget<TextField>(find.byType(TextField)).controller!.text, '');
    await tester.pumpWidget(const SizedBox());
    controller.dispose();
  });
}
