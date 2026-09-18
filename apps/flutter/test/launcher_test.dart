import 'dart:async';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:anywhere/launcher_screen.dart';
import 'package:anywhere/src/rust/api.dart';

/// Two action candidates with the documented stable ids.
CandidateDto candidate(
  String id,
  String title, {
  String kind = 'action',
  String availability = 'enabled',
  String? reason,
  String iconKey = 'file',
  String groupId = 'folder',
  double score = 100,
}) =>
    CandidateDto(
      id: id,
      kind: kind,
      title: title,
      subtitle: 'from Rust',
      iconKey: iconKey,
      score: score,
      groupId: groupId,
      availability: availability,
      availabilityReason: reason,
      commandId: kind == 'action' ? id.split(':')[1] : null,
      bundleId: kind == 'application' ? 'com.example.${id.split(':')[1]}' : null,
    );

LauncherStateDto state(
  int revision,
  String title, {
  int snapshotSeq = 1,
  bool contextComplete = true,
  String input = '',
}) {
  final first = candidate('action:first:c1', title);
  final second = candidate('action:second:c1', 'Second action', iconKey: 'copy');
  return LauncherStateDto(
    sessionId: 1,
    revision: revision,
    snapshotSeq: snapshotSeq,
    input: input,
    contextComplete: contextComplete,
    contexts: [],
    candidates: [first, second],
    scopes: const [],
    diagnostics: [],
  );
}

