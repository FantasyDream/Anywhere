import 'dart:ui' as ui;

import 'package:anywhere/infrastructure/desktop_environment.dart';
import 'package:anywhere/infrastructure/window_host.dart';
import 'package:anywhere/l10n/app_language.dart';
import 'package:anywhere/launcher/launcher_controller.dart';
import 'package:anywhere/launcher/launcher_screen.dart';
import 'package:anywhere/launcher/widgets/candidate_row.dart';
import 'package:anywhere/src/rust/api.dart';
import 'package:anywhere/ui/app_style.dart';
import 'package:flutter/material.dart';
import 'package:flutter/semantics.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

import 'launcher_fixtures.dart';
import 'launcher_test.dart' show candidate;
import 'support/semantics_update_recorder.dart';

Iterable<SemanticsNode> _subtree(SemanticsNode node) sync* {
  yield node;
  final children = <SemanticsNode>[];
  node.visitChildren((child) {
    children.add(child);
    return true;
  });
  for (final child in children) {
    yield* _subtree(child);
  }
}

void main() {
  final binding = SemanticsRecordingBinding();
  const channel = MethodChannel('test/launcher-semantics');
  final messenger = binding.defaultBinaryMessenger;

  setUp(() {
    DesktopEnvironment.configure(
        const DesktopEnvironment(platform: TargetPlatform.macOS));
    messenger.setMockMethodCallHandler(channel, (_) async => null);
    binding.recorder = SemanticsUpdateRecorder();
  });
  tearDown(() {
    binding.recorder = null;
    AppLanguage.current.apply('system');
    DesktopEnvironment.reset();
    messenger.setMockMethodCallHandler(channel, null);
  });

  testWidgets('row tooltips keep every serialized semantics node reachable',
      (tester) async {
    final shown = ValueNotifier(true);
    addTearDown(shown.dispose);
    await tester.pumpWidget(MaterialApp(
        theme: AppStyle.theme,
        home: Scaffold(
            body: ValueListenableBuilder(
                valueListenable: shown,
                builder: (_, visible, __) => visible
                    ? ListView(children: [
                        CandidateRow(
                            candidate: candidate('file', 'Synthetic.txt',
                                kind: CandidateKindDto.file),
                            selected: true,
                            actionShortcut: '⌘1',
                            primaryLabel: 'Open',
                            onTap: () {},
                            onPrimary: () {},
                            onActions: () {},
                            onPreview: () {})
                      ])
                    : const SizedBox.shrink()))));
    final tooltips = tester
        .stateList<TooltipState>(find.descendant(
            of: find.byType(CandidateRow), matching: find.byType(Tooltip)))
        .toList();
    expect(tooltips, hasLength(5));
    for (final tooltip in tooltips) {
      tooltip.ensureTooltipVisible();
      await tester.pump(const Duration(milliseconds: 200));
    }
    shown.value = false;
    await tester.pumpAndSettle();
    shown.value = true;
    await tester.pumpAndSettle();
    expect(binding.recorder!.failures, isEmpty);
    expect(tester.takeException(), isNull);
  });

  testWidgets('hovered row hints retain accessible labels and actions',
      (tester) async {
    final invoked = <String>[];
    await tester.pumpWidget(LocalizedApp(
        theme: AppStyle.theme,
        home: Scaffold(
            body: ListView(children: [
          CandidateRow(
              candidate: candidate('file', 'Synthetic.txt',
                  kind: CandidateKindDto.file),
              selected: true,
              actionShortcut: '⌘1',
              primaryLabel: 'Open',
              onTap: () {},
              onPrimary: () => invoked.add('primary'),
              onActions: () => invoked.add('actions'),
              onPreview: () => invoked.add('preview'))
        ]))));
    final mouse = await tester.createGesture(kind: ui.PointerDeviceKind.mouse);
    await mouse.addPointer(location: const Offset(790, 590));
    for (final locale in ['en', 'zh']) {
      AppLanguage.current.apply(locale);
      await tester.pumpAndSettle();
      final tooltips = find.byType(Tooltip);
      expect(tooltips, findsNWidgets(5));
      for (var index = 0; index < 5; index++) {
        await mouse.moveTo(tester.getCenter(tooltips.at(index)));
        await tester.pump(const Duration(milliseconds: 800));
        await tester.pump(const Duration(milliseconds: 200));
        expect(binding.recorder!.failures, isEmpty);
      }
    }
    for (final action in ['preview', 'actions', 'primary']) {
      final nodes =
          _subtree(tester.getSemantics(find.byKey(Key('selected-row-$action'))))
              .toList();
      final node = nodes.firstWhere(
          (node) => node.getSemanticsData().hasAction(ui.SemanticsAction.tap));
      expect(nodes.map((node) => node.getSemanticsData().label).join(),
          isNotEmpty);
      node.owner!.performAction(node.id, ui.SemanticsAction.tap);
    }
    expect(invoked, ['preview', 'actions', 'primary']);
    await mouse.removePointer();
    await tester.pumpWidget(const SizedBox.shrink());
    await tester.pumpAndSettle();
    expect(binding.recorder!.failures, isEmpty);
  });

  testWidgets('tooltip updates survive launcher retention and editor expiry',
      (tester) async {
    var clock = 0;
    var issued = 0;
    LauncherStateDto? held;
    LauncherStateDto state(int session, int revision, String input) =>
        held = launcherSnapshot(
            sessionId: session,
            revision: revision,
            snapshotSeq: 1,
            input: input,
            contextComplete: true,
            candidates: [candidate('copy', 'Copy synthetic path')],
            diagnostics: []);
    final controller = LauncherController(
        nowMillis: () => clock,
        window: WindowHost(channel),
        backend: LauncherBackend(
            beginActivation: () => ++issued,
            activate: (session,
                    {promptAccessibility = false,
                    capturedEnvironment = false}) async =>
                state(session, 0, ''),
            updateInput: (session, input, revision,
                    {scopeId, inputMode = InputModeDto.environment}) async =>
                state(session, revision, input),
            execute: (_, __, ___) async => throw StateError('unused'),
            deactivate: (_) async {},
            suspend: (_) async {},
            resume: (_) async => held));
    addTearDown(controller.dispose);
    await tester.pumpWidget(LocalizedApp(
        theme: AppStyle.theme, home: LauncherScreen(controller: controller)));
    await controller.activate(mode: WakeMode.universal);
    await tester.pump();
    final navigation = _subtree(
            tester.getSemantics(find.byKey(const Key('launcher-navigation'))))
        .firstWhere((node) =>
            node.getSemanticsData().hasAction(ui.SemanticsAction.tap));
    expect(navigation.getSemanticsData().label,
        contains('Global Search / Current App'));
    navigation.owner!.performAction(navigation.id, ui.SemanticsAction.tap);
    await tester.pump();
    expect(controller.view.navigationOpen, isTrue);
    navigation.owner!.performAction(navigation.id, ui.SemanticsAction.tap);
    await tester.pump();
    expect(controller.view.navigationOpen, isFalse);
    for (final seconds in [14, 16, 16]) {
      await controller.update('synthetic');
      await tester.pump();
      for (final tooltip
          in tester.stateList<TooltipState>(find.byType(Tooltip))) {
        tooltip.ensureTooltipVisible();
        await tester.pump(const Duration(milliseconds: 200));
      }
      await controller.dismiss();
      clock += seconds * 1000;
      await tester.pump(Duration(seconds: seconds));
      await tester.pump();
      expect(controller.hasRetainedState, seconds < 15);
      await controller.activate(mode: WakeMode.universal);
      await tester.pump();
      expect(controller.view.input, seconds < 15 ? 'synthetic' : '');
      await tester.enterText(find.byType(TextField), 'after-$seconds');
      await tester.pump(const Duration(milliseconds: 600));
      final nodes = _subtree(tester.getSemantics(find.byType(TextField)));
      expect(nodes.map((node) => node.getSemanticsData().value),
          contains('after-$seconds'));
      expect(binding.recorder!.failures, isEmpty);
    }
    await tester.pumpWidget(const SizedBox.shrink());
    await tester.pumpAndSettle();
    expect(binding.recorder!.failures, isEmpty);
  });
}
