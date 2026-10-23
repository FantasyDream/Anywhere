import 'dart:async';

import 'package:anywhere/infrastructure/desktop_environment.dart';
import 'package:anywhere/infrastructure/window_host.dart';
import 'package:anywhere/launcher/file_search_controller.dart';
import 'package:anywhere/launcher/launcher_controller.dart';
import 'package:anywhere/launcher/launcher_screen.dart';
import 'package:anywhere/launcher/widgets/launcher_components.dart';
import 'package:anywhere/settings/action_shortcut.dart';
import 'package:anywhere/settings/host_settings.dart';
import 'package:anywhere/src/rust/api.dart';
import 'package:anywhere/ui/app_components.dart';
import 'package:anywhere/ui/app_icons.dart';
import 'package:anywhere/ui/app_style.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

import 'launcher_fixtures.dart';
import 'support/launcher_ui_capture.dart';

CandidateDto row(String id, String title, String command,
        {bool file = false}) =>
    CandidateDto(
        id: id,
        title: title,
        commandId: command,
        kind: file ? CandidateKindDto.file : CandidateKindDto.action,
        fileKind: file
            ? (command == 'core.directory.open'
                ? FileObjectKindDto.directory
                : FileObjectKindDto.file)
            : null,
        iconKey: command == 'core.files.search'
            ? 'search'
            : command == 'core.directory.open'
                ? 'folder'
                : 'file',
        groupId: file ? 'files' : 'tools',
        score: 1,
        availability: AvailabilityDto.enabled,
        menuPath: [],
        contextKeys: [],
        executionContextId: file ? id : null,
        homeEligible: !file && command != 'core.files.search',
        primaryAction: file,
        subtitle: command == 'core.files.search' ? '' : '/project');
final entry = row('search-entry', 'File Search', 'core.files.search');
final text = row('text-file', 'report.txt', 'core.file.open', file: true);
final folder =
    row('folder-result', 'report-folder', 'core.directory.open', file: true);