/// Smallest valid PNG (1x1, transparent) so `Image.memory` has real bytes.
const _onePixelPng = <int>[
  0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, //
  0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52,
  0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01,
  0x08, 0x06, 0x00, 0x00, 0x00, 0x1F, 0x15, 0xC4,
  0x89, 0x00, 0x00, 0x00, 0x0A, 0x49, 0x44, 0x41,
  0x54, 0x78, 0x9C, 0x63, 0x00, 0x01, 0x00, 0x00,
  0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4, 0x00,
  0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE,
  0x42, 0x60, 0x82,
];

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  const channel = MethodChannel('test/launcher-window');
  const codec = StandardMethodCodec();
  late List<String> windowCalls;
  late List<MethodCall> windowMessages;
  final messenger =
      TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger;
  setUp(() {
    windowCalls = [];
    windowMessages = [];
    messenger.setMockMethodCallHandler(channel, (call) async {
      windowCalls.add(call.method);
      windowMessages.add(call);
      return null;
    });
  });
  tearDown(() => messenger.setMockMethodCallHandler(channel, null));

  Future<void> toggle(WidgetTester tester, {String mode = 'currentApp'}) async {
    final done = Completer<void>();
    messenger.handlePlatformMessage(
        channel.name,
        codec.encodeMethodCall(MethodCall('toggle', {'mode': mode})), (_) {
      done.complete();
    });
    await tester.pumpAndSettle();
    await done.future;
  }

  Future<void> dismiss(WidgetTester tester, int generation) async {
    final done = Completer<void>();
    messenger.handlePlatformMessage(
        channel.name,
        codec.encodeMethodCall(
            MethodCall('dismiss', {'generation': generation})),
        (_) => done.complete());
    await tester.pumpAndSettle();
    await done.future;
  }

  int presentedGeneration() =>
      (windowMessages.lastWhere((call) => call.method == 'show').arguments
          as Map)['generation'] as int;

  LauncherStateDto sizedState(int revision, int count,
      {String scopeFilter = '', String input = ''}) {
    final candidates = List.generate(
        count,
        (i) => candidate('action:command.$i:$scopeFilter',
            scopeFilter.isEmpty ? 'Action $i' : '$scopeFilter $i',
            score: 100.0 - i));
    return LauncherStateDto(
      sessionId: 1,
      revision: revision,
      snapshotSeq: 1,
      input: input,
      contextComplete: true,
      contexts: [],
      candidates: candidates,
      scopes: const [],
      diagnostics: [],
    );
  }

  testWidgets(
      'empty results collapse and candidate rows grow up to a scrollable maximum',
      (tester) async {
    await tester.pumpWidget(LauncherApp(
        window: channel,
        beginActivation: () => 1,
        activate: (_) async => sizedState(0, 0),
        updateInput: (_, input, revision) async =>
            sizedState(revision, int.parse(input), input: input),
        execute: (_, revision, candidate) async =>
            throw StateError('not invoked'),
        deactivate: (_) async {}));
    await toggle(tester);
    expect(find.byKey(const Key('action-list')), findsNothing);
    expect(find.byKey(const Key('empty-state')), findsNothing);
    final emptyHeight =
        tester.getSize(find.byKey(const Key('launcher-panel'))).height;
    expect(emptyHeight, 64);
    await tester.enterText(find.byType(TextField), '2');
    await tester.pumpAndSettle();
    final twoRows =
        tester.getSize(find.byKey(const Key('launcher-panel'))).height;
    await tester.enterText(find.byType(TextField), '12');
    await tester.pumpAndSettle();
    final manyRows =
        tester.getSize(find.byKey(const Key('launcher-panel'))).height;
    expect(manyRows, greaterThan(twoRows));
    expect(manyRows, lessThanOrEqualTo(520));
    final list = tester.widget<ListView>(find.byKey(const Key('action-list')));
    expect(list.controller!.position.maxScrollExtent, greaterThan(0));
    await tester.sendKeyEvent(LogicalKeyboardKey.arrowUp);
    await tester.pumpAndSettle();
    expect(find.text('Action 11'), findsOneWidget);
    await tester.enterText(find.byType(TextField), '0');
    await tester.pumpAndSettle();
    expect(find.byKey(const Key('action-list')), findsNothing);
    expect(find.byKey(const Key('empty-state')), findsNothing);
    expect(
        tester.getSize(find.byKey(const Key('launcher-panel'))).height,
        emptyHeight);
  });

  testWidgets('an empty search collapses to the search bar',
      (tester) async {
    await tester.pumpWidget(LauncherApp(
        window: channel,
        beginActivation: () => 1,
        activate: (_) async => sizedState(0, 0),
        updateInput: (_, input, revision) async => sizedState(revision, 0),
        execute: (_, revision, candidate) async =>
            throw StateError('not invoked'),
        deactivate: (_) async {}));
    await toggle(tester);
    expect(find.text('No actions for this context'), findsNothing);
    expect(find.byKey(const Key('empty-state')), findsNothing);
    expect(tester.getSize(find.byKey(const Key('launcher-panel'))).height, 64);
    await tester.enterText(find.byType(TextField), 'zzz');
    await tester.pumpAndSettle();
    expect(find.textContaining('zzz'), findsWidgets);
    expect(find.textContaining('match'), findsNothing);
    expect(tester.getSize(find.byKey(const Key('launcher-panel'))).height, 64);
  });

  testWidgets('an unresolved context stays on the search bar',
      (tester) async {
    await tester.pumpWidget(LauncherApp(
        window: channel,
        beginActivation: () => 1,
        activate: (_) async => LauncherStateDto(
              sessionId: 1,
              revision: 0,
              snapshotSeq: 1,
              input: '',
              contextComplete: false,
              contexts: [],
              candidates: [],
              scopes: const [],
              diagnostics: [],
            ),
        updateInput: (_, input, revision) async => sizedState(revision, 0),
        execute: (_, revision, candidate) async =>
            throw StateError('not invoked'),
        deactivate: (_) async {}));
    await toggle(tester);
    expect(find.textContaining('Looking for actions'), findsNothing);
    expect(find.byKey(const Key('empty-state')), findsNothing);
  });

  testWidgets(
      'blur invalidates pending input and stale blur cannot dismiss a fresh activation',
      (tester) async {
    final delayed = Completer<LauncherStateDto?>();
    var captures = 0;
    var invalidations = 0;
    await tester.pumpWidget(LauncherApp(
        window: channel,
        beginActivation: () => 1,
        activate: (_) async => state(0, 'Capture ${++captures}'),
        updateInput: (_, input, revision) => delayed.future,
        execute: (_, revision, candidate) async =>
            throw StateError('not invoked'),
        deactivate: (_) async {
          invalidations++;
        }));
    await toggle(tester);
    final oldGeneration = presentedGeneration();
    await tester.enterText(find.byType(TextField), 'old input');
    await tester.pump();
    await dismiss(tester, oldGeneration);
    expect(invalidations, 1);
    expect(find.byKey(const Key('action-list')), findsNothing);
    delayed.complete(state(1, 'Expired action'));
    await tester.pumpAndSettle();
    expect(find.text('Expired action'), findsNothing);
    expect(windowCalls.where((method) => method == 'show').length, 1);
    await toggle(tester);
    expect(find.text('Capture 2'), findsOneWidget);
    expect(tester.widget<TextField>(find.byType(TextField)).controller!.text,
        isEmpty);
    await dismiss(tester, oldGeneration);
    expect(invalidations, 1);
    expect(find.text('Capture 2'), findsOneWidget);
  });

  testWidgets(
      'blur during execution cannot reopen or overwrite the next session',
      (tester) async {
    final pending = Completer<ActionResultDto>();
    var captures = 0;
    await tester.pumpWidget(LauncherApp(
        window: channel,
        beginActivation: () => 1,
        activate: (_) async => state(0, 'Capture ${++captures}'),
        updateInput: (_, input, revision) async => state(revision, input, input: input),
        execute: (_, revision, candidate) => pending.future,
        deactivate: (_) async {}));
    await toggle(tester);
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await dismiss(tester, presentedGeneration());
    await toggle(tester);
    pending.complete(ActionResultDto(
        sessionId: 1,
        revision: 0,
        success: true,
        message: 'Done',
        outputText: null,
        closeWindow: true));
    await tester.pumpAndSettle();
    expect(find.text('Capture 2'), findsOneWidget);
    expect(windowCalls, isNot(contains('hide')));
  });

  testWidgets('window waits for capture and ranking before showing',
      (tester) async {
    final captured = Completer<LauncherStateDto?>();
    await tester.pumpWidget(LauncherApp(
      window: channel,
      beginActivation: () => 1,
      activate: (_) => captured.future,
      updateInput: (_, input, revision) async => state(revision, input, input: input),
      execute: (_, revision, candidate) async =>
          throw StateError('not invoked'),
      deactivate: (_) async {},
    ));
    final activation = Completer<void>();
    messenger.handlePlatformMessage(
        channel.name, codec.encodeMethodCall(const MethodCall('toggle')), (_) {
      activation.complete();
    });
    await tester.pump();
    expect(windowCalls, isNot(contains('show')));
    captured.complete(state(0, 'Captured Finder directory'));
    await tester.pumpAndSettle();
    await activation.future;
    expect(windowCalls.last, 'show');
    expect(find.text('Captured Finder directory'), findsOneWidget);
  });

  testWidgets(
      'keyboard navigation, execution, and Esc use the supplied Rust state',
      (tester) async {
    final executed = <String>[];
    var deactivated = 0;
    await tester.pumpWidget(LauncherApp(
      window: channel,
      beginActivation: () => 1,
      activate: (_) async => state(0, 'Open Terminal Here'),
      updateInput: (_, input, revision) async => state(revision, input, input: input),
      execute: (session, revision, candidate) async {
        executed.add(candidate);
        return ActionResultDto(
            sessionId: session,
            revision: revision,
            success: true,
            message: 'Done',
            outputText: null,
            closeWindow: false);
      },
      deactivate: (_) async {
        deactivated++;
      },
    ));
    await toggle(tester);
    expect(
        windowCalls.indexOf('show'), greaterThan(windowCalls.indexOf('ready')));
    expect(find.text('Open Terminal Here'), findsOneWidget);
    await tester.sendKeyEvent(LogicalKeyboardKey.arrowDown);
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pumpAndSettle();
    expect(executed, ['action:second:c1']);
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pumpAndSettle();
    expect(deactivated, 1);
    expect(windowCalls.last, 'hide');
  });

  testWidgets('out-of-order input responses never replace the latest revision',
      (tester) async {
    final old = Completer<LauncherStateDto?>();
    final fresh = Completer<LauncherStateDto?>();
    await tester.pumpWidget(LauncherApp(
      window: channel,
      beginActivation: () => 1,
      activate: (_) async => state(0, 'Initial action'),
      updateInput: (_, input, revision) =>
          revision == 1 ? old.future : fresh.future,
      execute: (_, revision, candidate) async =>
          throw StateError('not invoked'),
      deactivate: (_) async {},
    ));
    await toggle(tester);
    await tester.enterText(find.byType(TextField), 'old');
    await tester.enterText(find.byType(TextField), 'fresh');
    fresh.complete(state(2, 'Fresh action'));
    await tester.pumpAndSettle();
    old.complete(state(1, 'Old action'));
    await tester.pumpAndSettle();
    expect(find.text('Fresh action'), findsOneWidget);
    expect(find.text('Old action'), findsNothing);
  });

  testWidgets('late execution response cannot hide a newer input',
      (tester) async {
    final result = Completer<ActionResultDto>();
    await tester.pumpWidget(LauncherApp(
      window: channel,
      beginActivation: () => 1,
      activate: (_) async => state(0, 'Initial action'),
      updateInput: (_, input, revision) async => state(revision, input, input: input),
      execute: (_, revision, candidate) => result.future,
      deactivate: (_) async {},
    ));
    await toggle(tester);
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.enterText(find.byType(TextField), 'New input');
    result.complete(ActionResultDto(
        sessionId: 1,
        revision: 0,
        success: true,
        message: 'Done',
        outputText: null,
        closeWindow: true));
    await tester.pumpAndSettle();
    expect(find.text('New input'), findsWidgets);
    expect(windowCalls, isNot(contains('hide')));
  });

  testWidgets('scope bar switches scopes and stays presentation-only',
      (tester) async {
    final all = candidate('action:core.git.status:c1', 'Git Status', groupId: 'git');
    final folder = candidate('action:core.git.log:c1', 'Git Log', groupId: 'git');
    final appRow = candidate('action:core.directory.copy:c1', 'Copy Path');
    LauncherStateDto scoped(int revision) => LauncherStateDto(
          sessionId: 1,
          revision: revision,
          snapshotSeq: 1,
          input: '',
          contextComplete: true,
          contexts: [],
          candidates: [appRow, all, folder],
          scopes: [
            ScopeDto(
                id: 'app',
                kind: 'app',
                label: 'Finder',
                candidateIds: [appRow.id]),
            ScopeDto(
                id: 'git',
                kind: 'group',
                label: 'Git',
                candidateIds: [all.id, folder.id]),
          ],
          diagnostics: [],
        );
    await tester.pumpWidget(LauncherApp(
      window: channel,
      beginActivation: () => 1,
      activate: (_) async => scoped(0),
      updateInput: (_, input, revision) async => scoped(revision),
      execute: (_, revision, candidate) async =>
          throw StateError('not invoked'),
      deactivate: (_) async {},
    ));
    await toggle(tester);
    expect(find.byKey(const Key('scope-bar')), findsOneWidget);
    expect(find.text('All'), findsNothing);
    expect(find.text('Finder'), findsOneWidget);
    expect(find.text('Git'), findsOneWidget);
    expect(find.text('Copy Path'), findsOneWidget);
    expect(find.text('Git Status'), findsNothing);
    await tester.tap(find.text('Git'));
    await tester.pumpAndSettle();
    expect(find.text('Copy Path'), findsNothing);
    expect(find.text('Git Status'), findsOneWidget);
    expect(find.text('Git Log'), findsOneWidget);
    await tester.sendKeyDownEvent(LogicalKeyboardKey.controlLeft);
    await tester.sendKeyEvent(LogicalKeyboardKey.tab);
    await tester.sendKeyUpEvent(LogicalKeyboardKey.controlLeft);
    await tester.pumpAndSettle();
    expect(find.text('Copy Path'), findsOneWidget);
  });

  testWidgets('manual selection survives late context and falls back when removed',
      (tester) async {
    final first = candidate('action:first:c1', 'First row');
    final second = candidate('action:second:c1', 'Second row');
    final third = candidate('action:third:c1', 'Third row');
    LauncherStateDto lateState({required bool keepSecond}) => LauncherStateDto(
          sessionId: 1,
          revision: 0,
          snapshotSeq: 2,
          input: '',
          contextComplete: true,
          contexts: [],
          candidates: [third, first, if (keepSecond) second],
          scopes: const [],
          diagnostics: [],
        );
    final firstSnapshot = LauncherStateDto(
      sessionId: 1,
      revision: 0,
      snapshotSeq: 1,
      input: '',
      contextComplete: false,
      contexts: [],
      candidates: [first, second],
      scopes: const [],
      diagnostics: [],
    );
    // Each wait gets its own completer: the listener re-arms after every wait, so handing
    // back an already-completed future would spin instead of waiting.
    final pending = <Completer<LauncherStateDto?>>[];
    await tester.pumpWidget(LauncherApp(
      window: channel,
      beginActivation: () => 1,
      activate: (_) async => firstSnapshot,
      updateInput: (_, input, revision) async => state(revision, input, input: input),
      execute: (_, revision, candidate) async =>
          throw StateError('not invoked'),
      deactivate: (_) async {},
      waitForSnapshot: (_, after) {
        final completer = Completer<LauncherStateDto?>();
        pending.add(completer);
        return completer.future;
      },
    ));
    await toggle(tester);
    expect(pending, hasLength(1),
        reason: 'one listener for the visible session');
    // The user moves down before late context arrives.
    await tester.sendKeyEvent(LogicalKeyboardKey.arrowDown);
    await tester.pumpAndSettle();
    expect(find.text('Second row'), findsOneWidget);
    pending[0].complete(lateState(keepSecond: true));
    await tester.pumpAndSettle();
    // The list reordered, but the user's row is still present and selected.
    expect(find.text('Second row'), findsOneWidget);
    expect(windowCalls, isNot(contains('hide')));
  });

  for (final settledAtStart in [false, true]) {
    testWidgets(
        'the index rebuild reaches the window '
        '(contextComplete at start: $settledAtStart)', (tester) async {
      // The core rebuilds the committed snapshot when the application index arrives, and
      // that rebuild is a *newer* snapshot of the same revision. It must reach the list
      // whether or not the context had already finished, and it must do so without a
      // second listener being started.
      LauncherStateDto snapshot(int seq, String title,
              {required bool complete}) =>
          LauncherStateDto(
            sessionId: 1,
            revision: 0,
            snapshotSeq: seq,
            input: 'chrome',
            contextComplete: complete,
            contexts: [],
            candidates: [candidate('action:first:c1', title)],
            scopes: const [],
            diagnostics: [],
          );

      final pending = <Completer<LauncherStateDto?>>[];
      await tester.pumpWidget(LauncherApp(
        window: channel,
        beginActivation: () => 1,
        activate: (_) async =>
            snapshot(1, 'Search the Web', complete: settledAtStart),
        updateInput: (_, input, revision) async =>
            state(revision, 'Search the Web', input: input),
        execute: (_, revision, candidate) async => throw StateError('not invoked'),
        deactivate: (_) async {},
        waitForSnapshot: (_, after) {
          final completer = Completer<LauncherStateDto?>();
          pending.add(completer);
          return completer.future;
        },
      ));
      await toggle(tester);
      expect(find.text('Search the Web'), findsOneWidget);
      expect(pending, hasLength(1), reason: 'one listener per visible session');

      // The index lands: the core republishes the same revision with the new candidate.
      pending[0].complete(snapshot(2, 'Google Chrome', complete: true));
      await tester.pumpAndSettle();
      expect(find.text('Google Chrome'), findsOneWidget,
          reason: 'the rebuilt snapshot must be displayed');
      expect(pending, hasLength(2),
          reason: 'still exactly one listener, re-armed');

      // Nothing new for a while: the listener waits again instead of giving up, so a slow
      // index or late provider result still lands.
      pending[1].complete(null);
      await tester.pumpAndSettle();
      expect(pending, hasLength(3),
          reason: 'an elapsed wait must not end the subscription');
      pending[2].complete(snapshot(3, 'Later result', complete: true));
      await tester.pumpAndSettle();
      expect(find.text('Later result'), findsOneWidget);
      expect(windowCalls, isNot(contains('hide')));
    });
  }

  testWidgets('a selected row stays selectable when late rows push it past Home',
      (tester) async {
    // Home shows seven rows. A later update can rank other rows above the user's selection,
    // pushing it to eighth while it remains a perfectly valid candidate: the view limit
    // must neither hide it nor move the selection, and Enter must still run *that* row.
    CandidateDto row(int index, {double score = 100}) => candidate(
        'action:command.$index:c1', 'Row $index',
        score: score);

    LauncherStateDto withRows(int snapshotSeq, List<CandidateDto> rows) =>
        LauncherStateDto(
          sessionId: 1,
          revision: 0,
          snapshotSeq: snapshotSeq,
          input: '',
          contextComplete: true,
          contexts: [],
          candidates: rows,
          scopes: const [],
          diagnostics: [],
        );

    final initial = [for (var i = 0; i < 8; i++) row(i)];
    // Four rows ranked above every initial row.
    final pushed = [
      for (var i = 0; i < 4; i++) row(100 + i, score: 2000),
      ...initial,
    ];

    final pending = <Completer<LauncherStateDto?>>[];
    var executed = '';
    await tester.pumpWidget(LauncherApp(
      window: channel,
      // Still being enriched, so the window subscribes to later snapshots.
      beginActivation: () => 1,
      activate: (_) async => LauncherStateDto(
        sessionId: 1,
        revision: 0,
        snapshotSeq: 1,
        input: '',
        contextComplete: false,
        contexts: [],
        candidates: initial,
        scopes: const [],
        diagnostics: [],
      ),
      updateInput: (_, input, revision) async => state(revision, input, input: input),
      execute: (_, revision, candidate) async {
        executed = candidate;
        return ActionResultDto(
          sessionId: 1,
          revision: revision,
          success: true,
          message: 'Done',
          closeWindow: false,
        );
      },
      deactivate: (_) async {},
      waitForSnapshot: (_, after) {
        final completer = Completer<LauncherStateDto?>();
        pending.add(completer);
        return completer.future;
      },
    ));
    await toggle(tester);
    expect(pending, hasLength(1), reason: 'one listener per visible session');

    // Settle at eight rows and select the last one Home shows (position 7 of 8).
    pending[0].complete(withRows(2, initial));
    await tester.pumpAndSettle();
    for (var i = 0; i < 6; i++) {
      await tester.sendKeyEvent(LogicalKeyboardKey.arrowDown);
    }
    await tester.pumpAndSettle();
    expect(pending, hasLength(2), reason: 'the listener re-arms for later snapshots');

    // The later update pushes the selection from position 7 to position 11.
    pending[1].complete(withRows(3, pushed));
    await tester.pumpAndSettle();
    expect(find.text('Row 6'), findsOneWidget,
        reason: 'the selected row must stay on screen past the Home limit');
    expect(find.text('Row 100'), findsOneWidget,
        reason: 'the higher-ranked rows are listed too');

    // ...and Enter runs the row the user chose, not the new top row.
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pumpAndSettle();
    expect(executed, 'action:command.6:c1');
    expect(windowCalls, isNot(contains('hide')));
  });

  testWidgets('removed selection falls back to the first row on late update',
      (tester) async {
    final first = candidate('action:first:c1', 'First row');
    final second = candidate('action:second:c1', 'Second row');
    final third = candidate('action:third:c1', 'Third row');
    final pending = <Completer<LauncherStateDto?>>[];
    await tester.pumpWidget(LauncherApp(
      window: channel,
      beginActivation: () => 1,
      activate: (_) async => LauncherStateDto(
        sessionId: 1,
        revision: 0,
        snapshotSeq: 1,
        input: '',
        contextComplete: false,
        contexts: [],
        candidates: [first, second],
        scopes: const [],
        diagnostics: [],
      ),
      updateInput: (_, input, revision) async => state(revision, input, input: input),
      execute: (_, revision, candidate) async =>
          throw StateError('not invoked'),
      deactivate: (_) async {},
      waitForSnapshot: (_, after) {
        final completer = Completer<LauncherStateDto?>();
        pending.add(completer);
        return completer.future;
      },
    ));
    await toggle(tester);
    await tester.sendKeyEvent(LogicalKeyboardKey.arrowDown);
    await tester.pumpAndSettle();
    pending[0].complete(LauncherStateDto(
      sessionId: 1,
      revision: 0,
      snapshotSeq: 2,
      input: '',
      contextComplete: true,
      contexts: [],
      candidates: [third],
      scopes: const [],
      diagnostics: [],
    ));
    await tester.pumpAndSettle();
    expect(find.text('Third row'), findsOneWidget);
    expect(find.text('Second row'), findsNothing);
  });

  testWidgets('application icons render when loaded and fall back when absent',
      (tester) async {
    final chrome = CandidateDto(
      id: 'app:com.google.Chrome:/Applications/Google Chrome.app',
      kind: 'application',
      title: 'Google Chrome',
      subtitle: '/Applications/Google Chrome.app',
      iconKey: 'application',
      score: 5000,
      groupId: 'app',
      availability: 'enabled',
      availabilityReason: null,
      commandId: null,
      bundleId: 'com.google.Chrome',
    );
    final notes = CandidateDto(
      id: 'app:com.apple.Notes:/System/Applications/Notes.app',
      kind: 'application',
      title: 'Notes',
      subtitle: '/System/Applications/Notes.app',
      iconKey: 'application',
      score: 4000,
      groupId: 'app',
      availability: 'enabled',
      availabilityReason: null,
      commandId: null,
      bundleId: 'com.apple.Notes',
    );
    final state = LauncherStateDto(
      sessionId: 1,
      revision: 0,
      snapshotSeq: 1,
      input: 'app',
      contextComplete: true,
      contexts: [],
      candidates: [chrome, notes],
      scopes: const [],
      diagnostics: [],
    );
    final requested = <String>[];
    await tester.pumpWidget(LauncherApp(
      window: channel,
      beginActivation: () => 1,
      activate: (_) async => state,
      updateInput: (_, input, revision) async => state,
      execute: (_, revision, candidate) async =>
          throw StateError('not invoked'),
      deactivate: (_) async {},
      // Chrome loads a real 1x1 PNG; Notes fails, so the glyph must remain.
      loadIcon: (candidateId) async {
        requested.add(candidateId);
        if (candidateId.startsWith('app:com.google.Chrome')) {
          return Uint8List.fromList(_onePixelPng);
        }
        throw StateError('no icon');
      },
    ));
    await toggle(tester);
    await tester.pumpAndSettle();
    expect(requested, contains(chrome.id));
    expect(requested, contains(notes.id));
    expect(find.byType(Image), findsOneWidget);
    // The failing icon falls back to a glyph instead of breaking the row.
    expect(find.byIcon(Icons.apps_rounded), findsOneWidget);
    expect(find.text('Notes'), findsOneWidget);
  });

  testWidgets('an icon that fails once is retried on the next snapshot',
      (tester) async {
    CandidateDto app(String id, String title) => CandidateDto(
          id: id,
          kind: 'application',
          title: title,
          subtitle: '/Applications/$title.app',
          iconKey: 'application',
          score: 5000,
          groupId: 'app',
          availability: 'enabled',
          availabilityReason: null,
          commandId: null,
          bundleId: id,
        );
    final edge = app('app:com.microsoft.edgemac', 'Microsoft Edge');
    // Sequence must advance, or the ordering guard correctly ignores the second
    // snapshot and this test would not exercise a retry at all.
    LauncherStateDto withEdge(int revision) => LauncherStateDto(
          sessionId: 1,
          revision: revision,
          snapshotSeq: revision + 1,
          input: 'edge',
          contextComplete: true,
          contexts: [],
          candidates: [edge],
          scopes: const [],
          diagnostics: [],
        );
    var attempts = 0;
    await tester.pumpWidget(LauncherApp(
      window: channel,
      beginActivation: () => 1,
      activate: (_) async => withEdge(0),
      updateInput: (_, input, revision) async => withEdge(revision),
      execute: (_, revision, candidate) async =>
          throw StateError('not invoked'),
      deactivate: (_) async {},
      loadIcon: (candidateId) async {
        attempts++;
        // First attempt fails (as it would while the bridge is still starting).
        if (attempts == 1) throw StateError('not ready yet');
        return Uint8List.fromList(_onePixelPng);
      },
    ));
    await toggle(tester);
    await tester.pumpAndSettle();
    expect(attempts, 1);
    expect(find.byType(Image), findsNothing, reason: 'the first attempt failed');
    // A later snapshot must try again rather than cache the absence forever.
    await tester.enterText(find.byType(TextField), 'edge again');
    await tester.pumpAndSettle();
    expect(attempts, greaterThan(1));
    expect(find.byType(Image), findsOneWidget);
  });

  testWidgets('disabled candidates show their reason and cannot execute',
      (tester) async {
    var executed = 0;
    await tester.pumpWidget(LauncherApp(
      window: channel,
      beginActivation: () => 1,
      activate: (_) async => LauncherStateDto(
        sessionId: 1,
        revision: 0,
        snapshotSeq: 1,
        input: '',
        contextComplete: true,
        contexts: [],
        candidates: [
          candidate('action:core.git.status:c1', 'Git Status',
              availability: 'disabled',
              reason: 'git is not installed',
              iconKey: 'git'),
        ],
        scopes: const [],
        diagnostics: [],
      ),
      updateInput: (_, input, revision) async => state(revision, input, input: input),
      execute: (_, revision, candidate) async {
        executed++;
        throw StateError('must not execute');
      },
      deactivate: (_) async {},
    ));
    await toggle(tester);
    expect(find.text('git is not installed'), findsOneWidget);
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pumpAndSettle();
    expect(executed, 0);
  });

  testWidgets('typing keeps the active scope', (tester) async {
    final copy = candidate('action:core.directory.copy:c1', 'Copy Path');
    final git = candidate('action:core.git.status:c1', 'Git Status', groupId: 'git');
    LauncherStateDto scoped(int revision, String input) => LauncherStateDto(
          sessionId: 1,
          revision: revision,
          snapshotSeq: 1,
          input: input,
          contextComplete: true,
          contexts: [],
          candidates: [copy, git],
          scopes: [
            ScopeDto(
                id: 'app',
                kind: 'app',
                label: 'Finder',
                candidateIds: [copy.id]),
            ScopeDto(
                id: 'git',
                kind: 'group',
                label: 'Git',
                candidateIds: [git.id]),
          ],
          diagnostics: [],
        );
    await tester.pumpWidget(LauncherApp(
      window: channel,
      beginActivation: () => 1,
      activate: (_) async => scoped(0, ''),
      updateInput: (_, input, revision) async => scoped(revision, input),
      execute: (_, __, ___) async => throw StateError('not invoked'),
      deactivate: (_) async {},
    ));
    await toggle(tester);
    expect(find.text('Copy Path'), findsOneWidget);
    expect(find.text('Git Status'), findsNothing);
    await tester.enterText(find.byType(TextField), 'copy');
    await tester.pumpAndSettle();
    expect(find.text('Copy Path'), findsOneWidget);
    expect(find.text('Git Status'), findsNothing);
  });

  testWidgets('Esc clears query then scope then the window', (tester) async {
    final copy = candidate('action:core.directory.copy:c1', 'Copy Path');
    final git = candidate('action:core.git.status:c1', 'Git Status', groupId: 'git');
    LauncherStateDto scoped(int revision, String input) => LauncherStateDto(
          sessionId: 1,
          revision: revision,
          snapshotSeq: 1,
          input: input,
          contextComplete: true,
          contexts: [],
          candidates: [copy, git],
          scopes: [
            ScopeDto(
                id: 'app',
                kind: 'app',
                label: 'Finder',
                candidateIds: [copy.id]),
            ScopeDto(
                id: 'git',
                kind: 'group',
                label: 'Git',
                candidateIds: [git.id]),
          ],
          diagnostics: [],
        );
    var deactivated = 0;
    await tester.pumpWidget(LauncherApp(
      window: channel,
      beginActivation: () => 1,
      activate: (_) async => scoped(0, ''),
      updateInput: (_, input, revision) async => scoped(revision, input),
      execute: (_, __, ___) async => throw StateError('not invoked'),
      deactivate: (_) async {
        deactivated++;
      },
    ));
    await toggle(tester);
    await tester.enterText(find.byType(TextField), 'copy');
    await tester.pumpAndSettle();
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pumpAndSettle();
    expect(tester.widget<TextField>(find.byType(TextField)).controller!.text, isEmpty);
    expect(find.text('Copy Path'), findsOneWidget);
    expect(find.text('Git Status'), findsNothing);
    expect(deactivated, 0);
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pumpAndSettle();
    expect(find.text('Copy Path'), findsOneWidget);
    expect(find.text('Git Status'), findsOneWidget);
    expect(deactivated, 0);
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pumpAndSettle();
    expect(deactivated, 1);
    expect(windowCalls.last, 'hide');
  });

  testWidgets('another wake entry switches mode in place and keeps the query',
      (tester) async {
    final copy = candidate('action:core.directory.copy:c1', 'Copy Path');
    final git = candidate('action:core.git.status:c1', 'Git Status', groupId: 'git');
    LauncherStateDto scoped(int revision, String input) => LauncherStateDto(
          sessionId: 1,
          revision: revision,
          snapshotSeq: 1,
          input: input,
          contextComplete: true,
          contexts: [],
          candidates: [copy, git],
          scopes: [
            ScopeDto(
                id: 'app',
                kind: 'app',
                label: 'Finder',
                candidateIds: [copy.id]),
            ScopeDto(
                id: 'git',
                kind: 'group',
                label: 'Git',
                candidateIds: [git.id]),
          ],
          diagnostics: [],
        );
    await tester.pumpWidget(LauncherApp(
      window: channel,
      beginActivation: () => 1,
      activate: (_) async => scoped(0, ''),
      updateInput: (_, input, revision) async => scoped(revision, input),
      execute: (_, __, ___) async => throw StateError('not invoked'),
      deactivate: (_) async {},
    ));
    await toggle(tester);
    await tester.enterText(find.byType(TextField), 'copy');
    await tester.pumpAndSettle();
    await toggle(tester, mode: 'universal');
    expect(tester.widget<TextField>(find.byType(TextField)).controller!.text, 'copy');
    expect(find.text('Copy Path'), findsOneWidget);
    expect(find.text('Git Status'), findsOneWidget);
    expect(windowCalls, isNot(contains('hide')));
    await toggle(tester, mode: 'universal');
    await tester.pumpAndSettle();
    expect(windowCalls.last, 'hide');
  });

  testWidgets('Current App late snapshot is selected while idle', (tester) async {
    final git = candidate('action:core.git.status:c1', 'Git Status', groupId: 'git');
    final copy = candidate('action:core.directory.copy:c1', 'Copy Path');
    final first = LauncherStateDto(
      sessionId: 1,
      revision: 0,
      snapshotSeq: 1,
      input: '',
      contextComplete: false,
      contexts: [],
      candidates: [git],
      scopes: [
        ScopeDto(
            id: 'git',
            kind: 'group',
            label: 'Git',
            candidateIds: [git.id]),
      ],
      diagnostics: [],
    );
    final late = LauncherStateDto(
      sessionId: 1,
      revision: 0,
      snapshotSeq: 2,
      input: '',
      contextComplete: true,
      contexts: [],
      candidates: [copy, git],
      scopes: [
        ScopeDto(
            id: 'app',
            kind: 'app',
            label: 'Finder',
            candidateIds: [copy.id]),
        ScopeDto(
            id: 'git',
            kind: 'group',
            label: 'Git',
            candidateIds: [git.id]),
      ],
      diagnostics: [],
    );
    final pending = <Completer<LauncherStateDto?>>[];
    await tester.pumpWidget(LauncherApp(
      window: channel,
      beginActivation: () => 1,
      activate: (_) async => first,
      updateInput: (_, input, revision) async => state(revision, input, input: input),
      execute: (_, __, ___) async => throw StateError('not invoked'),
      deactivate: (_) async {},
      waitForSnapshot: (_, __) {
        final completer = Completer<LauncherStateDto?>();
        pending.add(completer);
        return completer.future;
      },
    ));
    await toggle(tester);
    expect(find.text('Git Status'), findsOneWidget);
    expect(find.text('Copy Path'), findsNothing);
    pending[0].complete(late);
    await tester.pumpAndSettle();
    expect(find.text('Copy Path'), findsOneWidget);
    expect(find.text('Git Status'), findsNothing);
  });
}
