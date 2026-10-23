import 'dart:async';

import 'package:anywhere/infrastructure/desktop_environment.dart';
import 'package:anywhere/infrastructure/window_host.dart';
import 'package:anywhere/launcher/host_view_navigator.dart';
import 'package:anywhere/launcher/launcher_controller.dart';
import 'package:anywhere/launcher/launcher_screen.dart';
import 'package:anywhere/launcher/launcher_session_coordinator.dart';
import 'package:anywhere/launcher/launcher_view_state.dart';
import 'package:anywhere/launcher/object_action_coordinator.dart';
import 'package:anywhere/launcher/stable_list.dart';
import 'package:anywhere/src/rust/api.dart';
import 'package:anywhere/ui/app_style.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

import 'launcher_fixtures.dart';

CandidateDto file(int i) => CandidateDto(
    id: 'file:$i',
    title: 'file-$i.txt',
    kind: CandidateKindDto.file,
    iconKey: 'file',
    groupId: 'files',
    score: 1,
    availability: AvailabilityDto.enabled,
    primaryAction: true,
    homeEligible: false,
    menuPath: const [],
    contextKeys: const [],
    executionContextId: 'file:$i');

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  testWidgets(
      'module-owned objects use the same Actions keyboard flow and preserve query',
      (tester) async {
    DesktopEnvironment.configure(
        const DesktopEnvironment(platform: TargetPlatform.macOS));
    addTearDown(DesktopEnvironment.reset);
    const channel = MethodChannel('test/bookmark-actions');
    final messenger =
        TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger;
    messenger.setMockMethodCallHandler(channel, (_) async => null);
    addTearDown(() => messenger.setMockMethodCallHandler(channel, null));
    CandidateDto object(String command, String title, bool primary) =>
        CandidateDto(
            id: command,
            commandId: command,
            title: title,
            kind: CandidateKindDto.action,
            iconKey: 'folder',
            groupId: 'folder',
            score: 1,
            availability: AvailabilityDto.enabled,
            menuPath: const [],
            contextKeys: const [],
            primaryAction: primary,
            homeEligible: false,
            executionContextId: 'saved-object');
    final bookmark = object('saved_directories.open', 'remove draft', true);
    final forget =
        object('saved_directories.forget', 'Forget Saved Directory', false);
    LauncherStateDto state(int revision, String input) => launcherSnapshot(
        sessionId: 1,
        revision: revision,
        snapshotSeq: 1,
        input: input,
        contextComplete: true,
        candidates: input.isEmpty ? [] : [bookmark],
        diagnostics: const []);
    final targets = <String>[];
    final c = LauncherController(
        window: WindowHost(channel),
        readExtensions: () => [],
        backend: LauncherBackend(
            beginActivation: () => 1,
            activate: (_,
                    {promptAccessibility = false,
                    capturedEnvironment = false}) async =>
                state(0, ''),
            updateInput: (_, input, revision,
                    {scopeId, inputMode = InputModeDto.environment}) async =>
                state(revision, input),
            execute: (_, __, ___) async =>
                throw StateError('No action executed in navigation test'),
            deactivate: (_) async {},
            objectActions: (_, __, id) async {
              targets.add(id);
              return [forget];
            }));
    addTearDown(c.dispose);
    await tester.pumpWidget(MaterialApp(
        theme: AppStyle.theme,
        builder: AppStyle.windowBuilder,
        home: LauncherScreen(controller: c)));
    await c.activate(mode: WakeMode.universal);
    await tester.pumpAndSettle();
    await tester.enterText(find.byType(TextField), 'remove draft');
    await tester.pumpAndSettle();
    expect(c.canShowObjectActions, isTrue);
    await tester.sendKeyDownEvent(LogicalKeyboardKey.metaLeft);
    await tester.sendKeyEvent(LogicalKeyboardKey.keyK);
    await tester.sendKeyUpEvent(LogicalKeyboardKey.metaLeft);
    await tester.pumpAndSettle();
    expect(targets, [bookmark.id]);
    expect(find.text(forget.title), findsOneWidget);
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pumpAndSettle();
    expect(c.view.input, 'remove draft');
    expect(
        c.view.visibleEntries[c.view.selectedIndex].candidate!.id, bookmark.id);
    expect(c.view.objectActionTitle, isNull);
    expect(tester.takeException(), isNull);
  });
  test('append shares complete chunks and preserves immutable predecessor', () {
    final first = StableList<int>.from(List.generate(1024, (i) => i));
    final second = first.append([1024, 1025]);
    expect(identical(first.chunks.first, second.chunks.first), isTrue);
    expect(first.length, 1024);
    expect(second.length, 1026);
    expect(second[1025], 1025);
    expect(() => second[0] = 4, throwsUnsupportedError);
    expect(() => second.chunks[0][0] = 4, throwsUnsupportedError);
  });

  test(
      'stable tail transport rejects mismatched base and shares presentation indexes',
      () {
    final rows = StableList<CandidateDto>.from(List.generate(1024, file));
    LauncherStateDto snapshot(List<CandidateDto> candidates, int seq) =>
        LauncherStateDto(
            sessionId: 1,
            revision: 1,
            snapshotSeq: seq,
            input: 'query',
            contextComplete: true,
            diagnostics: const [],
            candidateListVersion: 7,
            candidates: candidates,
            searchSources: const [],
            resultActions: const [],
            contexts: const [],
            contextCandidates: const [],
            scopes: const [],
            contextCaptures: const []);
    final held = snapshot(rows, 1), tail = snapshot([file(1024)], 2);
    final packet = SnapshotWaitDto(
        state: tail,
        kind: SnapshotWaitKindDto.updated,
        candidateUpdate: CandidateUpdateDto.append);
    expect(LauncherSessionCoordinator.materialize(held, packet, 0), isNull);
    final next =
        LauncherSessionCoordinator.materialize(held, packet, rows.length)!;
    final before = LauncherEntries.files(rows),
        after = LauncherEntries.files(next.candidates);
    expect(before.rows.length, 1024);
    expect(after.rows.length, 1025);
    expect(after.indices['candidate:file:1024'], 1024);
    expect(
        identical(
            before.indices.chunks.first.$2, after.indices.chunks.first.$2),
        isTrue);
    expect(after.rows[1024].candidate!.id, 'file:1024');
  });

  test('late object action responses cannot reopen a closed or newer target',
      () async {
    final flow = ObjectActionCoordinator();
    final old = Completer<List<CandidateDto>>();
    final pending = flow.load(() => old.future, () => true);
    flow.invalidate();
    old.complete([file(1)]);
    expect(await pending, isNull);
    final newer = await flow.load(() async => [file(2)], () => true);
    expect(newer!.single.id, 'file:2');
  });

  test('host views register one adapter and reject unavailable routes',
      () async {
    final navigation = HostViewNavigator();
    var opened = 0;
    expect(navigation.accepts(null), isFalse);
    navigation.register(HostViewDto.fileSearch, () async {
      opened++;
    });
    expect(() => navigation.register(HostViewDto.fileSearch, () async {}),
        throwsArgumentError);
    await navigation.open(HostViewDto.fileSearch);
    expect(opened, 1);
    await expectLater(navigation.open(null), throwsStateError);
  });

  test(
      '1k and 10k streamed results retain stable keyboard lookup without full row copies',
      () {
    for (final count in [1000, 10000]) {
      var rows = StableList<CandidateDto>.from(const []);
      final timer = Stopwatch()..start();
      for (var offset = 0; offset < count; offset += 64) {
        final end = (offset + 64).clamp(0, count);
        rows =
            rows.append(List.generate(end - offset, (i) => file(offset + i)));
        final entries = LauncherEntries.files(rows);
        expect(entries.indices['candidate:file:${end - 1}'], end - 1);
        expect(entries.rows[end - 1].candidate!.id, 'file:${end - 1}');
      }
      timer.stop();
      expect(rows.length, count);
      // Structural bound, rather than a machine-dependent timing threshold.
      expect(rows.chunks.length, (count / StableList.chunkSize).ceil());
      // ignore: avoid_print
      print(
          'Stable file list: $count candidates / ${rows.chunks.length} chunks in ${timer.elapsedMicroseconds}us');
    }
  });
}
