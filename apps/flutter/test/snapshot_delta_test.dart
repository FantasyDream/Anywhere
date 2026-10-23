import 'package:anywhere/launcher/launcher_session_coordinator.dart';
import 'package:anywhere/launcher/launcher_view_state.dart';
import 'package:anywhere/src/rust/api.dart';
import 'package:flutter_test/flutter_test.dart';

import 'launcher_fixtures.dart';
import 'launcher_test.dart' show candidate;

void main() {
  LauncherStateDto state(int seq, List<CandidateDto> rows,
          {int version = 1, int revision = 1}) =>
      launcherSnapshot(
          sessionId: 7,
          revision: revision,
          snapshotSeq: seq,
          candidateListVersion: version,
          input: 'report',
          contextComplete: true,
          candidates: rows,
          diagnostics: []);
  test(
      'metadata retains row identity; tails append; missed replacements fail closed',
      () {
    final row = candidate('first', 'report.txt',
        kind: CandidateKindDto.file, fileKind: FileObjectKindDto.file);
    final held = state(1, [row]);
    final metadata = SnapshotWaitDto(
        kind: SnapshotWaitKindDto.updated,
        candidateUpdate: CandidateUpdateDto.retain,
        state: state(2, []));
    final updated = LauncherSessionCoordinator.materialize(held, metadata, 1)!;
    expect(identical(updated.candidates, held.candidates), isTrue);
    final second = candidate('second', 'report2.txt',
        kind: CandidateKindDto.file, fileKind: FileObjectKindDto.file);
    final tail = SnapshotWaitDto(
        kind: SnapshotWaitKindDto.updated,
        candidateUpdate: CandidateUpdateDto.append,
        state: state(3, [second]));
    final appended = LauncherSessionCoordinator.materialize(updated, tail, 1)!;
    expect(appended.candidates.map((c) => c.id), ['first', 'second']);
    expect(LauncherSessionCoordinator.materialize(appended, tail, 1), isNull);
    expect(
        LauncherSessionCoordinator.materialize(
            state(1, [row], version: 2), metadata, 1),
        isNull);
    expect(
        LauncherSessionCoordinator.materialize(
            state(1, [row], revision: 2), metadata, 1),
        isNull);
    expect(
        LauncherSessionCoordinator.materialize(
                null,
                SnapshotWaitDto(
                    kind: SnapshotWaitKindDto.updated,
                    state: state(5, [second], version: 2)),
                0)!
            .candidates
            .single
            .id,
        'second');
  });
  test('large progress views reuse row collection and selection lookup', () {
    final rows = List.generate(
        10000, (i) => candidate('$i', 'report$i', kind: CandidateKindDto.file));
    final before = LauncherViewState(
        phase: LauncherPhase.ready,
        generation: 1,
        input: 'report',
        inputMode: InputModeDto.fileSearch,
        selectedId: 'candidate:9999',
        snapshot: state(1, rows));
    final collection = before.entryCollection;
    final packet = SnapshotWaitDto(
        kind: SnapshotWaitKindDto.updated,
        candidateUpdate: CandidateUpdateDto.retain,
        state: state(2, []));
    final after = LauncherViewState(
        phase: LauncherPhase.ready,
        generation: 1,
        input: 'report',
        inputMode: InputModeDto.fileSearch,
        selectedId: 'candidate:9999',
        snapshot: LauncherSessionCoordinator.materialize(
            before.snapshot, packet, 10000),
        retainedFileEntries: collection);
    expect(identical(after.entryCollection, collection), isTrue);
    expect(after.selectedIndex, 9999);
  });
  test(
      'explicit menu role works for root leaves and does not consume app plugin IDs',
      () {
    final ordinary = candidate('action:app.search:c1', 'Plugin Search',
        groupId: 'plugin.app');
    final menu = candidate('action:opaque.menu:c2', 'Root Leaf',
        groupId: 'app.editor', isMenu: true);
    final offers = [ordinary, menu];
    final snapshot = launcherSnapshot(
        sessionId: 1,
        revision: 0,
        snapshotSeq: 1,
        input: '',
        contextComplete: true,
        candidates: offers,
        diagnostics: [],
        scopes: [
          ScopeDto(id: 'plugin.app', label: 'Plugin', isCurrentApp: false),
          ScopeDto(id: 'app.editor', label: 'Editor', isCurrentApp: true)
        ]);
    final plugin = LauncherViewState(
        phase: LauncherPhase.ready,
        generation: 1,
        scopeId: 'plugin.app',
        snapshot: snapshot);
    expect(plugin.visibleCandidates.map((c) => c.id), [ordinary.id]);
    expect(plugin.canBrowseMenus, isFalse);
    final app = LauncherViewState(
        phase: LauncherPhase.ready,
        generation: 1,
        scopeId: 'app.editor',
        snapshot: snapshot);
    expect(app.menuActions.single.id, menu.id);
    expect(app.canBrowseMenus, isTrue);
    expect(menu.menuPath, isEmpty);
  });
}
