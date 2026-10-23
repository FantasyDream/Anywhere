import 'dart:async';

import 'package:anywhere/infrastructure/window_host.dart';
import 'package:anywhere/launcher/launcher_controller.dart';
import 'package:anywhere/launcher/launcher_view_state.dart';
import 'package:anywhere/src/rust/api.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

import 'launcher_fixtures.dart';

const channel = MethodChannel('test/retention-races');

class RetentionHarness {
  int clock = 0, issued = 0;
  final states = <int, LauncherStateDto>{};
  final suspensions = <int, Completer<void>>{};
  final resumptions = <int, Completer<LauncherStateDto?>>{};
  final discarded = <int>[];
  late final controller = LauncherController(
      nowMillis: () => clock,
      window: WindowHost(channel),
      backend: LauncherBackend(
          beginActivation: () => ++issued,
          activate: (id,
                  {promptAccessibility = false,
                  capturedEnvironment = false}) async =>
              snapshot(id, 0, ''),
          updateInput: (id, text, revision,
                  {scopeId, inputMode = InputModeDto.environment}) async =>
              snapshot(id, revision, text),
          execute: (_, __, ___) async => throw StateError('not used'),
          deactivate: (id) async => discarded.add(id),
          suspend: (id) => suspensions[id]?.future ?? Future.value(),
          resume: (id) async => resumptions[id] == null
              ? states[id]
              : await resumptions[id]!.future));

  LauncherStateDto snapshot(int id, int revision, String input) =>
      states[id] = launcherSnapshot(
          sessionId: id,
          revision: revision,
          snapshotSeq: 1,
          input: input,
          contextComplete: true,
          candidates: [],
          diagnostics: []);

  Future<void> begin(String input) async {
    await controller.activate(mode: WakeMode.universal);
    await controller.update(input);
  }
}

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  setUp(() => TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
      .setMockMethodCallHandler(channel, (_) async => null));
  tearDown(() => TestDefaultBinaryMessengerBinding
      .instance.defaultBinaryMessenger
      .setMockMethodCallHandler(channel, null));

  testWidgets(
      'hidden notification and duplicate dismiss preserve the workspace',
      (tester) async {
    final h = RetentionHarness();
    addTearDown(h.controller.dispose);
    await h.begin('kept');
    final hiddenNotifications = <bool>[];
    h.controller.addListener(() {
      if (h.controller.view.phase == LauncherPhase.hidden) {
        hiddenNotifications.add(h.controller.hasRetainedState);
      }
    });
    await h.controller.dismiss(hostAlreadyHidden: true);
    expect(hiddenNotifications, [true]);
    final generation = h.controller.view.generation;
    await h.controller.dismiss(hostAlreadyHidden: true);
    expect(h.controller.view.generation, generation);
    expect(h.controller.hasRetainedState, isTrue);
    h.clock = 14000;
    await h.controller.activate();
    expect(h.issued, 1);
    expect(h.controller.view.input, 'kept');
  });

  for (final waiting in ['suspend', 'resume']) {
    testWidgets('retention expires while awaiting $waiting', (tester) async {
      final h = RetentionHarness();
      addTearDown(h.controller.dispose);
      await h.begin('expired');
      final suspension = Completer<void>();
      final resumption = Completer<LauncherStateDto?>();
      if (waiting == 'suspend') {
        h.suspensions[1] = suspension;
      } else {
        h.resumptions[1] = resumption;
      }
      final hide = h.controller.dismiss(hostAlreadyHidden: true);
      h.clock = 14000;
      final wake = h.controller.activate();
      await tester.pump();
      h.clock = 15000;
      if (waiting == 'suspend') {
        suspension.complete();
      } else {
        resumption.complete(h.states[1]);
      }
      await hide;
      await wake;
      expect(h.issued, 2);
      expect(h.controller.view.input, isEmpty);
      expect(h.discarded, [1]);
      await tester.pump(const Duration(seconds: 1));
    });
  }

  for (final finishOldFirst in [true, false]) {
    testWidgets(
        'old cleanup cannot affect a newer retained session: '
        'old finishes first=$finishOldFirst', (tester) async {
      final h = RetentionHarness();
      addTearDown(h.controller.dispose);
      await h.begin('old');
      final suspension = h.suspensions[1] = Completer<void>();
      final oldHide = h.controller.dismiss(hostAlreadyHidden: true);
      h.clock = 15000;
      await tester.pump(const Duration(seconds: 15));
      await h.begin('new');
      expect(h.issued, 2);
      await h.controller.dismiss(hostAlreadyHidden: true);
      if (finishOldFirst) {
        suspension.complete();
        await oldHide;
        await tester.pump();
        expect(h.controller.hasRetainedState, isTrue);
        expect(h.controller.view.input, 'new');
        expect(h.discarded, isEmpty);
      }
      h.clock = 30000;
      await tester.pump(const Duration(seconds: 15));
      await tester.pump();
      expect(h.controller.hasRetainedState, isFalse);
      expect(h.discarded, [2]);
      if (!finishOldFirst) {
        suspension.complete();
        await oldHide;
        await tester.pump();
        expect(h.discarded, [2]);
      }
    });
  }

  testWidgets('a delayed native hide awaits only its own suspension',
      (tester) async {
    final h = RetentionHarness();
    addTearDown(h.controller.dispose);
    final nativeHide = Completer<void>();
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(channel, (call) async {
      if (call.method == 'hide') await nativeHide.future;
      return null;
    });
    await h.begin('old');
    final first = h.suspensions[1] = Completer<void>();
    var oldHideFinished = false;
    final oldHide = h.controller.dismiss().then((_) => oldHideFinished = true);
    h.clock = 15000;
    await tester.pump(const Duration(seconds: 15));
    await h.begin('new');
    final second = h.suspensions[2] = Completer<void>();
    final newHide = h.controller.dismiss(hostAlreadyHidden: true);
    nativeHide.complete();
    await tester.pump();
    first.complete();
    await tester.pump();
    final completedOwnSuspension = oldHideFinished;
    second.complete();
    await oldHide;
    await newHide;
    h.clock = 30000;
    await tester.pump(const Duration(seconds: 15));
    await tester.pump();
    expect(completedOwnSuspension, isTrue,
        reason: 'a previous window hide must not await the next session');
  });
}