FilePreviewDto preview(String path, {bool directory = false}) => FilePreviewDto(
    path: path,
    isDirectory: directory,
    sizeBytes: BigInt.from(12),
    text: directory ? null : 'Preview content');

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  const channel = MethodChannel('test/file-workspace');
  final messenger =
      TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger;
  setUpAll(loadCaptureFonts);
  setUp(() {
    DesktopEnvironment.configure(
        const DesktopEnvironment(platform: TargetPlatform.macOS));
    messenger.setMockMethodCallHandler(channel, (_) async => null);
  });
  tearDown(() {
    DesktopEnvironment.reset();
    messenger.setMockMethodCallHandler(channel, null);
  });

  Future<LauncherController> mount(WidgetTester tester,
      {ActionShortcut? shortcut,
      List<String>? executed,
      List<(String, int, InputModeDto)>? queries,
      List<CandidateDto>? hits,
      Map<String, List<CandidateDto>>? filteredHits,
      Completer<void>? executionGate,
      Completer<void>? queryGate,
      bool failFirst = false,
      bool environmentScope = false,
      FileSearchProgressDto? progress,
      Future<LauncherStateDto?> Function(int, int)? more,
      List<(String, String)>? fileQueries,
      List<(int, int)>? frozen,
      Future<SnapshotWaitDto> Function(int, int, int, int)? waitDelta,
      Future<SnapshotWaitDto> Function(int, int, int, int)? moreDelta,
      List<String> diagnostics = const [],
      double textScale = 1,
      LoadFilePreview? load}) async {
    await tester.binding.setSurfaceSize(const Size(1100, 760));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    if (shortcut != null) {
      messenger.setMockMethodCallHandler(
          channel,
          (call) async => call.method == 'settings'
              ? HostSettings.defaults
                  .copyWith(actionShortcuts: [shortcut]).toMap()
              : null);
    }
    var attempt = 0;
    LauncherStateDto state(int revision, String input, InputModeDto mode,
            {String filter = 'all'}) =>
        launcherSnapshot(
            sessionId: 1,
            revision: revision,
            snapshotSeq: 1,
            candidateListVersion: mode == InputModeDto.fileSearch ? 1 : 0,
            input: input,
            contextComplete: true,
            contextCandidates: environmentScope &&
                    mode == InputModeDto.environment
                ? [row('ambient-path', 'Copy Path', 'core.directory.copy_path')]
                : [],
            scopes: environmentScope && mode == InputModeDto.environment
                ? const [
                    ScopeDto(
                        id: 'app.editor', label: 'Editor', isCurrentApp: true),
                    ScopeDto(id: 'folder', label: 'Folder', isCurrentApp: false)
                  ]
                : [],
            candidates: mode != InputModeDto.fileSearch
                ? input == 'unmatched'
                    ? []
                    : [entry]
                : input.length < 2 || (failFirst && attempt == 1)
                    ? []
                    : (filteredHits?[filter] ??
                        (hits ?? [text, folder])
                            .where((row) =>
                                filter == 'all' ||
                                (filter == 'files' &&
                                    row.fileKind == FileObjectKindDto.file) ||
                                (filter == 'folders' &&
                                    row.fileKind ==
                                        FileObjectKindDto.directory))
                            .toList()),
            resultActions: mode != InputModeDto.fileSearch || input.length < 2
                ? []
                : [
                    CandidateDto(
                        id: 'copy-text',
                        title: 'Copy Path',
                        commandId: 'core.file.copy',
                        kind: CandidateKindDto.action,
                        fileKind: FileObjectKindDto.file,
                        iconKey: 'copy',
                        groupId: 'files',
                        score: 1,
                        availability: AvailabilityDto.enabled,
                        menuPath: [],
                        contextKeys: [],
                        executionContextId: 'text-file',
                        homeEligible: false,
                        primaryAction: false)
                  ],
            searchSources: mode != InputModeDto.fileSearch || input.length < 2
                ? []
                : [
                    SearchSourceDto(
                        id: 'files',
                        state: failFirst && attempt == 1
                            ? SearchSourceStateDto.unavailable
                            : (progress?.issueCount ?? BigInt.zero) >
                                    BigInt.zero
                                ? SearchSourceStateDto.partial
                                : SearchSourceStateDto.ready)
                  ],
            fileSearch: mode == InputModeDto.fileSearch ? progress : null,
            diagnostics: []);
    final controller = LauncherController(
        backend: LauncherBackend(
            waitForSnapshotDelta: waitDelta,
            fileMoreDelta: moreDelta,
            fileMore: more,
            freezeFiles: frozen == null
                ? null
                : (session, revision) async {
                    frozen.add((session, revision));
                  },
            updateFiles: (session, input, revision, filter) async {
              fileQueries?.add((input, filter));
              queries?.add((input, revision, InputModeDto.fileSearch));
              if (input.length >= 2) {
                attempt++;
              }
              return state(revision, input, InputModeDto.fileSearch,
                  filter: filter);
            },
            beginActivation: () => 1,
            activate: (_,
                    {promptAccessibility = false,
                    capturedEnvironment = false}) async =>
                state(0, '', InputModeDto.search),
            updateInput: (_, input, revision,
                {scopeId, inputMode = InputModeDto.environment}) async {
              queries?.add((input, revision, inputMode));
              if (input == 'unmatched') await queryGate?.future;
              if (inputMode == InputModeDto.fileSearch && input.length >= 2) {
                attempt++;
              }
              return state(revision, input, inputMode);
            },
            resolveBinding: (_, __, action) async =>
                action == entry.commandId ? [entry] : [],
            shortcutActions: () async => const [
                  ShortcutActionDto(
                      actionId: 'core.files.search',
                      title: 'File Search',
                      contextTypes: ['core.launcher'])
                ],
            loadFilePreview: load ??
                (_, __, id) async => preview(
                    id == folder.id
                        ? '/project/report-folder'
                        : '/project/report.txt',
                    directory: id == folder.id),
            execute: (_, revision, id) async {
              executed?.add(id);
              if (id == entry.id) await executionGate?.future;
              return ActionResultDto(
                  sessionId: 1,
                  revision: revision,
                  status: ActionStatusDto.succeeded,
                  closeWindow: id != entry.id,
                  view: id == entry.id ? HostViewDto.fileSearch : null);
            },
            deactivate: (_) async {}),
        window: WindowHost(channel),
        applyFileSearch: (_) async {});
    addTearDown(controller.dispose);
    await tester.pumpWidget(MaterialApp(
        theme: AppStyle.theme,
        builder: (context, child) => AppStyle.windowBuilder(
            context,
            captureRoot(MediaQuery(
                data: MediaQuery.of(context)
                    .copyWith(textScaler: TextScaler.linear(textScale)),
                child: child!))),
        home: LauncherScreen(controller: controller)));
    await controller.settings.load();
    await controller.toggle('universal');
    await tester.pumpAndSettle();
    return controller;
  }

  Future<void> openFiles(WidgetTester tester) async {
    tester.testTextInput.updateEditingValue(const TextEditingValue(
        text: 'files', selection: TextSelection.collapsed(offset: 5)));
    await tester.pumpAndSettle();
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pumpAndSettle();
  }

  Future<void> filterFiles(WidgetTester tester, FileFilter filter) async {
    await tester.ensureVisible(find.byKey(Key('file-filter-${filter.name}')));
    await tester.tap(find.byKey(Key('file-filter-${filter.name}')));
    await tester.pumpAndSettle();
  }

  Future<void> chord(
      WidgetTester tester, LogicalKeyboardKey modifier, LogicalKeyboardKey key,
      {bool shift = false}) async {
    await tester.sendKeyDownEvent(modifier);
    if (shift) await tester.sendKeyDownEvent(LogicalKeyboardKey.shiftLeft);
    await tester.sendKeyEvent(key);
    if (shift) await tester.sendKeyUpEvent(LogicalKeyboardKey.shiftLeft);
    await tester.sendKeyUpEvent(modifier);
    await tester.pumpAndSettle();
  }

  Finder fileRow(String title) => find.descendant(
      of: find.byKey(const Key('action-list')), matching: find.text(title));

  Future<void> openPreview(WidgetTester tester, {bool capture = false}) async {
    expect(find.byKey(const Key('file-preview-pane')), findsOneWidget);
    expect(find.byKey(const Key('file-preview-close')), findsNothing);
    if (capture) {
      await chord(tester, LogicalKeyboardKey.metaLeft, LogicalKeyboardKey.keyK);
      expect(find.byKey(const Key('file-preview-action')), findsNothing);
      await saveLauncherUi(tester, 'files-actions');
      await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    }
    await tester.pumpAndSettle();
  }

  FileSearchProgressDto searchProgress(
          {int page = 0,
          int inaccessible = 0,
          int issues = 0,
          List<String> examples = const [],
          String? next,
          String? previous,
          bool refresh = false}) =>
      FileSearchProgressDto(
          queryId: BigInt.one,
          roots: const ['/project'],
          scannedEntries: BigInt.from(5),
          confirmedMatches: BigInt.from(5),
          finished: true,
          issueCount: BigInt.from(issues),
          inaccessibleCount: BigInt.from(inaccessible),
          issueExamples: examples,
          page: page,
          nextCursor: next,
          previousCursor: previous,
          needsRefresh: refresh);

  testWidgets(
      'incomplete search keeps results and exposes the actual diagnostic',
      (tester) async {
    const example = 'Non-Unicode filename: /project/unsupported-name';
    await mount(tester,
        progress: searchProgress(issues: 1, examples: const [example]));
    await openFiles(tester);
    await tester.enterText(find.byType(TextField), 'report');
    await tester.pumpAndSettle();
    expect(
        find.text(
            'File search encountered 1 issue. Available results are still usable.'),
        findsOneWidget);
    expect(find.textContaining('Partial results · 1 issue'), findsOneWidget);
    expect(find.byTooltip(example), findsOneWidget);
    expect(fileRow('report.txt'), findsOneWidget);
    expect(find.text('Retry'), findsOneWidget);
    expect(find.textContaining('Some locations could not'), findsNothing);
    expect(tester.takeException(), isNull);
  });

  testWidgets(
      'missing issue examples do not suppress the incomplete-search notice',
      (tester) async {
    await mount(tester, progress: searchProgress(issues: 2));
    await openFiles(tester);
    await tester.enterText(find.byType(TextField), 'report');
    await tester.pumpAndSettle();
    const notice =
        'File search encountered 2 issues. Available results are still usable.';
    expect(find.text(notice), findsOneWidget);
    expect(find.byTooltip(notice), findsOneWidget);
    expect(fileRow('report.txt'), findsOneWidget);
    expect(tester.takeException(), isNull);
  });

  testWidgets(
      'restricted locations are skipped without a warning or permission retry',
      (tester) async {
    final executed = <String>[];
    await mount(tester,
        executed: executed, progress: searchProgress(inaccessible: 438));
    await openFiles(tester);
    await tester.enterText(find.byType(TextField), 'report');
    await tester.pumpAndSettle();
    expect(find.textContaining('438 restricted locations skipped'),
        findsOneWidget);
    expect(find.text('Retry'), findsNothing);
    expect(find.textContaining('Operation not permitted'), findsNothing);
    expect(fileRow('report.txt'), findsOneWidget);
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pumpAndSettle();
    expect(executed, [entry.id, text.id]);
    expect(tester.takeException(), isNull);
  });

  testWidgets(
      'keyboard navigation appends results and returns to earlier files without paging',
      (tester) async {
    final queries = <(String, String)>[],
        frozen = <(int, int)>[],
        cursors = <String?>[];
    final second =
        row('second', 'report-two.pdf', 'core.file.open', file: true);
    final c = await mount(tester,
        fileQueries: queries,
        frozen: frozen,
        progress: searchProgress(next: 'next'),
        more: (session, revision) async {
      cursors.add('more');
      return launcherSnapshot(
          sessionId: session,
          revision: revision,
          snapshotSeq: 10 + cursors.length,
          input: 'report',
          contextComplete: true,
          candidates: [text, folder, second],
          contextCandidates: [],
          diagnostics: [],
          searchSources: const [
            SearchSourceDto(id: 'files', state: SearchSourceStateDto.ready)
          ],
          fileSearch: searchProgress());
    });
    await openFiles(tester);
    await c.update('report');
    await tester.pumpAndSettle();
    expect(queries.last, ('report', 'all'));
    expect(find.text('5 found · Search complete'), findsOneWidget);
    await chord(tester, LogicalKeyboardKey.controlLeft, LogicalKeyboardKey.tab);
    expect(queries.last, ('report', 'files'));
    // Production rows are already filtered by Rust; no second lossy UI filter.
    expect(c.view.visibleEntries.length, 1);
    await chord(tester, LogicalKeyboardKey.controlLeft, LogicalKeyboardKey.tab,
        shift: true);
    expect(c.view.visibleEntries.length, 2);
    await tester.sendKeyEvent(LogicalKeyboardKey.arrowDown);
    await tester.sendKeyEvent(LogicalKeyboardKey.arrowDown);
    await tester.pumpAndSettle();
    expect(cursors, ['more']);
    expect(
        c.view.visibleEntries[c.view.selectedIndex].candidate?.id, second.id);
    expect(find.text('5 found · Search complete'), findsOneWidget);
    expect(find.byKey(const Key('file-next-page')), findsNothing);
    expect(find.byKey(const Key('file-previous-page')), findsNothing);
    expect(c.view.visibleCandidates.length, 3);
    await tester.sendKeyEvent(LogicalKeyboardKey.arrowUp);
    await tester.pumpAndSettle();
    expect(cursors, ['more']);
    expect(
        c.view.visibleEntries[c.view.selectedIndex].candidate?.id, folder.id);
    expect(frozen, isNotEmpty);
    expect(tester.takeException(), isNull);
  });

  for (final subscriptionFirst in [true, false]) {
    testWidgets(
        'delta load and subscription race: subscription first=$subscriptionFirst',
        (tester) async {
      final pending = <(int, int, int, int, Completer<SnapshotWaitDto>)>[];
      final queries = <(String, int, InputModeDto)>[];
      final moreReply = Completer<SnapshotWaitDto>();
      final c = await mount(tester,
          queries: queries, progress: searchProgress(next: 'next'),
          waitDelta: (session, cursor, version, count) {
        final reply = Completer<SnapshotWaitDto>();
        pending.add((session, cursor, version, count, reply));
        return reply.future;
      }, moreDelta: (session, revision, version, count) {
        expect(version, 1);
        expect(count, 2);
        return moreReply.future;
      });
      await openFiles(tester);
      await tester.enterText(find.byType(TextField), 'report');
      await tester.pumpAndSettle();
      final revision = queries.last.$2;
      final second =
          row('second', 'report-two.pdf', 'core.file.open', file: true);
      SnapshotWaitDto packet(int seq, List<CandidateDto> rows,
              {CandidateUpdateDto update = CandidateUpdateDto.append,
              int version = 1}) =>
          SnapshotWaitDto(
              kind: SnapshotWaitKindDto.updated,
              candidateUpdate: update,
              state: launcherSnapshot(
                  sessionId: 1,
                  revision: revision,
                  snapshotSeq: seq,
                  candidateListVersion: version,
                  input: 'report',
                  contextComplete: true,
                  candidates: rows,
                  diagnostics: [],
                  fileSearch: searchProgress()));
      // Unblock the environment request with a complete current replacement.
      pending.last.$5.complete(
          packet(1, [text, folder], update: CandidateUpdateDto.replace));
      await tester.pumpAndSettle();
      expect(pending.last.$4, 2);
      final subscribed = pending.last.$5;
      final more = c.loadMoreFiles(advanceSelection: true);
      await tester.pump();
      if (subscriptionFirst) {
        subscribed.complete(packet(2, [second]));
        await tester.pumpAndSettle();
        moreReply.complete(packet(2, [second]));
      } else {
        moreReply.complete(packet(2, [second]));
        await more;
        subscribed.complete(packet(3, [second]));
      }
      await more;
      await tester.pumpAndSettle();
      expect(c.view.visibleCandidates.map((r) => r.id),
          [text.id, folder.id, second.id]);
      expect(
          c.view.visibleEntries[c.view.selectedIndex].candidate!.id, second.id);
      expect(
          pending.last.$4, 3); // Retry uses the list actually held by the UI.
      pending.last.$5
          .complete(packet(4, [], update: CandidateUpdateDto.retain));
      await tester.pumpAndSettle();
      expect(c.view.visibleCandidates.length, 3);
      // A changed/reordered list must replace the held array, never append it.
      pending.last.$5.complete(packet(5, [folder, text],
          update: CandidateUpdateDto.replace, version: 2));
      await tester.pumpAndSettle();
      expect(c.view.visibleCandidates.map((r) => r.id), [folder.id, text.id]);
      expect(pending.last.$3, 2);
      expect(tester.testTextInput.hasAnyClients, isTrue);
      await c.dismiss(retain: false);
      pending.last.$5.complete(
          const SnapshotWaitDto(kind: SnapshotWaitKindDto.superseded));
      await tester.pumpWidget(const SizedBox());
      await tester.pump();
      expect(tester.takeException(), isNull);
    });
  }

  testWidgets('an old load-more response cannot overwrite a newly typed query',
      (tester) async {
    final reply = Completer<LauncherStateDto?>();
    final c = await mount(tester,
        fileQueries: [],
        progress: searchProgress(next: 'next'),
        more: (_, __) => reply.future);
    await openFiles(tester);
    await c.update('report');
    await tester.pumpAndSettle();
    final revision = c.view.snapshot!.revision;
    final pending = c.loadMoreFiles();
    await tester.pump();
    await c.update('new query');
    reply.complete(launcherSnapshot(
        sessionId: 1,
        revision: revision,
        snapshotSeq: 100,
        input: 'report',
        contextComplete: true,
        candidates: [row('stale', 'stale.pdf', 'core.file.open', file: true)],
        diagnostics: [],
        fileSearch: searchProgress(page: 1)));
    await pending;
    await tester.pumpAndSettle();
    expect(c.view.input, 'new query');
    expect(c.view.visibleCandidates.any((file) => file.id == 'stale'), false);
    expect(c.fileMoreLoading, false);
  });

  testWidgets(
      'scrolling appends results without resetting selection or scroll position',
      (tester) async {
    final cursors = <String?>[];
    late LauncherController c;
    String? selectedAtLoad;
    c = await mount(tester,
        fileQueries: [],
        hits: List.generate(
            64,
            (i) =>
                row('file-$i', 'report-$i.txt', 'core.file.open', file: true)),
        progress: searchProgress(next: 'next'),
        more: (session, revision) async {
      selectedAtLoad = c.view.visibleCandidates[c.view.selectedIndex].id;
      cursors.add('more');
      return launcherSnapshot(
          sessionId: session,
          revision: revision,
          snapshotSeq: 100,
          input: 'report',
          contextComplete: true,
          candidates: [
            ...List.generate(
                64,
                (i) => row('file-$i', 'report-$i.txt', 'core.file.open',
                    file: true)),
            row('last-page', 'report-last.txt', 'core.file.open', file: true)
          ],
          diagnostics: [],
          fileSearch: searchProgress(page: 1, previous: 'previous'));
    });
    await openFiles(tester);
    await c.update('report');
    await tester.pumpAndSettle();
    await tester.drag(
        find.byKey(const Key('action-list')), const Offset(0, -5000));
    await tester.pumpAndSettle();
    expect(cursors, ['more']);
    expect(c.view.visibleCandidates.length, 65);
    expect(c.view.visibleCandidates.last.id, 'last-page');
    expect(c.view.visibleCandidates[c.view.selectedIndex].id, selectedAtLoad);
    final list = tester.widget<ListView>(find.byKey(const Key('action-list')));
    expect(list.controller!.offset, greaterThan(2000));
    await tester.sendKeyEvent(LogicalKeyboardKey.arrowDown);
    await tester.pumpAndSettle();
    expect(c.view.visibleCandidates[c.view.selectedIndex].id,
        'file-${int.parse(selectedAtLoad!.split('-').last) + 1}');
    expect(list.controller!.offset, lessThan(300));
    expect(tester.takeException(), isNull);
  });

  testWidgets(
      'load failure retains rows and selection and the next arrow retries',
      (tester) async {
    var calls = 0;
    final third =
        row('third', 'report-three.pdf', 'core.file.open', file: true);
    final c = await mount(tester,
        fileQueries: [], progress: searchProgress(next: 'more'),
        more: (session, revision) async {
      if (++calls == 1) throw StateError('temporary read failure');
      return launcherSnapshot(
          sessionId: session,
          revision: revision,
          diagnostics: [],
          snapshotSeq: 20,
          input: 'report',
          contextComplete: true,
          candidates: [text, folder, third],
          fileSearch: searchProgress());
    });
    await openFiles(tester);
    await c.update('report');
    await tester.pumpAndSettle();
    c.move(1);
    c.move(1);
    await tester.pumpAndSettle();
    expect(calls, 1);
    expect(c.view.visibleCandidates.map((f) => f.id), [text.id, folder.id]);
    expect(
        c.view.visibleEntries[c.view.selectedIndex].candidate?.id, folder.id);
    expect(c.fileMoreLoading, false);
    expect(find.text('Could not load more files. Scroll or press ↓ to retry.'),
        findsOneWidget);
    await tester.sendKeyEvent(LogicalKeyboardKey.arrowDown);
    await tester.pumpAndSettle();
    expect(calls, 2);
    expect(c.view.visibleEntries[c.view.selectedIndex].candidate?.id, third.id);
    await tester.sendKeyEvent(LogicalKeyboardKey.arrowDown);
    await tester.pumpAndSettle();
    expect(c.view.visibleEntries[c.view.selectedIndex].candidate?.id, third.id);
    expect(tester.testTextInput.hasAnyClients, true);
    expect(tester.takeException(), isNull);
  });

  testWidgets(
      'repeated boundary arrows share a load and do not override a new selection',
      (tester) async {
    final reply = Completer<LauncherStateDto?>();
    var calls = 0;
    final c = await mount(tester,
        fileQueries: [], progress: searchProgress(next: 'more'), more: (_, __) {
      calls++;
      return reply.future;
    });
    await openFiles(tester);
    await c.update('report');
    await tester.pumpAndSettle();
    final revision = c.view.snapshot!.revision;
    c.move(1);
    c.move(1);
    c.move(1);
    expect(calls, 1);
    c.move(-1);
    reply.complete(launcherSnapshot(
        sessionId: 1,
        revision: revision,
        diagnostics: [],
        snapshotSeq: 10,
        input: 'report',
        contextComplete: true,
        candidates: [
          text,
          folder,
          row('third', 'report-three.pdf', 'core.file.open', file: true)
        ],
        fileSearch: searchProgress()));
    await tester.pumpAndSettle();
    expect(c.view.visibleEntries[c.view.selectedIndex].candidate?.id, text.id);
    expect(c.fileMoreLoading, false);
    expect(c.view.visibleCandidates.length, 3);
    expect(tester.takeException(), isNull);
  });

  testWidgets('ambient menu errors do not leak into a completed file query',
      (tester) async {
    final c = await mount(tester, diagnostics: const [
      'Menu discovery timed out; Current App menus were not updated.'
    ]);
    await openFiles(tester);
    await c.update('report');
    await tester.pumpAndSettle();
    expect(c.view.isFileSearch, true);
    expect(c.view.notice, isNull);
    expect(find.textContaining('Menu discovery timed out'), findsNothing);
    expect(fileRow('report.txt'), findsOneWidget);
    expect(tester.takeException(), isNull);
  });

  testWidgets('a pending query does not retain the previous matching tool',
      (tester) async {
    final gate = Completer<void>();
    final c = await mount(tester, queryGate: gate);
    await tester.enterText(find.byType(TextField), 'files');
    await tester.pumpAndSettle();
    expect(find.text('File Search'), findsOneWidget);
    await tester.enterText(find.byType(TextField), 'unmatched');
    await tester.pump(const Duration(milliseconds: 20));
    expect(c.view.visibleEntries, isEmpty);
    expect(find.text('File Search'), findsNothing);
    gate.complete();
    await tester.pumpAndSettle();
    expect(c.view.visibleEntries, isEmpty);
    expect(tester.takeException(), isNull);
  });

  for (final gesture in [
    'Return',
    'double click',
    'primary button',
    'Return with lost focus'
  ]) {
    testWidgets('delayed $gesture navigation preserves editing and input',
        (tester) async {
      final gate = Completer<void>();
      final executed = <String>[];
      final queries = <(String, int, InputModeDto)>[];
      final c = await mount(tester,
          executionGate: gate, executed: executed, queries: queries);
      await tester.enterText(find.byType(TextField), 'files');
      await tester.pumpAndSettle();
      final editor = tester.state<EditableTextState>(find.byType(EditableText));
      if (gesture.startsWith('Return')) {
        await tester.sendKeyEvent(LogicalKeyboardKey.enter);
      } else if (gesture == 'double click') {
        await tester.tap(find.text('File Search'));
        await tester.pump(const Duration(milliseconds: 80));
        await tester.tap(find.text('File Search'));
      } else {
        await tester.tap(find.text('Search'));
      }
      await tester.pump(const Duration(milliseconds: 30));
      expect(executed, [entry.id]);
      if (gesture == 'Return with lost focus') {
        // Also cover a responder change during the asynchronous transition.
        tester.widget<TextField>(find.byType(TextField)).focusNode!.unfocus();
        await tester.pump();
        expect(tester.testTextInput.hasAnyClients, isFalse);
      }
      gate.complete();
      await tester.pumpAndSettle();
      expect(c.view.isFileSearch, isTrue);
      expect(tester.state<EditableTextState>(find.byType(EditableText)),
          same(editor));
      expect(tester.testTextInput.hasAnyClients, isTrue);
      expect(
          tester.widget<TextField>(find.byType(TextField)).focusNode!.hasFocus,
          isTrue);
      // Use the connection itself; enterText would silently repair focus.
      tester.testTextInput.updateEditingValue(const TextEditingValue(
          text: 'report', selection: TextSelection.collapsed(offset: 6)));
      await tester.pumpAndSettle();
      expect(queries.last.$1, 'report');
      expect(queries.last.$3, InputModeDto.fileSearch);
      expect(fileRow('report.txt'), findsOneWidget);
      await tester.sendKeyEvent(LogicalKeyboardKey.escape);
      await tester.pumpAndSettle();
      await tester.sendKeyEvent(LogicalKeyboardKey.escape);
      await tester.pumpAndSettle();
      expect(c.view.isFileSearch, isFalse);
      expect(tester.testTextInput.hasAnyClients, isTrue);
      expect(tester.takeException(), isNull);
    });
  }

  testWidgets('Scope keys cycle types and Actions preserve the fixed preview',
      (tester) async {
    final executed = <String>[];
    final queries = <(String, int, InputModeDto)>[];
    final c = await mount(tester, executed: executed, queries: queries);
    await openFiles(tester);
    await tester.enterText(find.byType(TextField), 'report');
    await tester.pumpAndSettle();
    final revision = queries.last.$2;
    // Scope and file types share bare Tab / Shift+Tab, preserving the query.
    await tester.sendKeyEvent(LogicalKeyboardKey.tab);
    await tester.pumpAndSettle();
    expect(c.files.filter, FileFilter.files);
    expect(fileRow('report-folder'), findsNothing);
    expect(c.view.input, 'report');
    expect(queries.last.$2, greaterThan(revision));
    await tester.sendKeyDownEvent(LogicalKeyboardKey.shiftLeft);
    await tester.sendKeyEvent(LogicalKeyboardKey.tab);
    await tester.sendKeyUpEvent(LogicalKeyboardKey.shiftLeft);
    await tester.pumpAndSettle();
    expect(tester.widget<TextField>(find.byType(TextField)).focusNode!.hasFocus,
        isTrue);
    expect(c.files.filter, FileFilter.all);
    expect(executed, [entry.id]);
    await chord(tester, LogicalKeyboardKey.controlLeft, LogicalKeyboardKey.tab);
    expect(c.files.filter, FileFilter.files);
    expect(fileRow('report-folder'), findsNothing);
    expect(c.view.input, 'report');
    expect(queries.last.$2, greaterThan(revision));
    await chord(tester, LogicalKeyboardKey.controlLeft, LogicalKeyboardKey.tab,
        shift: true);
    expect(c.files.filter, FileFilter.all);
    final filteredRevision = queries.last.$2;

    await chord(tester, LogicalKeyboardKey.metaLeft, LogicalKeyboardKey.keyK);
    expect(find.byKey(const Key('file-preview-action')), findsNothing);
    expect(find.text('Preview content'), findsOneWidget);
    expect(find.text('Copy Path'), findsOneWidget);
    expect(find.byKey(const Key('file-search-controls')), findsNothing);
    await tester.sendKeyEvent(LogicalKeyboardKey.tab);
    await tester.pumpAndSettle();
    expect(c.files.filter, FileFilter.all); // No switching behind Actions.
    await chord(tester, LogicalKeyboardKey.controlLeft, LogicalKeyboardKey.tab);
    expect(c.files.filter, FileFilter.all); // The action layer has no type bar.
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pumpAndSettle();
    expect(c.view.hasFilePreview, isTrue);
    expect(find.text('Preview content'), findsOneWidget);
    expect(c.view.visibleEntries[c.view.selectedIndex].candidate!.id, text.id);
    expect(
        executed, [entry.id]); // Host preview does not execute or record Usage.
    expect(c.view.hasFilePreview, isTrue);
    expect(c.view.input, 'report');
    expect(queries.last.$2, filteredRevision);
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pumpAndSettle();
    expect(c.view.input, '');
    expect(c.view.isFileSearch, isTrue);
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pumpAndSettle();
    expect(c.view.isFileSearch, isFalse);
    expect(executed, [entry.id]);
    expect(tester.takeException(), isNull);
  });

  testWidgets('Scope-style type chips support all categories and enlarged text',
      (tester) async {
    final hits = [
      for (final name in [
        'report.txt',
        'report.PDF',
        'report.png',
        'report.mp3',
        'report.mp4',
        'report.zip',
        'report.dart'
      ])
        row(name, name, 'core.file.open', file: true),
      row('pdf-folder', 'report.pdf', 'core.directory.open', file: true),
    ];
    final expectedByType = {
      'documents': ['report.txt', 'report.PDF'],
      'pdf': ['report.PDF'],
      'images': ['report.png'],
      'audio': ['report.mp3'],
      'video': ['report.mp4'],
      'archives': ['report.zip'],
      'code': ['report.dart'],
    };
    final c = await mount(tester, hits: hits, textScale: 1.35, filteredHits: {
      for (final type in expectedByType.entries)
        type.key: hits.where((row) => type.value.contains(row.title)).toList()
    });
    await openFiles(tester);
    await tester.enterText(find.byType(TextField), 'report');
    await tester.pumpAndSettle();
    expect(find.byType(LauncherFilterBar), findsOneWidget);
    expect(find.byKey(const Key('file-preview-toggle')), findsNothing);
    for (final category in FileFilter.values.skip(3)) {
      await filterFiles(tester, category);
      final expected = expectedByType[category.name]!;
      for (final hit in hits) {
        expect(
            find.descendant(
                of: find.byKey(const Key('action-list')),
                matching: find.text(hit.title)),
            expected.contains(hit.title) ? findsOneWidget : findsNothing);
      }
      expect(c.files.filter, category);
      expect(tester.takeException(), isNull);
    }
    expect(find.byKey(const Key('file-filter-code')).hitTestable(),
        findsOneWidget);
    await saveLauncherUi(tester, 'files-types');
  });

  testWidgets('file Actions and Back retain the fixed target and parent',
      (tester) async {
    final queries = <(String, int, InputModeDto)>[];
    final c = await mount(tester, queries: queries);
    await openFiles(tester);
    await tester.enterText(find.byType(TextField), 'report');
    await tester.pumpAndSettle();
    await tester.sendKeyEvent(LogicalKeyboardKey.arrowDown);
    await tester.pumpAndSettle();
    final selected = c.view.visibleEntries[c.view.selectedIndex].candidate!.id;
    expect(c.files.preview!.path, '/project/report-folder');
    await chord(tester, LogicalKeyboardKey.metaLeft, LogicalKeyboardKey.keyK);
    expect(find.byKey(const Key('file-preview-pane')), findsOneWidget);
    expect(find.byKey(const Key('file-preview-action')), findsNothing);
    expect(c.files.preview!.path, '/project/report-folder');
    await tester.tap(find.byTooltip('Back to results'));
    await tester.pumpAndSettle();
    expect(c.view.visibleEntries[c.view.selectedIndex].candidate!.id, selected);
    expect(c.view.input, 'report');
    expect(c.view.hasFilePreview, isTrue);
    await tester.tap(find.byTooltip('Back to results'));
    await tester.pumpAndSettle();
    expect(c.view.isFileSearch, isFalse);
    expect(c.view.input, 'files');
    expect(tester.takeException(), isNull);
  });

  testWidgets('reverse Scope key wraps types and reveals offscreen chips',
      (tester) async {
    tester.view.physicalSize = const Size(648, 297);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    final executed = <String>[];
    final c = await mount(tester, executed: executed);
    await openFiles(tester);
    await tester.sendKeyDownEvent(LogicalKeyboardKey.shiftLeft);
    await tester.sendKeyEvent(LogicalKeyboardKey.tab);
    await tester.sendKeyUpEvent(LogicalKeyboardKey.shiftLeft);
    await tester.pumpAndSettle();
    expect(c.files.filter, FileFilter.code);
    expect(find.byKey(const Key('file-filter-code')).hitTestable(),
        findsOneWidget);
    await tester.sendKeyEvent(LogicalKeyboardKey.tab);
    await tester.pumpAndSettle();
    expect(c.files.filter, FileFilter.all);
    expect(
        find.byKey(const Key('file-filter-all')).hitTestable(), findsOneWidget);
    expect(c.view.input, '');
    expect(executed, [entry.id]);
    expect(tester.testTextInput.hasAnyClients, isTrue);
    expect(tester.takeException(), isNull);
  });

  testWidgets('empty Global Search has no pinned tools or footer',
      (tester) async {
    await mount(tester);
    expect(find.text('File Search'), findsNothing);
    expect(find.byKey(const Key('launcher-footer')), findsNothing);
    expect(find.byKey(const Key('scope-bar')), findsNothing);
    expect(tester.getSize(find.byKey(const Key('launcher-panel'))).height, 72);
    await saveLauncherUi(tester, 'global-empty');
  });

  testWidgets(
      'Tab respects visible scopes, navigation layers and IME ownership',
      (tester) async {
    final queries = <(String, int, InputModeDto)>[];
    final c = await mount(tester, queries: queries, environmentScope: true);
    final editor = tester.state<EditableTextState>(find.byType(EditableText));
    final initialQueries = queries.length;
    await tester.sendKeyEvent(LogicalKeyboardKey.tab);
    await tester.pumpAndSettle();
    expect(c.view.inputMode, InputModeDto.search);
    expect(c.view.scopeId, isNull);
    expect(queries.length, initialQueries);

    await c.navigateRoot(WakeMode.currentApp);
    await tester.pumpAndSettle();
    expect(c.view.scopeId, 'app.editor');
    await tester.tap(find.byKey(const Key('launcher-navigation')));
    await tester.pumpAndSettle();
    final beforeNavigation = queries.length;
    await tester.sendKeyEvent(LogicalKeyboardKey.tab);
    await tester.pumpAndSettle();
    expect(c.view.navigationOpen, isTrue);
    expect(c.view.scopeId, 'app.editor');
    expect(queries.length, beforeNavigation);
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pumpAndSettle();

    const composing = TextEditingValue(
        text: 'peizhi',
        composing: TextRange(start: 0, end: 6),
        selection: TextSelection.collapsed(offset: 6));
    tester.testTextInput.updateEditingValue(composing);
    await tester.pump();
    await tester.sendKeyEvent(LogicalKeyboardKey.tab);
    await chord(tester, LogicalKeyboardKey.shiftLeft, LogicalKeyboardKey.tab);
    await chord(tester, LogicalKeyboardKey.controlLeft, LogicalKeyboardKey.tab);
    expect(c.view.scopeId, 'app.editor');
    expect(queries.length, beforeNavigation);
    expect(tester.widget<TextField>(find.byType(TextField)).controller!.value,
        composing);
    tester.testTextInput.updateEditingValue(const TextEditingValue(
        text: '配置', selection: TextSelection.collapsed(offset: 2)));
    await tester.pumpAndSettle();
    await tester.sendKeyEvent(LogicalKeyboardKey.tab);
    await tester.pumpAndSettle();
    expect(c.view.scopeId, 'folder');
    expect(c.view.input, '配置');
    expect(tester.state<EditableTextState>(find.byType(EditableText)),
        same(editor));
    expect(tester.widget<TextField>(find.byType(TextField)).focusNode!.hasFocus,
        isTrue);
    expect(tester.testTextInput.hasAnyClients, isTrue);
    expect(tester.takeException(), isNull);
  });

  testWidgets(
      'a user configured Action shortcut opens files from a blank launcher',
      (tester) async {
    const shortcut = ActionShortcut(
        actionId: 'core.files.search',
        title: 'File Search',
        keyCode: 9,
        modifiers: HostSettings.altKey);
    final executed = <String>[];
    final c = await mount(tester, shortcut: shortcut, executed: executed);
    expect(find.text('File Search'), findsNothing);
    await tester.sendKeyDownEvent(LogicalKeyboardKey.altLeft);
    await tester.sendKeyEvent(LogicalKeyboardKey.keyF);
    await tester.sendKeyUpEvent(LogicalKeyboardKey.altLeft);
    await tester.pumpAndSettle();
    expect(c.view.isFileSearch, isTrue);
    expect(executed, [entry.id]);
    expect(tester.testTextInput.hasAnyClients, isTrue);
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pumpAndSettle();
    expect(c.view.inputMode, InputModeDto.search);
    expect(find.byKey(const Key('launcher-footer')), findsNothing);
    expect(tester.takeException(), isNull);
  });

  testWidgets(
      'mode chooser resizes the empty launcher and uses arrows, Enter and Escape',
      (tester) async {
    final queries = <(String, int, InputModeDto)>[];
    final c = await mount(tester, queries: queries, environmentScope: true);
    final editor = tester.state<EditableTextState>(find.byType(EditableText));
    final queryCount = queries.length;
    final calls = <MethodCall>[];
    messenger.setMockMethodCallHandler(channel, (call) async {
      calls.add(call);
      return null;
    });
    await tester.tap(find.byKey(const Key('launcher-navigation')));
    await tester.pumpAndSettle();
    expect(c.view.height, 181);
    expect(
        calls.any((call) =>
            call.method == 'resize' &&
            (call.arguments as Map)['height'] == 181),
        isTrue);
    final panel = tester.getRect(find.byKey(const Key('launcher-panel')));
    for (final mode in ['search', 'environment']) {
      final choice = find.byKey(Key('launcher-navigation-$mode'));
      expect(choice.hitTestable(), findsOneWidget);
      expect(panel.contains(tester.getBottomRight(choice)), isTrue);
    }
    expect(find.byType(PopupMenuButton<InputModeDto>), findsNothing);
    await saveLauncherUi(tester, 'mode-chooser');
    await tester.sendKeyEvent(LogicalKeyboardKey.arrowDown);
    await tester.pumpAndSettle();
    expect(c.view.navigationMode, InputModeDto.environment);
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pumpAndSettle();
    expect(c.view.navigationOpen, isFalse);
    expect(c.view.height, 72);
    expect(queries.length, queryCount);
    expect(c.view.inputMode, InputModeDto.search);
    expect(tester.state<EditableTextState>(find.byType(EditableText)),
        same(editor));

    await tester.tap(find.byKey(const Key('launcher-navigation')));
    await tester.pumpAndSettle();
    await tester.sendKeyEvent(LogicalKeyboardKey.arrowDown);
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pumpAndSettle();
    expect(c.view.navigationOpen, isFalse);
    expect(c.view.inputMode, InputModeDto.environment);
    expect(c.view.scopeId, 'app.editor');
    expect(tester.testTextInput.hasAnyClients, isTrue);
    await tester.enterText(find.byType(TextField), 'files');
    await tester.pumpAndSettle();
    expect(queries.last.$1, 'files');
    expect(tester.takeException(), isNull);
  });

  testWidgets(
      'root navigation keeps one input and never toggles the window closed',
      (tester) async {
    final queries = <(String, int, InputModeDto)>[];
    final c = await mount(tester, queries: queries, environmentScope: true);
    final editor = find.descendant(
        of: find.byKey(const Key('launcher-search-input')),
        matching: find.byType(EditableText));
    final editing = tester.state<EditableTextState>(editor);
    await tester.tap(find.byKey(const Key('launcher-navigation')));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const Key('launcher-navigation-environment')));
    await tester.pumpAndSettle();
    expect(queries.last.$3, InputModeDto.environment);
    expect(c.view.scopeId, 'app.editor');
    await tester.tap(find.byKey(const Key('launcher-navigation')));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const Key('launcher-navigation-environment')));
    await tester.pumpAndSettle();
    expect(queries.last.$3, InputModeDto.environment);
    expect(tester.testTextInput.hasAnyClients, isTrue);
    expect(tester.state<EditableTextState>(find.byType(EditableText)),
        same(editing));
    await tester.enterText(find.byType(TextField), 'files');
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const Key('launcher-navigation')));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const Key('launcher-navigation-search')));
    await tester.pumpAndSettle();
    expect(queries.last.$3, InputModeDto.search);
    expect(queries.last.$1, 'files');
    expect(tester.testTextInput.hasAnyClients, isTrue);
    expect(tester.state<EditableTextState>(find.byType(EditableText)),
        same(editing));
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pumpAndSettle();
    expect(find.byKey(const Key('launcher-footer')), findsNothing);
    expect(tester.takeException(), isNull);
  });

  testWidgets(
      'entering file search retains focus and accepts typing without a click',
      (tester) async {
    final queries = <(String, int, InputModeDto)>[];
    await mount(tester, queries: queries);
    final originalEditable =
        tester.state<EditableTextState>(find.byType(EditableText));
    expect(tester.testTextInput.hasAnyClients, isTrue);
    expect(tester.widget<TextField>(find.byType(TextField)).focusNode!.hasFocus,
        isTrue);
    await openFiles(tester);
    final field = tester.widget<TextField>(find.byType(TextField));
    expect(field.focusNode!.hasFocus, isTrue);
    expect(tester.testTextInput.hasAnyClients, isTrue);
    expect(tester.state<EditableTextState>(find.byType(EditableText)),
        same(originalEditable));
    // Send editing input through the existing connection: enterText would focus
    // the field itself and hide a lost-focus regression.
    tester.testTextInput.updateEditingValue(const TextEditingValue(
        text: 'report', selection: TextSelection.collapsed(offset: 6)));
    await tester.pumpAndSettle();
    expect(queries.last.$1, 'report');
    expect(queries.last.$3, InputModeDto.fileSearch);
    expect(fileRow('report.txt'), findsOneWidget);
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pumpAndSettle();
    expect(queries.last.$1, '');
    expect(queries.last.$3, InputModeDto.fileSearch);
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pumpAndSettle();
    expect(tester.widget<TextField>(find.byType(TextField)).focusNode!.hasFocus,
        isTrue);
    expect(tester.testTextInput.hasAnyClients, isTrue);
    expect(tester.state<EditableTextState>(find.byType(EditableText)),
        same(originalEditable));
    tester.testTextInput.updateEditingValue(const TextEditingValue(
        text: 'files', selection: TextSelection.collapsed(offset: 5)));
    await tester.pumpAndSettle();
    expect(queries.last.$1, 'files');
    expect(queries.last.$3, InputModeDto.search);
    // Re-enter using the mouse, then exercise the input method connection too.
    await tester.tap(find.text('File Search'));
    await tester.pump(const Duration(milliseconds: 80));
    await tester.tap(find.text('File Search'));
    await tester.pumpAndSettle();
    expect(tester.testTextInput.hasAnyClients, isTrue);
    expect(tester.state<EditableTextState>(find.byType(EditableText)),
        same(originalEditable));
    tester.testTextInput.updateEditingValue(const TextEditingValue(
        text: '报告',
        composing: TextRange(start: 0, end: 2),
        selection: TextSelection.collapsed(offset: 2)));
    await tester.pump();
    expect(queries.last.$1, '');
    tester.testTextInput.updateEditingValue(const TextEditingValue(
        text: '报告', selection: TextSelection.collapsed(offset: 2)));
    await tester.pumpAndSettle();
    expect(queries.last.$1, '报告');
    expect(queries.last.$3, InputModeDto.fileSearch);
    expect(tester.takeException(), isNull);
  });

  testWidgets(
      'background search progress preserves the third Chinese character composition',
      (tester) async {
    final pending = <Completer<SnapshotWaitDto>>[];
    final queries = <(String, int, InputModeDto)>[];
    final executed = <String>[];
    final c = await mount(tester, queries: queries, executed: executed,
        waitDelta: (_, __, ___, ____) {
      final response = Completer<SnapshotWaitDto>();
      pending.add(response);
      return response.future;
    });
    await openFiles(tester);
    await tester.enterText(find.byType(TextField), '配置');
    await tester.pumpAndSettle();
    final editor = find.descendant(
        of: find.byKey(const Key('launcher-search-input')),
        matching: find.byType(EditableText));
    final editing = tester.state<EditableTextState>(editor);
    final field = tester.widget<TextField>(find.byType(TextField));
    final revision = queries.last.$2;
    for (var batch = 0; batch < 2; batch++) {
      final composing = TextEditingValue(
          text: batch == 0 ? '配置yan' : '配置yanshou',
          selection: TextSelection.collapsed(offset: batch == 0 ? 5 : 9),
          composing: TextRange(start: 2, end: batch == 0 ? 5 : 9));
      tester.testTextInput.updateEditingValue(composing);
      await tester.pump();
      // The native input method owns this suffix. Progress still describes the
      // last submitted query and must never be written back into the editor.
      pending.last.complete(SnapshotWaitDto(
          kind: SnapshotWaitKindDto.updated,
          candidateUpdate: CandidateUpdateDto.retain,
          state: launcherSnapshot(
              sessionId: 1,
              revision: revision,
              snapshotSeq: batch + 20,
              candidateListVersion: 1,
              input: '配置',
              contextComplete: true,
              candidates: [],
              diagnostics: [],
              fileSearch: searchProgress())));
      await tester.pumpAndSettle();
      expect(field.controller!.value, composing);
      expect(tester.state<EditableTextState>(editor), same(editing));
      expect(tester.testTextInput.hasAnyClients, isTrue);
      await tester.sendKeyEvent(LogicalKeyboardKey.tab);
      await chord(tester, LogicalKeyboardKey.shiftLeft, LogicalKeyboardKey.tab);
      expect(c.files.filter, FileFilter.all);
      expect(field.controller!.value, composing);
      await tester.sendKeyEvent(LogicalKeyboardKey.enter);
      await tester.pumpAndSettle();
      expect(executed, [entry.id]);
      expect(queries.last.$1, '配置');
    }
    tester.testTextInput.updateEditingValue(const TextEditingValue(
        text: '配置验收', selection: TextSelection.collapsed(offset: 4)));
    await tester.pumpAndSettle();
    expect(queries.last.$1, '配置验收');
    expect(field.controller!.text, '配置验收');
    // A deliberate new Host query still replaces an unfinished composition.
    tester.testTextInput.updateEditingValue(const TextEditingValue(
        text: '配置验收ce',
        selection: TextSelection.collapsed(offset: 6),
        composing: TextRange(start: 4, end: 6)));
    await tester.pump();
    await c.update('explicit host query');
    await tester.pumpAndSettle();
    expect(field.controller!.text, 'explicit host query');
    expect(field.controller!.value.composing.isCollapsed, isTrue);
    await c.dismiss(retain: false);
    pending.last
        .complete(const SnapshotWaitDto(kind: SnapshotWaitKindDto.superseded));
    await tester.pumpWidget(const SizedBox());
    expect(tester.takeException(), isNull);
  });

  testWidgets(
      'Action opens workspace; filters, preview and Enter use selected file',
      (tester) async {
    final executed = <String>[];
    final queries = <(String, int, InputModeDto)>[];
    await mount(tester, executed: executed, queries: queries);
    expect(find.text('File Search'), findsNothing);
    await tester.enterText(find.byType(TextField), 'files');
    await tester.pumpAndSettle();
    await saveLauncherUi(tester, 'launcher');
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pumpAndSettle();
    expect(find.byKey(const Key('file-search-controls')), findsOneWidget);
    expect(find.byKey(const Key('scope-bar')), findsNothing);
    expect(queries.last.$3, InputModeDto.fileSearch);
    expect(find.byType(LauncherSearchArea), findsOneWidget);
    expect(find.byKey(const Key('launcher-footer')), findsNothing);
    expect(find.byType(AppToolbar), findsNothing);
    expect(find.byType(TextField), findsOneWidget);
    expect(tester.getSize(find.byType(LauncherSearchArea)).height, 72);
    expect(
        tester.widget<TextField>(find.byType(TextField)).style!.fontSize, 21);
    expect(tester.getSize(find.byKey(const Key('launcher-panel'))),
        const Size(920, 600));
    expect(find.byKey(const Key('file-preview-pane')), findsOneWidget);
    await saveLauncherUi(tester, 'files-empty');

    await tester.enterText(
        find.byKey(const Key('launcher-search-input')), 'report');
    await tester.pumpAndSettle();
    await filterFiles(tester, FileFilter.files);
    expect(fileRow('report.txt'), findsOneWidget);
    expect(fileRow('report-folder'), findsNothing);
    expect(find.byKey(const Key('action-list')), findsOneWidget);
    await saveLauncherUi(tester, 'files-results');
    final compactHeight =
        tester.getSize(find.byKey(const Key('launcher-panel'))).height;

    await openPreview(tester, capture: true);
    expect(find.text('Preview content'), findsOneWidget);
    expect(find.text('12 B'), findsNothing);
    await tester.tap(find.text('File information'));
    await tester.pumpAndSettle();
    expect(find.text('12 B'), findsOneWidget);
    await saveLauncherUi(tester, 'files-preview');
    expect(find.byTooltip('Close preview'), findsNothing);
    expect(tester.getSize(find.byKey(const Key('launcher-panel'))).height,
        compactHeight);
    await filterFiles(tester, FileFilter.folders);
    expect(fileRow('report.txt'), findsNothing);
    expect(
        find.descendant(
            of: find.byKey(const Key('action-list')),
            matching: fileRow('report-folder')),
        findsOneWidget);
    expect(find.text('Preview content'), findsNothing);
    expect(executed, [entry.id]);
    expect(tester.takeException(), isNull);
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pumpAndSettle();
    expect(executed, [entry.id, folder.id]);
  });

  testWidgets(
      'list navigation remains consistent with Scope-style file filtering',
      (tester) async {
    final executed = <String>[];
    final controller = await mount(tester, executed: executed);
    await openFiles(tester);
    tester.testTextInput.updateEditingValue(const TextEditingValue(
        text: 'report', selection: TextSelection.collapsed(offset: 6)));
    await tester.pumpAndSettle();
    await tester.sendKeyDownEvent(LogicalKeyboardKey.controlLeft);
    await tester.sendKeyEvent(LogicalKeyboardKey.tab);
    await tester.sendKeyUpEvent(LogicalKeyboardKey.controlLeft);
    await tester.pumpAndSettle();
    expect(controller.files.filter, FileFilter.files);
    await chord(tester, LogicalKeyboardKey.controlLeft, LogicalKeyboardKey.tab,
        shift: true);
    expect(controller.files.filter, FileFilter.all);
    expect(fileRow('report-folder'), findsOneWidget);
    await tester.sendKeyDownEvent(LogicalKeyboardKey.metaLeft);
    await tester.sendKeyEvent(LogicalKeyboardKey.keyP);
    await tester.sendKeyUpEvent(LogicalKeyboardKey.metaLeft);
    await tester.pumpAndSettle();
    expect(controller.view.hasFilePreview, isTrue);
    await tester.sendKeyEvent(LogicalKeyboardKey.arrowDown);
    await tester.pumpAndSettle();
    expect(
        controller
            .view.visibleEntries[controller.view.selectedIndex].candidate!.id,
        folder.id);
    await tester.sendKeyEvent(LogicalKeyboardKey.arrowUp);
    await tester.pumpAndSettle();
    await openPreview(tester);
    expect(find.text('Preview content'), findsOneWidget);
    await chord(tester, LogicalKeyboardKey.metaLeft, LogicalKeyboardKey.keyK);
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pumpAndSettle();
    expect(controller.view.hasFilePreview, isTrue);
    expect(controller.files.preview, isNotNull);
    expect(tester.widget<TextField>(find.byType(TextField)).controller!.text,
        'report');
    expect(tester.testTextInput.hasAnyClients, isTrue);
    expect(executed, [entry.id]);
    expect(tester.takeException(), isNull);
  });

  testWidgets(
      'empty guidance and preview remain usable with larger desktop text',
      (tester) async {
    await mount(tester, textScale: 1.35);
    await openFiles(tester);
    expect(find.text('Find files and folders'), findsOneWidget);
    expect(find.text('Run'), findsNothing);
    expect(tester.takeException(), isNull);
    tester.testTextInput.updateEditingValue(const TextEditingValue(
        text: 'report', selection: TextSelection.collapsed(offset: 6)));
    await tester.pumpAndSettle();
    await tester.pumpAndSettle();
    expect(find.byKey(const Key('file-preview-close')), findsNothing);
    expect(find.byKey(const Key('file-preview-pane')), findsOneWidget);
    expect(find.text('Open').hitTestable(), findsOneWidget);
    expect(tester.takeException(), isNull);
    await tester.tap(find.byKey(const Key('selected-row-actions')));
    await tester.pumpAndSettle();
    expect(find.text('Copy Path').hitTestable(), findsOneWidget);
    expect(find.byIcon(AppIcons.copy), findsOneWidget,
        reason: 'secondary file actions keep their operation icon');
    expect(tester.takeException(), isNull);
  });

  testWidgets('fixed preview adapts to a smaller screen without overflow',
      (tester) async {
    await mount(tester, textScale: 1.35);
    await tester.binding.setSurfaceSize(const Size(640, 480));
    await openFiles(tester);
    await tester.enterText(find.byType(TextField), 'report');
    await tester.pumpAndSettle();
    expect(find.byKey(const Key('file-preview-pane')), findsOneWidget);
    expect(tester.getSize(find.byKey(const Key('launcher-panel'))),
        const Size(640, 480));
    expect(find.text('Preview content'), findsOneWidget);
    expect(find.text('Open').hitTestable(), findsOneWidget);
    expect(tester.takeException(), isNull);
  });

  testWidgets('file window size is stable and restored when leaving the tool',
      (tester) async {
    final messages = <Map>[];
    messenger.setMockMethodCallHandler(channel, (call) async {
      if (call.method == 'show' || call.method == 'resize') {
        messages.add(call.arguments as Map);
      }
      return null;
    });
    final controller = await mount(tester);
    await openFiles(tester);
    expect(messages.last['width'], 920);
    expect(messages.last['height'], 600);
    expect(find.byKey(const Key('file-preview-pane')), findsOneWidget);
    final pane = tester.getRect(find.byKey(const Key('file-preview-pane')));
    await tester.enterText(find.byType(TextField), 'report');
    await tester.pumpAndSettle();
    expect(tester.getRect(find.byKey(const Key('file-preview-pane'))), pane);
    expect(find.byKey(const Key('file-preview-close')), findsNothing);
    await chord(tester, LogicalKeyboardKey.metaLeft, LogicalKeyboardKey.keyK);
    expect(tester.getSize(find.byKey(const Key('launcher-panel'))),
        const Size(920, 600));
    expect(find.byKey(const Key('file-preview-action')), findsNothing);
    expect(find.byKey(const Key('file-preview-pane')), findsOneWidget);
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pumpAndSettle();
    await controller.closeFileSearch();
    await tester.pumpAndSettle();
    expect(messages.last['width'], 648);
    expect(find.byKey(const Key('file-preview-pane')), findsNothing);
    expect(tester.takeException(), isNull);
  });

  testWidgets('Actions and Esc preserve file query then return to parent query',
      (tester) async {
    final executed = <String>[];
    await mount(tester, executed: executed);
    await tester.enterText(find.byType(TextField), 'files');
    await tester.pumpAndSettle();
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pumpAndSettle();
    await tester.enterText(find.byType(TextField), 'report');
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const Key('selected-row-actions')));
    await tester.pumpAndSettle();
    expect(find.text('Copy Path'), findsOneWidget);
    expect(find.byKey(const Key('file-search-controls')), findsNothing);
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pumpAndSettle();
    expect(fileRow('report.txt'), findsOneWidget);
    expect(tester.widget<TextField>(find.byType(TextField)).controller!.text,
        'report');
    await tester.tap(find.byKey(const Key('selected-row-actions')));
    await tester.pumpAndSettle();
    await tester.sendKeyEvent(LogicalKeyboardKey.arrowDown);
    await tester.pumpAndSettle();
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pumpAndSettle();
    expect(executed, [entry.id, 'copy-text']);
  });

  testWidgets(
      'Esc clears query before returning to parent; Retry advances revision',
      (tester) async {
    final queries = <(String, int, InputModeDto)>[];
    await mount(tester, queries: queries, failFirst: true);
    await tester.enterText(find.byType(TextField), 'files');
    await tester.pumpAndSettle();
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pumpAndSettle();
    await tester.enterText(find.byType(TextField), 'report');
    await tester.pumpAndSettle();
    expect(find.text('Retry'), findsOneWidget);
    final revision = queries.last.$2;
    await tester.tap(find.text('Retry'));
    await tester.pumpAndSettle();
    expect(queries.last.$1, 'report');
    expect(queries.last.$2, greaterThan(revision));
    expect(find.text('Retry'), findsNothing);
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pumpAndSettle();
    expect(find.byKey(const Key('file-search-controls')), findsOneWidget);
    expect(queries.last.$1, '');
    expect(queries.last.$3, InputModeDto.fileSearch);
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pumpAndSettle();
    expect(find.byKey(const Key('file-search-controls')), findsNothing);
    expect(tester.widget<TextField>(find.byType(TextField)).controller!.text,
        'files');
    expect(queries.last.$3, InputModeDto.search);
  });

  testWidgets('shared back button returns to parent without executing a file',
      (tester) async {
    final executed = <String>[];
    await mount(tester, executed: executed);
    await tester.enterText(find.byType(TextField), 'files');
    await tester.pumpAndSettle();
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pumpAndSettle();
    await tester.enterText(find.byType(TextField), 'report');
    await tester.pumpAndSettle();
    await tester.tap(fileRow('report-folder'));
    await tester.pumpAndSettle();
    expect(executed, [entry.id]);
    await tester.tap(find.byTooltip('Back to results'));
    await tester.pumpAndSettle();
    expect(find.byKey(const Key('file-search-controls')), findsNothing);
    expect(tester.widget<TextField>(find.byType(TextField)).controller!.text,
        'files');
    expect(executed, [entry.id]);
    expect(tester.takeException(), isNull);
  });

  testWidgets(
      'right preview follows keyboard selection and preserves the file workspace',
      (tester) async {
    final queries = <(String, int, InputModeDto)>[];
    final loaded = <String>[];
    final hits = List.generate(60,
        (i) => row('file-$i', 'report-$i.txt', 'core.file.open', file: true));
    final controller = await mount(tester, hits: hits, queries: queries,
        load: (_, __, id) async {
      loaded.add(id);
      return FilePreviewDto(
          path: '/project/$id.txt',
          isDirectory: false,
          sizeBytes: BigInt.from(1000),
          text: List.generate(80, (i) => '$id line $i').join('\n'));
    });
    await openFiles(tester);
    await tester.enterText(find.byType(TextField), 'report');
    await tester.pumpAndSettle();
    for (var i = 0; i < 35; i++) {
      await tester.sendKeyEvent(LogicalKeyboardKey.arrowDown);
      await tester.pumpAndSettle();
    }
    final list = tester.widget<ListView>(find.byKey(const Key('action-list')));
    final offset = list.controller!.offset;
    expect(offset, greaterThan(1000));
    final queryCount = queries.length;

    await openPreview(tester);
    expect(controller.view.selectedIndex, 35);
    expect(list.controller!.offset, closeTo(offset, .01));
    expect(loaded.last, 'file-35');
    expect(find.text('/project/file-35.txt'), findsOneWidget);
    expect(
        tester.getTopLeft(find.byKey(const Key('file-preview'))).dx,
        greaterThan(
            tester.getTopLeft(find.byKey(const Key('action-list'))).dx));

    final content = find.byKey(const Key('file-preview'));
    await tester.drag(content, const Offset(0, -350));
    await tester.pumpAndSettle();
    ScrollPosition previewPosition() => tester
        .state<ScrollableState>(find
            .descendant(
                of: find.byKey(const Key('file-preview')),
                matching: find.byType(Scrollable))
            .first)
        .position;
    expect(previewPosition().pixels, greaterThan(0));

    await tester.sendKeyEvent(LogicalKeyboardKey.arrowDown);
    await tester.pumpAndSettle();
    expect(controller.view.selectedIndex, 36);
    expect(loaded.last, 'file-36');
    expect(find.text('/project/file-36.txt'), findsOneWidget);
    expect(find.text('/project/file-35.txt'), findsNothing);
    expect(previewPosition().pixels, 0);
    final input = tester.widget<TextField>(find.byType(TextField)).controller!;
    // Escape from selectable preview content follows the main input rule:
    // clear the query, keep the fixed pane, then return to the parent tool.
    tester
        .widget<EditableText>(find
            .descendant(
                of: find.byKey(const Key('file-preview')),
                matching: find.byType(EditableText))
            .last)
        .focusNode
        .requestFocus();
    await tester.pump();
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pumpAndSettle();
    expect(input.text, '');
    expect(find.byKey(const Key('file-preview-pane')), findsOneWidget);
    expect(find.byKey(const Key('file-preview')), findsNothing);
    expect(queries.length, queryCount + 1);
    expect(tester.testTextInput.hasAnyClients, isTrue);
    tester.testTextInput.updateEditingValue(const TextEditingValue(
        text: 'report next', selection: TextSelection.collapsed(offset: 11)));
    await tester.pumpAndSettle();
    expect(queries.last.$1, 'report next');
    expect(tester.takeException(), isNull);
  });

  testWidgets(
      'clearing query cancels stale preview while the fixed pane remains',
      (tester) async {
    final pending = Completer<FilePreviewDto>();
    var requests = 0;
    final controller = await mount(tester, load: (_, __, id) {
      requests++;
      return requests == 1
          ? pending.future
          : Future.value(preview('/project/fresh.txt'));
    });
    await openFiles(tester);
    await tester.enterText(find.byType(TextField), 'report');
    await tester.pump(const Duration(milliseconds: 130));
    expect(requests, 1);
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pumpAndSettle();
    pending.complete(preview('/project/stale.txt'));
    await tester.pumpAndSettle();
    expect(controller.files.preview, isNull);
    expect(controller.view.input, '');
    expect(find.byKey(const Key('file-preview-close')), findsNothing);
    await tester.enterText(find.byType(TextField), 'report next');
    await tester.pumpAndSettle();
    expect(requests, 2);
    expect(find.text('/project/fresh.txt'), findsOneWidget);
    expect(find.text('/project/stale.txt'), findsNothing);
    expect(tester.takeException(), isNull);
  });

  testWidgets(
      'late preview cannot replace a newer selection or survive closing',
      (tester) async {
    final first = Completer<FilePreviewDto>(),
        second = Completer<FilePreviewDto>();
    final controller = await mount(tester,
        load: (_, __, id) => id == text.id ? first.future : second.future);
    await openFiles(tester);
    await tester.enterText(find.byType(TextField), 'report');
    await tester.pump(const Duration(milliseconds: 130));
    controller.move(1);
    await tester.pump(const Duration(milliseconds: 130));
    second.complete(preview('/new', directory: true));
    await tester.pumpAndSettle();
    first.complete(preview('/old'));
    await tester.pumpAndSettle();
    expect(find.text('/new'), findsOneWidget);
    expect(find.text('/old'), findsNothing);
    await controller.closeFileSearch();
    await tester.pumpAndSettle();
    expect(controller.files.preview, isNull);
  });
}
