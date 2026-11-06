import 'dart:async';

import 'package:anywhere/launcher/launcher_backend.dart';
import 'package:anywhere/launcher/launcher_session_coordinator.dart';
import 'package:anywhere/src/rust/api.dart';
import 'package:flutter_test/flutter_test.dart';

import 'launcher_fixtures.dart';
import 'launcher_test.dart' show candidate;

LauncherStateDto state(int sequence,
        {int session = 7,
        int revision = 1,
        int version = 1,
        List<CandidateDto> rows = const []}) =>
    launcherSnapshot(
        sessionId: session,
        revision: revision,
        snapshotSeq: sequence,
        candidateListVersion: version,
        input: 'report',
        contextComplete: true,
        candidates: rows,
        diagnostics: []);

LauncherBackend backend(
        {WaitForSnapshot? wait,
        Future<SnapshotWaitDto> Function(int, int, int, int)? waitDelta}) =>
    LauncherBackend(
        beginActivation: () => 7,
        activate: (_,
                {promptAccessibility = false,
                capturedEnvironment = false}) async =>
            null,
        updateInput: (_, __, ___,
                {scopeId, inputMode = InputModeDto.environment}) async =>
            null,
        execute: (_, __, ___) async => throw StateError('not used'),
        deactivate: (_) async {},
        waitForSnapshot: wait,
        waitForSnapshotDelta: waitDelta);

void main() {
  test('snapshots cannot cross sessions or supersede newer input and sequence',
      () {
    final flow = LauncherSessionCoordinator(() => 0);
    addTearDown(flow.dispose);
    flow.attachSession(7);
    flow.acceptRevision(1);
    final initial = state(2);
    expect(flow.acceptSnapshot(initial), isTrue);
    expect(flow.acceptSnapshot(state(3, session: 8)), isFalse);
    expect(flow.acceptSnapshot(state(2)), isFalse);
    expect(flow.acceptSnapshot(state(1)), isFalse);
    expect(flow.beginRevision(), 2);
    expect(flow.acceptSnapshot(state(99)), isFalse);
    expect(identical(flow.snapshot, initial), isTrue);

    expect(
        flow.acceptSnapshot(state(1, revision: 3), followInput: true), isTrue);
    expect(flow.requestedRevision, 3);
    expect(flow.beginRevision(), 4);
  });

  test('a delta cannot append to a list replaced while its wait was pending',
      () async {
    final flow = LauncherSessionCoordinator(() => 0);
    addTearDown(flow.dispose);
    flow.attachSession(7);
    flow.acceptRevision(1);
    flow.acceptSnapshot(state(1, rows: [candidate('old', 'Old')]));
    final pending = [
      Completer<SnapshotWaitDto>(),
      Completer<SnapshotWaitDto>()
    ];
    final secondWait = Completer<void>();
    final requests = <(int, int, int)>[];
    var visible = true;
    final published = <LauncherStateDto>[];
    final following = flow.follow(
        backend: backend(waitDelta: (_, cursor, version, count) {
          requests.add((cursor, version, count));
          if (requests.length == 2) secondWait.complete();
          return pending[requests.length - 1].future;
        }),
        generation: flow.generation,
        session: 7,
        visible: () => visible,
        publish: (next) async {
          if (flow.acceptSnapshot(next)) published.add(next);
          visible = false;
        });
    final replacement =
        state(2, version: 2, rows: [candidate('replacement', 'Replacement')]);
    flow.acceptSnapshot(replacement);
    pending[0].complete(SnapshotWaitDto(
        kind: SnapshotWaitKindDto.updated,
        candidateUpdate: CandidateUpdateDto.append,
        state: state(3, rows: [candidate('tail', 'Stale tail')])));
    await secondWait.future;
    expect(published, isEmpty);
    expect(requests.last, (snapshotCursor(replacement), 2, 1));
    pending[1].complete(SnapshotWaitDto(
        kind: SnapshotWaitKindDto.updated,
        candidateUpdate: CandidateUpdateDto.retain,
        state: state(4, version: 2)));
    await following;
    expect(published.single.candidates, same(replacement.candidates));
  });

  test(
      'a wait from an invalidated generation cannot publish into a new session',
      () async {
    final flow = LauncherSessionCoordinator(() => 0);
    addTearDown(flow.dispose);
    flow.attachSession(7);
    flow.acceptRevision(1);
    flow.acceptSnapshot(state(1));
    final pending = Completer<SnapshotWaitDto>();
    final published = <LauncherStateDto>[];
    final following = flow.follow(
        backend: backend(wait: (_, __) => pending.future),
        generation: flow.generation,
        session: 7,
        visible: () => true,
        publish: (next) async => published.add(next));
    flow.invalidateGeneration();
    flow.clearSession();
    flow.attachSession(8);
    final replacement = state(1, session: 8);
    flow.acceptSnapshot(replacement);
    pending.complete(
        SnapshotWaitDto(kind: SnapshotWaitKindDto.updated, state: state(2)));
    await following;
    expect(published, isEmpty);
    expect(flow.snapshot, same(replacement));
  });
}
