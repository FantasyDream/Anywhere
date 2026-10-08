import 'package:anywhere/infrastructure/desktop_environment.dart';
import 'package:anywhere/ui/app_icons.dart';
import 'package:anywhere/ui/app_components.dart';
import 'dart:async';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:anywhere/launcher_screen.dart';
import 'package:anywhere/launcher/launcher_controller.dart';
import 'package:anywhere/launcher/launcher_view_state.dart';
import 'package:anywhere/infrastructure/window_host.dart';
import 'package:anywhere/settings/settings_screen.dart';
import 'package:anywhere/src/rust/api.dart';
import 'launcher_fixtures.dart';

/// Two action candidates with the documented stable ids.
CandidateDto candidate(
  String id,
  String title, {
  CandidateKindDto kind = CandidateKindDto.action,
  AvailabilityDto availability = AvailabilityDto.enabled,
  String? reason,
  String iconKey = 'file',
  String groupId = 'folder',
  double score = 100,
  bool homeEligible = true,
  String? sectionLabel,
  String? executionContextId,
  String? executionTargetLabel,
  String? subtitle = 'from Rust',
  List<String> menuPath = const [],
}) =>
    CandidateDto(
      contextKeys: const [],
      homeEligible: homeEligible,
      primaryAction: false,
      id: id,
      kind: kind,
      title: title,
      menuPath: menuPath,
      sectionLabel: sectionLabel,
      executionContextId: executionContextId,
      executionTargetLabel: executionTargetLabel,
      subtitle: subtitle,
      iconKey: iconKey,
      score: score,
      groupId: groupId,
      availability: availability,
      availabilityReason: reason,
      commandId: kind == CandidateKindDto.action
          ? (id.contains(':') ? id.split(':')[1] : id)
          : null,
      bundleId: kind == CandidateKindDto.application
          ? 'com.example.${id.split(':')[1]}'
          : null,
    );

LauncherStateDto state(
  int revision,
  String title, {
  int snapshotSeq = 1,
  bool contextComplete = true,
  String input = '',
}) {
  final first = candidate('action:first:c1', title);
  final second =
      candidate('action:second:c1', 'Second action', iconKey: 'copy');
  return launcherSnapshot(
    sessionId: 1,
    revision: revision,
    snapshotSeq: snapshotSeq,
    input: input,
    contextComplete: contextComplete,
    contexts: [],
    contextCaptures: const [],
    candidates: [first, second],
    roots: const [],
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
  setUp(() => DesktopEnvironment.configure(
      const DesktopEnvironment(platform: TargetPlatform.macOS)));
  tearDown(DesktopEnvironment.reset);
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

  test(
      'explicit feature scopes expose run targets and custom scripts without flooding home',
      () {
    final actions = [
      candidate('action:custom:c1', 'pnpm Run · check:types',
          groupId: 'node', homeEligible: false),
      candidate('action:run:c2', 'Cargo Run · server / main',
          groupId: 'rust', homeEligible: false),
      candidate('action:copy:c3', 'Copy Path'),
    ];
    final snapshot = launcherSnapshot(
        sessionId: 1,
        revision: 0,
        snapshotSeq: 1,
        contextComplete: true,
        input: '',
        contexts: [],
        diagnostics: [],
        candidates: actions,
        contextCandidates: actions,
        scopes: const [
          ScopeDto(id: 'node', label: 'Node', isCurrentApp: false),
          ScopeDto(id: 'rust', label: 'Rust', isCurrentApp: false)
        ]);
    final home = LauncherViewState(
        phase: LauncherPhase.ready, generation: 1, snapshot: snapshot);
    expect(home.visibleCandidates.map((c) => c.title), ['Copy Path']);
    final node = LauncherViewState(
        phase: LauncherPhase.ready,
        generation: 1,
        snapshot: snapshot,
        scopeId: 'node');
    expect(
        node.visibleCandidates.map((c) => c.title), ['pnpm Run · check:types']);
    expect(node.resultHeading, 'Actions');
    final rust = LauncherViewState(
        phase: LauncherPhase.ready,
        generation: 1,
        snapshot: snapshot,
        scopeId: 'rust');
    expect(rust.visibleCandidates.map((c) => c.title),
        ['Cargo Run · server / main']);
  });

  Future<void> toggle(WidgetTester tester, {String mode = 'currentApp'}) async {
    final done = Completer<void>();
    messenger.handlePlatformMessage(channel.name,
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
    return launcherSnapshot(
      sessionId: 1,
      revision: revision,
      snapshotSeq: 1,
      input: input,
      contextComplete: true,
      contexts: [],
      contextCaptures: const [],
      candidates: candidates,
      roots: const [],
      diagnostics: [],
    );
  }

  testWidgets(
      'empty results collapse and candidate rows grow up to a scrollable maximum',
      (tester) async {
    await tester.pumpWidget(LauncherApp(
        window: channel,
        beginActivation: () => 1,
        activate: (_,
                {promptAccessibility = false,
                capturedEnvironment = false}) async =>
            sizedState(0, 0),
        updateInput: (_, input, revision,
                {scopeId, inputMode = InputModeDto.environment}) async =>
            sizedState(revision, int.parse(input), input: input),
        execute: (_, revision, candidate) async =>
            throw StateError('not invoked'),
        deactivate: (_) async {}));
    await toggle(tester);
    expect(find.byKey(const Key('action-list')), findsNothing);
    expect(find.byKey(const Key('empty-state')), findsNothing);
    final emptyHeight =
        tester.getSize(find.byKey(const Key('launcher-panel'))).height;
    expect(emptyHeight, LauncherViewState.inputHeight);
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
    expect(tester.getSize(find.byKey(const Key('launcher-panel'))).height,
        emptyHeight);
  });

  testWidgets('an empty search collapses to the search bar', (tester) async {
    await tester.pumpWidget(LauncherApp(
        window: channel,
        beginActivation: () => 1,
        activate: (_,
                {promptAccessibility = false,
                capturedEnvironment = false}) async =>
            sizedState(0, 0),
        updateInput: (_, input, revision,
                {scopeId, inputMode = InputModeDto.environment}) async =>
            sizedState(revision, 0),
        execute: (_, revision, candidate) async =>
            throw StateError('not invoked'),
        deactivate: (_) async {}));
    await toggle(tester);
    expect(find.text('No actions for this context'), findsNothing);
    expect(find.byKey(const Key('empty-state')), findsNothing);
    expect(tester.getSize(find.byKey(const Key('launcher-panel'))).height,
        LauncherViewState.inputHeight);
    await tester.enterText(find.byType(TextField), 'zzz');
    await tester.pumpAndSettle();
    expect(find.textContaining('zzz'), findsWidgets);
    expect(find.textContaining('match'), findsNothing);
    expect(tester.getSize(find.byKey(const Key('launcher-panel'))).height,
        LauncherViewState.inputHeight);
  });

  testWidgets('scope homes stay short and menu queries flatten the entire app',
      (tester) async {
    final sidebar = candidate('app.sidebar', 'Toggle Sidebar',
        groupId: 'app.editor', menuPath: ['View', 'Appearance']);
    final terminal = candidate('app.terminal', 'Toggle Terminal',
        groupId: 'app.editor', menuPath: ['View']);
    final menus = [
      for (var i = 0; i < 60; i++)
        candidate('app.export.$i', 'Export $i',
            groupId: 'app.editor', menuPath: ['File'], homeEligible: false)
    ];
    final git = candidate('git.status', 'Git Status', groupId: 'git');
    final copy = candidate('copy.path', 'Copy Path');
    final offers = [sidebar, terminal, ...menus, git, copy];
    final boundaries = <String?>[];
    var captures = 0;
    final executions = <String>[];
    LauncherStateDto result(int revision, String input, String? scope) =>
        launcherSnapshot(
            sessionId: 1,
            revision: revision,
            snapshotSeq: 1,
            input: input,
            contextComplete: true,
            contextCandidates: offers,
            selectedScope: scope,
            candidates: offers
                .where((item) =>
                    (scope == null ||
                        scope == 'app.editor' ||
                        item.groupId == scope) &&
                    input.toLowerCase().split(' ').every((word) =>
                        '${item.title} ${item.menuPath.join(' ')}'
                            .toLowerCase()
                            .contains(word)))
                .toList(),
            diagnostics: []);
    await tester.pumpWidget(LauncherApp(
        window: channel,
        beginActivation: () => 1,
        activate: (_,
            {promptAccessibility = false, capturedEnvironment = false}) async {
          captures++;
          return result(0, '', null);
        },
        updateInput: (_, input, revision,
            {scopeId, inputMode = InputModeDto.environment}) async {
          boundaries.add(scopeId);
          return result(revision, input, scopeId);
        },
        execute: (session, revision, id) async {
          executions.add(id);
          return ActionResultDto(
              sessionId: session,
              revision: revision,
              status: ActionStatusDto.succeeded,
              closeWindow: false);
        },
        deactivate: (_) async {}));
    await toggle(tester);
    expect(find.byKey(const Key('scope-bar')), findsOneWidget);
    expect(find.byKey(const Key('active-context')), findsNothing);
    expect(find.text('All'), findsNothing);
    expect(find.text('5 results'),
        findsOneWidget); // Menu and context recommendations plus Browse.
    expect(find.text('Git Status'), findsOneWidget);
    expect(find.text('Copy Path'), findsOneWidget);
    expect(find.text('Export 0'), findsNothing);
    await tester.tap(find.text('Browse Menus…'));
    await tester.pumpAndSettle();
    expect(find.text('File'), findsOneWidget);
    expect(find.text('View'), findsOneWidget);
    await tester.tap(find.text('View'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Appearance'));
    await tester.pumpAndSettle();
    expect(find.text('View › Appearance'), findsOneWidget);
    expect(executions, isEmpty);
    expect(boundaries, isEmpty);
    await tester.enterText(find.byType(TextField), 'File Export 59');
    await tester.pumpAndSettle();
    expect(find.text('Export 59'), findsOneWidget);
    expect(boundaries.last, 'app.editor');
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pumpAndSettle();
    expect(find.text('Toggle Sidebar'), findsOneWidget);
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pumpAndSettle();
    expect(executions, ['app.sidebar']);
    await tester.sendKeyEvent(LogicalKeyboardKey.escape); // View
    await tester.pumpAndSettle();
    await tester.sendKeyEvent(LogicalKeyboardKey.escape); // App Home
    await tester.pumpAndSettle();
    expect(find.text('Browse Menus…'), findsOneWidget);
    await tester.enterText(find.byType(TextField), 'git status');
    await tester.pumpAndSettle();
    expect(boundaries.last, 'app.editor');
    expect(find.text('Git Status'), findsOneWidget);
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pumpAndSettle();
    expect(executions, ['app.sidebar', 'git.status']);
    await tester.enterText(find.byType(TextField), 'copy path');
    await tester.pumpAndSettle();
    expect(find.text('Copy Path'), findsOneWidget);
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pumpAndSettle();
    expect(executions, ['app.sidebar', 'git.status', 'copy.path']);
    await tester.enterText(find.byType(TextField), 'absent');
    await tester.pumpAndSettle();
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pumpAndSettle();
    expect(boundaries.last,
        'app.editor'); // Enter cannot silently leave the scope.
    await tester.sendKeyDownEvent(LogicalKeyboardKey.controlLeft);
    await tester.sendKeyEvent(LogicalKeyboardKey.tab);
    await tester.sendKeyUpEvent(LogicalKeyboardKey.controlLeft);
    await tester.pumpAndSettle();
    expect(boundaries.last, 'git');
    expect(tester.widget<TextField>(find.byType(TextField)).controller!.text,
        'absent');
    await tester.sendKeyEvent(LogicalKeyboardKey.escape); // Query
    await tester.pumpAndSettle();
    expect(find.text('Git Status'), findsOneWidget);
    await tester.sendKeyEvent(LogicalKeyboardKey.escape); // Scope
    await tester.pumpAndSettle();
    expect(boundaries.last, isNull);
    expect(find.text('Copy Path'), findsNothing);
    expect(captures, 1);
    expect(tester.takeException(), isNull);
  });

  testWidgets('an unresolved context stays on the search bar', (tester) async {
    await tester.pumpWidget(LauncherApp(
        window: channel,
        beginActivation: () => 1,
        activate: (_,
                {promptAccessibility = false,
                capturedEnvironment = false}) async =>
            launcherSnapshot(
              sessionId: 1,
              revision: 0,
              snapshotSeq: 1,
              input: '',
              contextComplete: false,
              contexts: [],
              contextCaptures: const [],
              candidates: [],
              roots: const [],
              diagnostics: [],
            ),
        updateInput: (_, input, revision,
                {scopeId, inputMode = InputModeDto.environment}) async =>
            sizedState(revision, 0),
        execute: (_, revision, candidate) async =>
            throw StateError('not invoked'),
        deactivate: (_) async {}));
    await toggle(tester);
    expect(find.textContaining('Looking for actions'), findsNothing);
    expect(find.byKey(const Key('empty-state')), findsNothing);
  });

  testWidgets(
      'captured source and permission guidance appear without hiding actions',
      (tester) async {
    await tester.pumpWidget(LauncherApp(
      window: channel,
      beginActivation: () => 1,
      activate: (_,
              {promptAccessibility = false,
              capturedEnvironment = false}) async =>
          launcherSnapshot(
        sessionId: 1,
        revision: 0,
        snapshotSeq: 1,
        input: '',
        contextComplete: true,
        contexts: const [
          ContextDto(
            id: 'c1',
            key: 'c1',
            rootKey: 'c1',
            iconKey: 'folder',
            capabilities: [],
            candidateIds: ['action:first:c1'],
            typeId: 'core.url',
            label: 'URL',
            detail: 'https://example.org',
            source: ContextSourceDto.browserCurrentPage,
          ),
        ],
        contextCaptures: const [
          ContextCaptureDto(
            source: ContextSourceDto.browserCurrentPage,
            status: ContextCaptureStatusDto.captured,
            capturedAtMs: 1,
          ),
          ContextCaptureDto(
            source: ContextSourceDto.selectedText,
            status: ContextCaptureStatusDto.permissionDenied,
            capturedAtMs: 1,
          ),
        ],
        candidates: [candidate('action:first:c1', 'Open URL')],
        roots: const [],
        diagnostics: const [],
      ),
      updateInput: (_, input, revision,
              {scopeId, inputMode = InputModeDto.environment}) async =>
          state(revision, 'Open URL'),
      execute: (_, revision, candidate) async =>
          throw StateError('not invoked'),
      deactivate: (_) async {},
    ));
    await toggle(tester);
    expect(find.textContaining('Browser current page'), findsNothing);
    expect(find.textContaining('Allow Accessibility'), findsOneWidget);
    expect(find.text('Open URL'), findsOneWidget);
    expect(find.text('Open Settings'), findsOneWidget);
    await tester.tap(find.text('Open Settings'));
    await tester.pumpAndSettle();
    final request = windowMessages
        .lastWhere((call) => call.method == 'openPrivacySettings');
    expect(request.arguments, {'pane': 'accessibility'});
    expect(windowCalls.last, 'hide');
    expect(tester.takeException(), isNull);
  });

  testWidgets(
      'Settings shows last capture status and opens clipboard privacy settings',
      (tester) async {
    final controller = LauncherController(
      window: WindowHost(channel),
      backend: LauncherBackend(
        beginActivation: () => 1,
        activate: (_,
                {promptAccessibility = false,
                capturedEnvironment = false}) async =>
            launcherSnapshot(
                sessionId: 1,
                revision: 0,
                snapshotSeq: 1,
                input: '',
                contextComplete: true,
                contextCaptures: const [
              ContextCaptureDto(
                  source: ContextSourceDto.selectedText,
                  status: ContextCaptureStatusDto.unsupported,
                  capturedAtMs: 1),
              ContextCaptureDto(
                  source: ContextSourceDto.clipboard,
                  status: ContextCaptureStatusDto.permissionDenied,
                  capturedAtMs: 1),
            ],
                candidates: [],
                diagnostics: []),
        updateInput: (_, input, revision,
                {scopeId, inputMode = InputModeDto.environment}) async =>
            null,
        execute: (_, revision, id) async => throw StateError('not invoked'),
        deactivate: (_) async {},
      ),
    );
    await controller.activate();
    await controller.dismiss();
    await tester.pumpWidget(MaterialApp(
        home: Center(
            child: SizedBox(
                width: 648,
                height: 480,
                child: Material(
                    child: SettingsScreen(
                        controller: controller.settings,
                        onClose: controller.closeSettings))))));
    await tester.tap(find.byKey(const ValueKey('settings-context')));
    await tester.pumpAndSettle();
    await tester.scrollUntilVisible(find.text('Permission needed'), 160);
    await tester.pumpAndSettle();
    expect(find.text('Not supported by this app'), findsOneWidget);
    expect(find.text('Permission needed'), findsOneWidget);
    await tester.tap(find.text('Open Settings'));
    await tester.pumpAndSettle();
    expect(
        windowMessages
            .lastWhere((call) => call.method == 'openPrivacySettings')
            .arguments,
        {'pane': 'clipboard'});
    expect(tester.takeException(), isNull);
    await tester.pumpWidget(const SizedBox.shrink());
    controller.dispose();
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
        activate: (_,
                {promptAccessibility = false,
                capturedEnvironment = false}) async =>
            state(0, 'Capture ${++captures}'),
        updateInput: (_, input, revision,
                {scopeId, inputMode = InputModeDto.environment}) =>
            delayed.future,
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
        activate: (_,
                {promptAccessibility = false,
                capturedEnvironment = false}) async =>
            state(0, 'Capture ${++captures}'),
        updateInput: (_, input, revision,
                {scopeId, inputMode = InputModeDto.environment}) async =>
            state(revision, input, input: input),
        execute: (_, revision, candidate) => pending.future,
        deactivate: (_) async {}));
    await toggle(tester);
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await dismiss(tester, presentedGeneration());
    await toggle(tester);
    pending.complete(ActionResultDto(
        sessionId: 1,
        revision: 0,
        status: ActionStatusDto.succeeded,
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
      activate: (_,
              {promptAccessibility = false, capturedEnvironment = false}) =>
          captured.future,
      updateInput: (_, input, revision,
              {scopeId, inputMode = InputModeDto.environment}) async =>
          state(revision, input, input: input),
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
      activate: (_,
              {promptAccessibility = false,
              capturedEnvironment = false}) async =>
          state(0, 'Open Terminal Here'),
      updateInput: (_, input, revision,
              {scopeId, inputMode = InputModeDto.environment}) async =>
          state(revision, input, input: input),
      execute: (session, revision, candidate) async {
        executed.add(candidate);
        return ActionResultDto(
            sessionId: session,
            revision: revision,
            status: ActionStatusDto.succeeded,
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
    expect(deactivated, 0);
    expect(find.byKey(const Key('action-list')), findsNothing);
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
      activate: (_,
              {promptAccessibility = false,
              capturedEnvironment = false}) async =>
          state(0, 'Initial action'),
      updateInput: (_, input, revision,
              {scopeId, inputMode = InputModeDto.environment}) =>
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
      activate: (_,
              {promptAccessibility = false,
              capturedEnvironment = false}) async =>
          state(0, 'Initial action'),
      updateInput: (_, input, revision,
              {scopeId, inputMode = InputModeDto.environment}) async =>
          state(revision, input, input: input),
      execute: (_, revision, candidate) => result.future,
      deactivate: (_) async {},
    ));
    await toggle(tester);
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.enterText(find.byType(TextField), 'New input');
    result.complete(ActionResultDto(
        sessionId: 1,
        revision: 0,
        status: ActionStatusDto.succeeded,
        outputText: null,
        closeWindow: true));
    await tester.pumpAndSettle();
    expect(find.text('New input'), findsWidgets);
    expect(windowCalls, isNot(contains('hide')));
  });

  testWidgets('scope bar changes query boundary without another capture',
      (tester) async {
    final all =
        candidate('action:core.git.status:c1', 'Git Status', groupId: 'git');
    final folder =
        candidate('action:core.git.log:c1', 'Git Log', groupId: 'git');
    final appRow = candidate('action:core.directory.copy:c1', 'Copy Path');
    LauncherStateDto scoped(int revision) => launcherSnapshot(
          sessionId: 1,
          revision: revision,
          snapshotSeq: 1,
          input: '',
          contextComplete: true,
          contexts: [],
          contextCaptures: const [],
          candidates: [appRow, all, folder],
          roots: [
            contextFixture(
                id: 'app', label: 'Finder', candidateIds: [appRow.id]),
            contextFixture(
                id: 'git', label: 'Git', candidateIds: [all.id, folder.id]),
          ],
          diagnostics: [],
        );
    await tester.pumpWidget(LauncherApp(
      window: channel,
      beginActivation: () => 1,
      activate: (_,
              {promptAccessibility = false,
              capturedEnvironment = false}) async =>
          scoped(0),
      updateInput: (_, input, revision,
              {scopeId, inputMode = InputModeDto.environment}) async =>
          scoped(revision),
      execute: (_, revision, candidate) async =>
          throw StateError('not invoked'),
      deactivate: (_) async {},
    ));
    await toggle(tester);
    expect(find.byKey(const Key('scope-bar')), findsOneWidget);
    expect(find.text('All'), findsNothing);
    expect(find.text('Folder'), findsOneWidget);
    expect(find.text('Git'), findsOneWidget);
    expect(find.text('Copy Path'), findsOneWidget);
    expect(find.text('Git Status'), findsOneWidget);
    await tester.tap(find.text('Git').first);
    await tester.pumpAndSettle();
    expect(find.text('Copy Path'), findsNothing);
    expect(find.text('Git Status'), findsOneWidget);
    expect(find.text('Git Log'), findsOneWidget);
    expect(tester.widget<TextField>(find.byType(TextField)).focusNode!.hasFocus,
        isTrue);
    await tester.sendKeyDownEvent(LogicalKeyboardKey.controlLeft);
    await tester.sendKeyEvent(LogicalKeyboardKey.tab);
    await tester.sendKeyUpEvent(LogicalKeyboardKey.controlLeft);
    await tester.pumpAndSettle();
    expect(find.text('Copy Path'), findsOneWidget);
  });

  testWidgets(
      'manual selection survives late context and falls back when removed',
      (tester) async {
    final first = candidate('action:first:c1', 'First row');
    final second = candidate('action:second:c1', 'Second row');
    final third = candidate('action:third:c1', 'Third row');
    LauncherStateDto lateState({required bool keepSecond}) => launcherSnapshot(
          sessionId: 1,
          revision: 0,
          snapshotSeq: 2,
          input: '',
          contextComplete: true,
          contexts: [],
          contextCaptures: const [],
          candidates: [third, first, if (keepSecond) second],
          roots: const [],
          diagnostics: [],
        );
    final firstSnapshot = launcherSnapshot(
      sessionId: 1,
      revision: 0,
      snapshotSeq: 1,
      input: '',
      contextComplete: false,
      contexts: [],
      contextCaptures: const [],
      candidates: [first, second],
      roots: const [],
      diagnostics: [],
    );
    // Each wait gets its own completer: the listener re-arms after every wait, so handing
    // back an already-completed future would spin instead of waiting.
    final pending = <Completer<LauncherStateDto?>>[];
    await tester.pumpWidget(LauncherApp(
      window: channel,
      beginActivation: () => 1,
      activate: (_,
              {promptAccessibility = false,
              capturedEnvironment = false}) async =>
          firstSnapshot,
      updateInput: (_, input, revision,
              {scopeId, inputMode = InputModeDto.environment}) async =>
          state(revision, input, input: input),
      execute: (_, revision, candidate) async =>
          throw StateError('not invoked'),
      deactivate: (_) async {},
      waitForSnapshot: (_, after) {
        final completer = Completer<LauncherStateDto?>();
        pending.add(completer);
        return completer.future.then((state) => SnapshotWaitDto(
              kind: state == null
                  ? SnapshotWaitKindDto.timedOut
                  : SnapshotWaitKindDto.updated,
              state: state,
            ));
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
          launcherSnapshot(
            sessionId: 1,
            revision: 0,
            snapshotSeq: seq,
            input: 'chrome',
            contextComplete: complete,
            contexts: [],
            contextCaptures: const [],
            candidates: [candidate('action:first:c1', title)],
            roots: const [],
            diagnostics: [],
          );

      final pending = <Completer<LauncherStateDto?>>[];
      await tester.pumpWidget(LauncherApp(
        window: channel,
        beginActivation: () => 1,
        activate: (_,
                {promptAccessibility = false,
                capturedEnvironment = false}) async =>
            snapshot(1, 'Search the Web', complete: settledAtStart),
        updateInput: (_, input, revision,
                {scopeId, inputMode = InputModeDto.environment}) async =>
            state(revision, 'Search the Web', input: input),
        execute: (_, revision, candidate) async =>
            throw StateError('not invoked'),
        deactivate: (_) async {},
        waitForSnapshot: (_, after) {
          final completer = Completer<LauncherStateDto?>();
          pending.add(completer);
          return completer.future.then((state) => SnapshotWaitDto(
                kind: state == null
                    ? SnapshotWaitKindDto.timedOut
                    : SnapshotWaitKindDto.updated,
                state: state,
              ));
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

  testWidgets('a selected row stays selectable when late rows push it down',
      (tester) async {
    // A later update can rank other rows above the user's selection. The window
    // must keep that candidate selected, and Enter must still run *that* row.
    CandidateDto row(int index, {double score = 100}) =>
        candidate('action:command.$index:c1', 'Row $index', score: score);

    LauncherStateDto withRows(int snapshotSeq, List<CandidateDto> rows,
            {int revision = 1, String input = 'row'}) =>
        launcherSnapshot(
          sessionId: 1,
          revision: revision,
          snapshotSeq: snapshotSeq,
          input: input,
          contextComplete: true,
          contexts: [],
          contextCaptures: const [],
          candidates: rows,
          roots: const [],
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
      activate: (_,
              {promptAccessibility = false,
              capturedEnvironment = false}) async =>
          launcherSnapshot(
        sessionId: 1,
        revision: 0,
        snapshotSeq: 1,
        input: '',
        contextComplete: false,
        contexts: [],
        contextCaptures: const [],
        candidates: initial,
        roots: const [],
        diagnostics: [],
      ),
      updateInput: (_, input, revision,
              {scopeId, inputMode = InputModeDto.environment}) async =>
          withRows(1, initial, revision: revision, input: input),
      execute: (_, revision, candidate) async {
        executed = candidate;
        return ActionResultDto(
          sessionId: 1,
          revision: revision,
          status: ActionStatusDto.succeeded,
          closeWindow: false,
        );
      },
      deactivate: (_) async {},
      waitForSnapshot: (_, after) {
        final completer = Completer<LauncherStateDto?>();
        pending.add(completer);
        return completer.future.then((state) => SnapshotWaitDto(
              kind: state == null
                  ? SnapshotWaitKindDto.timedOut
                  : SnapshotWaitKindDto.updated,
              state: state,
            ));
      },
    ));
    await toggle(tester);
    expect(pending, hasLength(1), reason: 'one listener per visible session');
    await tester.enterText(find.byType(TextField), 'row');
    await tester.pumpAndSettle();

    pending[0].complete(withRows(2, initial));
    await tester.pumpAndSettle();
    for (var i = 0; i < 6; i++) {
      await tester.sendKeyEvent(LogicalKeyboardKey.arrowDown);
    }
    await tester.pumpAndSettle();
    expect(pending, hasLength(2),
        reason: 'the listener re-arms for later snapshots');

    pending[1].complete(withRows(3, pushed));
    await tester.pumpAndSettle();
    expect(find.text('Row 6'), findsOneWidget,
        reason: 'the selected row must stay on screen');

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
      activate: (_,
              {promptAccessibility = false,
              capturedEnvironment = false}) async =>
          launcherSnapshot(
        sessionId: 1,
        revision: 0,
        snapshotSeq: 1,
        input: '',
        contextComplete: false,
        contexts: [],
        contextCaptures: const [],
        candidates: [first, second],
        roots: const [],
        diagnostics: [],
      ),
      updateInput: (_, input, revision,
              {scopeId, inputMode = InputModeDto.environment}) async =>
          state(revision, input, input: input),
      execute: (_, revision, candidate) async =>
          throw StateError('not invoked'),
      deactivate: (_) async {},
      waitForSnapshot: (_, after) {
        final completer = Completer<LauncherStateDto?>();
        pending.add(completer);
        return completer.future.then((state) => SnapshotWaitDto(
              kind: state == null
                  ? SnapshotWaitKindDto.timedOut
                  : SnapshotWaitKindDto.updated,
              state: state,
            ));
      },
    ));
    await toggle(tester);
    await tester.sendKeyEvent(LogicalKeyboardKey.arrowDown);
    await tester.pumpAndSettle();
    pending[0].complete(launcherSnapshot(
      sessionId: 1,
      revision: 0,
      snapshotSeq: 2,
      input: '',
      contextComplete: true,
      contexts: [],
      contextCaptures: const [],
      candidates: [third],
      roots: const [],
      diagnostics: [],
    ));
    await tester.pumpAndSettle();
    expect(find.text('Third row'), findsOneWidget);
    expect(find.text('Second row'), findsNothing);
  });

  testWidgets('keyboard selection scrolls the last result above the footer',
      (tester) async {
    tester.view.physicalSize = const Size(648, 520);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    final actions = List.generate(
        15, (index) => candidate('action:$index', 'Action $index'));
    LauncherStateDto state(int revision, String input) => launcherSnapshot(
        sessionId: 1,
        revision: revision,
        snapshotSeq: revision + 1,
        input: input,
        contextComplete: true,
        candidates: actions,
        diagnostics: []);
    await tester.pumpWidget(LauncherApp(
        window: channel,
        beginActivation: () => 1,
        activate: (_,
                {promptAccessibility = false,
                capturedEnvironment = false}) async =>
            state(0, ''),
        updateInput: (_, input, revision,
                {scopeId, inputMode = InputModeDto.environment}) async =>
            state(revision, input),
        execute: (_, __, ___) async => throw StateError('not invoked'),
        deactivate: (_) async {}));
    await toggle(tester);
    await tester.enterText(find.byType(TextField), 'action');
    await tester.pumpAndSettle();
    for (var index = 0; index < 14; index++) {
      await tester.sendKeyEvent(LogicalKeyboardKey.arrowDown);
      await tester.pumpAndSettle();
    }
    final last = find.text('Action 14');
    expect(last.hitTestable(), findsOneWidget);
    expect(tester.getRect(last).bottom,
        lessThanOrEqualTo(tester.getRect(find.text('Run')).top));
    expect(find.text('15 results'), findsOneWidget);
    final panel = tester.getRect(find.byKey(const Key('launcher-panel')));
    final footerKeycap = tester.getRect(find.descendant(
        of: find.byKey(const Key('launcher-footer')),
        matching: find.byType(AppKeycap)));
    final runButton = tester.getRect(find.ancestor(
        of: find.text('Run'), matching: find.byType(OutlinedButton)));
    expect(panel.bottom - footerKeycap.bottom, greaterThanOrEqualTo(10));
    expect(panel.right - footerKeycap.right, greaterThanOrEqualTo(24));
    expect(panel.bottom - runButton.bottom, greaterThanOrEqualTo(10));
    expect(tester.takeException(), isNull);
  });

  testWidgets('application icons render when loaded and fall back when absent',
      (tester) async {
    final chrome = CandidateDto(
      contextKeys: const [],
      homeEligible: true,
      primaryAction: false,
      id: 'app:com.google.Chrome:/Applications/Google Chrome.app',
      kind: CandidateKindDto.application,
      title: 'Google Chrome',
      menuPath: const [],
      subtitle: '/Applications/Google Chrome.app',
      iconKey: 'application',
      score: 5000,
      groupId: 'app',
      availability: AvailabilityDto.enabled,
      availabilityReason: null,
      commandId: null,
      bundleId: 'com.google.Chrome',
    );
    final notes = CandidateDto(
      contextKeys: const [],
      homeEligible: true,
      primaryAction: false,
      id: 'app:com.apple.Notes:/System/Applications/Notes.app',
      kind: CandidateKindDto.application,
      title: 'Notes',
      menuPath: const [],
      subtitle: '/System/Applications/Notes.app',
      iconKey: 'application',
      score: 4000,
      groupId: 'app',
      availability: AvailabilityDto.enabled,
      availabilityReason: null,
      commandId: null,
      bundleId: 'com.apple.Notes',
    );
    LauncherStateDto state(int revision, String input) => launcherSnapshot(
          sessionId: 1,
          revision: revision,
          snapshotSeq: 1,
          input: input,
          contextComplete: true,
          contexts: [],
          contextCaptures: const [],
          candidates: [chrome, notes],
          roots: const [],
          diagnostics: [],
        );
    final requested = <String>[];
    await tester.pumpWidget(LauncherApp(
      window: channel,
      beginActivation: () => 1,
      activate: (_,
              {promptAccessibility = false,
              capturedEnvironment = false}) async =>
          state(0, ''),
      updateInput: (_, input, revision,
              {scopeId, inputMode = InputModeDto.environment}) async =>
          state(revision, input),
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
    expect(requested, isEmpty, reason: 'Apps are not on empty-query Home');
    await tester.enterText(find.byType(TextField), 'app');
    await tester.pumpAndSettle();
    expect(requested, contains(chrome.id));
    expect(requested, contains(notes.id));
    expect(
        find.descendant(
            of: find.byKey(const Key('action-list')),
            matching: find.byType(Image)),
        findsOneWidget);
    // The failing icon falls back to a glyph instead of breaking the row.
    expect(find.byIcon(AppIcons.apps), findsOneWidget);
    expect(find.text('Notes'), findsOneWidget);
  });

  testWidgets('an icon that fails once is retried on the next snapshot',
      (tester) async {
    CandidateDto app(String id, String title) => CandidateDto(
          contextKeys: const [],
          homeEligible: true,
          primaryAction: false,
          id: id,
          kind: CandidateKindDto.application,
          title: title,
          menuPath: const [],
          subtitle: '/Applications/$title.app',
          iconKey: 'application',
          score: 5000,
          groupId: 'app',
          availability: AvailabilityDto.enabled,
          availabilityReason: null,
          commandId: null,
          bundleId: id,
        );
    final edge = app('app:com.microsoft.edgemac', 'Microsoft Edge');
    // Sequence must advance, or the ordering guard correctly ignores the second
    // snapshot and this test would not exercise a retry at all.
    LauncherStateDto withEdge(int revision) => launcherSnapshot(
          sessionId: 1,
          revision: revision,
          snapshotSeq: revision + 1,
          input: 'edge',
          contextComplete: true,
          contexts: [],
          contextCaptures: const [],
          candidates: [edge],
          roots: const [],
          diagnostics: [],
        );
    var attempts = 0;
    await tester.pumpWidget(LauncherApp(
      window: channel,
      beginActivation: () => 1,
      activate: (_,
              {promptAccessibility = false,
              capturedEnvironment = false}) async =>
          withEdge(0),
      updateInput: (_, input, revision,
              {scopeId, inputMode = InputModeDto.environment}) async =>
          withEdge(revision),
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
    await tester.enterText(find.byType(TextField), 'edge');
    await tester.pumpAndSettle();
    expect(attempts, 1);
    expect(
        find.descendant(
            of: find.byKey(const Key('action-list')),
            matching: find.byType(Image)),
        findsNothing,
        reason: 'the first attempt failed');
    // A later snapshot must try again rather than cache the absence forever.
    await tester.enterText(find.byType(TextField), 'edge again');
    await tester.pumpAndSettle();
    expect(attempts, greaterThan(1));
    expect(
        find.descendant(
            of: find.byKey(const Key('action-list')),
            matching: find.byType(Image)),
        findsOneWidget);
  });

  testWidgets('disabled candidates show their reason and cannot execute',
      (tester) async {
    var executed = 0;
    await tester.pumpWidget(LauncherApp(
      window: channel,
      beginActivation: () => 1,
      activate: (_,
              {promptAccessibility = false,
              capturedEnvironment = false}) async =>
          launcherSnapshot(
        sessionId: 1,
        revision: 0,
        snapshotSeq: 1,
        input: '',
        contextComplete: true,
        contexts: [],
        contextCaptures: const [],
        candidates: [
          candidate('action:core.git.status:c1', 'Git Status',
              availability: AvailabilityDto.disabled,
              reason: 'git is not installed',
              iconKey: 'git'),
        ],
        roots: const [],
        diagnostics: [],
      ),
      updateInput: (_, input, revision,
              {scopeId, inputMode = InputModeDto.environment}) async =>
          state(revision, input, input: input),
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

  testWidgets('typing keeps the active Scope', (tester) async {
    final copy = candidate('action:core.directory.copy:c1', 'Copy Path',
        groupId: 'app.editor');
    final git =
        candidate('action:core.git.status:c1', 'Git Status', groupId: 'git');
    LauncherStateDto scoped(int revision, String input) => launcherSnapshot(
          sessionId: 1,
          revision: revision,
          snapshotSeq: 1,
          input: input,
          contextComplete: true,
          contexts: [],
          contextCaptures: const [],
          contextCandidates: [copy, git],
          candidates: [copy, git]
              .where((action) =>
                  action.title.toLowerCase().contains(input.toLowerCase()))
              .toList(),
          roots: [
            contextFixture(id: 'app', label: 'Finder', candidateIds: [copy.id]),
            contextFixture(id: 'git', label: 'Git', candidateIds: [git.id]),
          ],
          diagnostics: [],
        );
    await tester.pumpWidget(LauncherApp(
      window: channel,
      beginActivation: () => 1,
      activate: (_,
              {promptAccessibility = false,
              capturedEnvironment = false}) async =>
          scoped(0, ''),
      updateInput: (_, input, revision,
              {scopeId, inputMode = InputModeDto.environment}) async =>
          scoped(revision, input),
      execute: (_, __, ___) async => throw StateError('not invoked'),
      deactivate: (_) async {},
    ));
    await toggle(tester);
    expect(find.text('Copy Path'), findsOneWidget);
    expect(find.text('Git Status'), findsOneWidget);
    expect(find.text('Search Editor actions…'), findsOneWidget);
    await tester.enterText(find.byType(TextField), 'copy');
    await tester.pumpAndSettle();
    expect(find.text('Copy Path'), findsOneWidget);
    expect(find.text('Git Status'), findsNothing);
  });

  testWidgets('Esc clears query then Scope then closes the window',
      (tester) async {
    final copy = candidate('action:core.directory.copy:c1', 'Copy Path',
        groupId: 'app.editor');
    final git =
        candidate('action:core.git.status:c1', 'Git Status', groupId: 'git');
    LauncherStateDto scoped(int revision, String input) => launcherSnapshot(
          sessionId: 1,
          revision: revision,
          snapshotSeq: 1,
          input: input,
          contextComplete: true,
          contexts: [],
          contextCaptures: const [],
          contextCandidates: [copy, git],
          candidates: [copy, git]
              .where((action) =>
                  action.title.toLowerCase().contains(input.toLowerCase()))
              .toList(),
          roots: [
            contextFixture(id: 'app', label: 'Finder', candidateIds: [copy.id]),
            contextFixture(id: 'git', label: 'Git', candidateIds: [git.id]),
          ],
          diagnostics: [],
        );
    var deactivated = 0;
    await tester.pumpWidget(LauncherApp(
      window: channel,
      beginActivation: () => 1,
      activate: (_,
              {promptAccessibility = false,
              capturedEnvironment = false}) async =>
          scoped(0, ''),
      updateInput: (_, input, revision,
              {scopeId, inputMode = InputModeDto.environment}) async =>
          scoped(revision, input),
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
    expect(tester.widget<TextField>(find.byType(TextField)).controller!.text,
        isEmpty);
    expect(find.text('Copy Path'), findsOneWidget);
    expect(find.text('Git Status'), findsOneWidget);
    expect(find.text('Search Editor actions…'), findsOneWidget);
    expect(deactivated, 0);
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pumpAndSettle();
    expect(find.text('Git Status'), findsNothing);
    expect(find.text('Search for an action or app…'), findsOneWidget);
    expect(deactivated, 0);
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pumpAndSettle();
    expect(deactivated, 1);
    expect(windowCalls.last, 'hide');
  });

  testWidgets('another wake entry switches mode in place and keeps the query',
      (tester) async {
    final copy = candidate('action:core.directory.copy:c1', 'Copy Path',
        groupId: 'app.editor');
    final git =
        candidate('action:core.git.status:c1', 'Git Status', groupId: 'git');
    LauncherStateDto scoped(int revision, String input) => launcherSnapshot(
          sessionId: 1,
          revision: revision,
          snapshotSeq: 1,
          input: input,
          contextComplete: true,
          contexts: [],
          contextCaptures: const [],
          contextCandidates: [copy, git],
          candidates: [copy, git]
              .where((action) =>
                  action.title.toLowerCase().contains(input.toLowerCase()))
              .toList(),
          roots: [
            contextFixture(id: 'app', label: 'Finder', candidateIds: [copy.id]),
            contextFixture(id: 'git', label: 'Git', candidateIds: [git.id]),
          ],
          diagnostics: [],
        );
    await tester.pumpWidget(LauncherApp(
      window: channel,
      beginActivation: () => 1,
      activate: (_,
              {promptAccessibility = false,
              capturedEnvironment = false}) async =>
          scoped(0, ''),
      updateInput: (_, input, revision,
              {scopeId, inputMode = InputModeDto.environment}) async =>
          scoped(revision, input),
      execute: (_, __, ___) async => throw StateError('not invoked'),
      deactivate: (_) async {},
    ));
    await toggle(tester);
    await tester.enterText(find.byType(TextField), 'copy');
    await tester.pumpAndSettle();
    await toggle(tester, mode: 'universal');
    expect(tester.widget<TextField>(find.byType(TextField)).controller!.text,
        'copy');
    expect(find.text('Copy Path'), findsOneWidget);
    expect(find.text('Git Status'), findsNothing);
    expect(find.text('Search for an action or app…'), findsOneWidget);
    expect(windowCalls, isNot(contains('hide')));
    await toggle(tester, mode: 'universal');
    await tester.pumpAndSettle();
    expect(windowCalls.last, 'hide');
  });

  testWidgets(
      'URL actions show page or clipboard source and execute the selected target',
      (tester) async {
    const pageUrl = 'https://example.org/current';
    final pageCopy = candidate('action:core.url.copy:page', 'Copy URL',
        groupId: 'web', executionContextId: 'page', subtitle: pageUrl);
    final pageOpen = candidate(
        'action:core.url.open:page', 'Open in Default Browser',
        groupId: 'web', executionContextId: 'page', subtitle: pageUrl);
    final clipboardOpen = candidate(
        'action:core.url.open:clipboard', 'Open in Default Browser',
        groupId: 'web',
        executionContextId: 'clipboard',
        subtitle: 'Copied URL');
    // Rust omits redundant Copy URL for the clipboard's own value.
    final offers = [pageCopy, pageOpen, clipboardOpen];
    final contexts = [
      contextFixture(
          id: 'clipboard',
          label: 'Clipboard',
          typeId: 'core.url',
          source: ContextSourceDto.clipboard,
          detail: 'Copied URL',
          candidateIds: [clipboardOpen.id]),
      contextFixture(
          id: 'page',
          label: 'Current page',
          typeId: 'core.url',
          source: ContextSourceDto.browserCurrentPage,
          detail: pageUrl,
          candidateIds: [pageCopy.id, pageOpen.id]),
    ];
    final executed = <String>[];
    LauncherStateDto snapshot(int revision, String input) => launcherSnapshot(
            sessionId: 1,
            revision: revision,
            snapshotSeq: 1,
            input: input,
            contextComplete: true,
            contexts: contexts,
            contextCandidates: offers,
            candidates: offers
                .where((action) =>
                    action.title.toLowerCase().contains(input.toLowerCase()))
                .toList(),
            scopes: const [
              ScopeDto(
                  id: 'app.edge', label: 'Microsoft Edge', isCurrentApp: true),
              ScopeDto(id: 'web', label: 'Web', isCurrentApp: false),
            ],
            diagnostics: []);
    await tester.pumpWidget(LauncherApp(
        window: channel,
        beginActivation: () => 1,
        activate: (_,
                {promptAccessibility = false,
                capturedEnvironment = false}) async =>
            snapshot(0, ''),
        updateInput: (_, input, revision,
                {scopeId, inputMode = InputModeDto.environment}) async =>
            snapshot(revision, input),
        execute: (session, revision, id) async {
          executed.add(id);
          return ActionResultDto(
              sessionId: session,
              revision: revision,
              status: ActionStatusDto.succeeded,
              closeWindow: false);
        },
        deactivate: (_) async {}));
    await toggle(tester);
    expect(find.text('Copy URL'), findsOneWidget);
    expect(find.text('Open in Default Browser'), findsNWidgets(2));
    expect(find.text('Current page · $pageUrl'), findsNWidgets(2));
    expect(find.text('Clipboard · Copied URL'), findsOneWidget);
    await tester.tap(find.text('Clipboard · Copied URL'));
    await tester.pumpAndSettle();
    expect(executed, [clipboardOpen.id]);
    await tester.enterText(find.byType(TextField), 'copy');
    await tester.pumpAndSettle();
    expect(find.text('Current page · $pageUrl'), findsOneWidget);
    expect(find.text('Clipboard · Copied URL'), findsNothing);
    await tester.tap(find.text('Current page · $pageUrl'));
    await tester.pumpAndSettle();
    expect(executed, [clipboardOpen.id, pageCopy.id]);
    expect(tester.takeException(), isNull);
  });

  testWidgets('Current App with no menus shows and searches context actions',
      (tester) async {
    final copy = candidate('action:core.directory.copy:c1', 'Copy Path');
    final git =
        candidate('action:core.git.status:c1', 'Git Status', groupId: 'git');
    final hidden = candidate('action:hidden:c1', 'Git Hidden',
        groupId: 'git', availability: AvailabilityDto.hidden);
    final application = candidate('app:gittools', 'Git Tools',
        kind: CandidateKindDto.application, groupId: 'app');
    final offers = [copy, git, hidden, application];
    final requests = <String?>[];
    final executed = <String>[];
    LauncherStateDto snapshot(int revision, String input) => launcherSnapshot(
            sessionId: 1,
            revision: revision,
            snapshotSeq: 1,
            input: input,
            contextComplete: true,
            contextCandidates: offers,
            candidates: offers
                .where((action) =>
                    action.title.toLowerCase().contains(input.toLowerCase()))
                .toList(),
            scopes: const [
              ScopeDto(id: 'app.finder', label: 'Finder', isCurrentApp: true),
              ScopeDto(id: 'folder', label: 'Folder', isCurrentApp: false),
              ScopeDto(id: 'git', label: 'Git', isCurrentApp: false),
            ],
            diagnostics: []);
    await tester.pumpWidget(LauncherApp(
        window: channel,
        beginActivation: () => 1,
        activate: (_,
                {promptAccessibility = false,
                capturedEnvironment = false}) async =>
            snapshot(0, ''),
        updateInput: (_, input, revision,
            {scopeId, inputMode = InputModeDto.environment}) async {
          requests.add(scopeId);
          return snapshot(revision, input);
        },
        execute: (session, revision, id) async {
          executed.add(id);
          return ActionResultDto(
              sessionId: session,
              revision: revision,
              status: ActionStatusDto.succeeded,
              closeWindow: false);
        },
        deactivate: (_) async {}));
    await toggle(tester);
    expect(find.text('Search Finder actions…'), findsOneWidget);
    expect(find.text('Copy Path'), findsOneWidget);
    expect(find.text('Git Status'), findsOneWidget);
    expect(find.text('Browse Menus…'), findsNothing);
    expect(find.text('Git Hidden'), findsNothing);
    expect(find.text('Git Tools'), findsNothing);
    await tester.enterText(find.byType(TextField), 'git');
    await tester.pumpAndSettle();
    expect(requests.last, 'app.finder');
    expect(find.text('Git Status'), findsOneWidget);
    expect(find.text('Copy Path'), findsNothing);
    expect(find.text('Git Hidden'), findsNothing);
    expect(find.text('Git Tools'), findsNothing);
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pumpAndSettle();
    expect(executed, [git.id]);
    expect(tester.takeException(), isNull);
  });

  testWidgets('late Current App Scope is selected while idle', (tester) async {
    final git =
        candidate('action:core.git.status:c1', 'Git Status', groupId: 'git');
    final copy = candidate('action:core.directory.copy:c1', 'Copy Path',
        groupId: 'app.editor');
    final first = launcherSnapshot(
      sessionId: 1,
      revision: 0,
      snapshotSeq: 1,
      input: '',
      contextComplete: false,
      contexts: [],
      contextCaptures: const [],
      candidates: [git],
      roots: [
        contextFixture(id: 'git', label: 'Git', candidateIds: [git.id]),
      ],
      diagnostics: [],
    );
    final late = launcherSnapshot(
      sessionId: 1,
      revision: 0,
      snapshotSeq: 2,
      input: '',
      contextComplete: true,
      contexts: [],
      contextCaptures: const [],
      candidates: [copy, git],
      roots: [
        contextFixture(id: 'app', label: 'Finder', candidateIds: [copy.id]),
        contextFixture(id: 'git', label: 'Git', candidateIds: [git.id]),
      ],
      diagnostics: [],
    );
    final pending = <Completer<LauncherStateDto?>>[];
    await tester.pumpWidget(LauncherApp(
      window: channel,
      beginActivation: () => 1,
      activate: (_,
              {promptAccessibility = false,
              capturedEnvironment = false}) async =>
          first,
      updateInput: (_, input, revision,
              {scopeId, inputMode = InputModeDto.environment}) async =>
          state(revision, input, input: input),
      execute: (_, __, ___) async => throw StateError('not invoked'),
      deactivate: (_) async {},
      waitForSnapshot: (_, __) {
        final completer = Completer<LauncherStateDto?>();
        pending.add(completer);
        return completer.future.then((state) => SnapshotWaitDto(
              kind: state == null
                  ? SnapshotWaitKindDto.timedOut
                  : SnapshotWaitKindDto.updated,
              state: state,
            ));
      },
    ));
    await toggle(tester);
    expect(find.text('Git Status'), findsOneWidget);
    expect(find.text('Copy Path'), findsNothing);
    pending[0].complete(late);
    await tester.pumpAndSettle();
    expect(find.text('Copy Path'), findsOneWidget);
    expect(find.text('Git Status'), findsOneWidget);
    expect(find.text('Search Editor actions…'), findsOneWidget);
  });
}
