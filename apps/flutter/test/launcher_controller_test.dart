import 'dart:async';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:anywhere/infrastructure/application_icon_cache.dart';
import 'package:anywhere/infrastructure/window_host.dart';
import 'package:anywhere/launcher/launcher_controller.dart';
import 'package:anywhere/launcher/launcher_view_state.dart';
import 'package:anywhere/settings/host_settings.dart';
import 'package:anywhere/src/rust/api.dart';

LauncherStateDto snapshot(int session) => LauncherStateDto(
      sessionId: session,
      revision: 0,
      snapshotSeq: 1,
      input: '',
      contextComplete: true,
      contexts: [],
      contextCaptures: const [],
      candidates: [],
      scopes: [],
      diagnostics: [],
    );

CandidateDto appCandidate() => const CandidateDto(
      id: 'app:test',
      kind: CandidateKindDto.application,
      title: 'Test',
      menuPath: [],
      iconKey: 'application',
      score: 0,
      groupId: 'app',
      availability: AvailabilityDto.enabled,
    );

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  const channel = MethodChannel('test/controller-window');
  final messenger =
      TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger;
  setUp(() => messenger.setMockMethodCallHandler(channel, (_) async => null));
  tearDown(() => messenger.setMockMethodCallHandler(channel, null));

  test('older settings enable context sources and a saved toggle round-trips',
      () {
    final previous = HostSettings.fromMap(const {
      'currentAppKeyCode': HostSettings.spaceKey,
      'excludedApps': <String>[],
    });
    expect(previous.captureFinderSelection, isTrue);
    expect(previous.captureSelectedText, isTrue);
    expect(previous.captureBrowserCurrentPage, isTrue);
    expect(previous.captureClipboard, isTrue);
    final changed = previous.copyWith(captureClipboard: false);
    expect(HostSettings.fromMap(changed.toMap()).captureClipboard, isFalse);
  });

  LauncherController open({
    required BeginActivation beginActivation,
    required Activate activate,
    required UpdateInput updateInput,
    required Execute execute,
    required Future<void> Function(int) deactivate,
    WaitForSnapshot? waitForSnapshot,
    LoadIcon? loadIcon,
    PromptAccessibility? promptAccessibility,
    String startupMessage = '',
    ApplyExcludedApps? applyExcludedApps,
    Future<void> Function(HostSettings)? applyContextSources,
  }) =>
      LauncherController(
        backend: LauncherBackend(
          beginActivation: beginActivation,
          activate: activate,
          updateInput: updateInput,
          execute: execute,
          deactivate: deactivate,
          waitForSnapshot: waitForSnapshot,
          loadIcon: loadIcon,
          promptAccessibility: promptAccessibility,
        ),
        window: WindowHost(channel),
        startupMessage: startupMessage,
        applyExcludedApps: applyExcludedApps,
        applyContextSources: applyContextSources,
      );

  test('host warning survives unrelated saves and clears when host recovers',
      () async {
    var warning = 'Current App shortcut is unavailable';
    final appliedSources = <bool>[];
    messenger.setMockMethodCallHandler(channel, (call) async {
      switch (call.method) {
        case 'settings':
          return HostSettings.defaults.toMap();
        case 'settingsStatus':
          return {'hotkeyError': warning, 'loginError': ''};
        default:
          return null;
      }
    });
    final controller = open(
      beginActivation: () => 1,
      activate: (session, {promptAccessibility = false}) async =>
          snapshot(session),
      updateInput: (_, __, ___) async => null,
      execute: (_, __, ___) async => throw StateError('not used'),
      deactivate: (_) async {},
      startupMessage: warning,
      applyExcludedApps: (_) async {},
      applyContextSources: (settings) async {
        appliedSources.add(settings.captureClipboard);
      },
    );

    expect(await controller.saveSettings(HostSettings.defaults), isTrue);
    expect(appliedSources, [true]);
    expect(controller.statusMessage, warning);
    await controller.activate();
    expect(controller.view.message, warning);
    await controller.dismiss();

    warning = '';
    expect(await controller.saveSettings(HostSettings.defaults), isTrue);
    expect(appliedSources, [true, true]);
    expect(controller.statusMessage, isNull);
    await controller.activate();
    expect(controller.view.message, isNull);
    controller.dispose();
  });

  test(
    'dismiss cancels a reserved activation before its first snapshot',
    () async {
      final oldCapture = Completer<LauncherStateDto?>();
      var nextSession = 0;
      final cancelled = <int>[];
      final controller = open(
        beginActivation: () => ++nextSession,
        activate: (session, {promptAccessibility = false}) =>
            session == 1 ? oldCapture.future : Future.value(snapshot(session)),
        updateInput: (_, __, ___) async => null,
        execute: (_, __, ___) async => throw StateError('not used'),
        deactivate: (session) async {
          cancelled.add(session);
        },
      );
      final first = controller.activate();
      expect(controller.view.phase, LauncherPhase.activating);
      await controller.dismiss();
      expect(cancelled, [1]);
      await controller.activate();
      expect(controller.view.snapshot?.sessionId, 2);
      oldCapture.complete(snapshot(1));
      await first;
      expect(controller.view.snapshot?.sessionId, 2);
      expect(
          cancelled,
          [
            1,
          ],
          reason: 'an old completion must never cancel the new session');
      controller.dispose();
    },
  );

  test('a disposed controller cancels a pending activation', () async {
    final capture = Completer<LauncherStateDto?>();
    final cancelled = <int>[];
    final controller = open(
      beginActivation: () => 7,
      activate: (_, {promptAccessibility = false}) => capture.future,
      updateInput: (_, __, ___) async => null,
      execute: (_, __, ___) async => throw StateError('not used'),
      deactivate: (session) async {
        cancelled.add(session);
      },
    );
    final activation = controller.activate();
    controller.dispose();
    expect(cancelled, [7]);
    capture.complete(snapshot(7));
    await activation;
  });

  test(
    'an old icon response cannot refill a cleared session or remove its new request',
    () async {
      final old = Completer<Uint8List?>(), current = Completer<Uint8List?>();
      var calls = 0;
      final cache = ApplicationIconCache(
        (_) => ++calls == 1 ? old.future : current.future,
      );
      cache.request([appCandidate()]);
      cache.clear();
      cache.request([appCandidate()]);
      old.complete(Uint8List.fromList([1]));
      await Future<void>.delayed(Duration.zero);
      expect(cache.bytesFor('app:test'), isNull);
      cache.request([appCandidate()]);
      expect(
        calls,
        2,
        reason: 'the old response must not remove the new in-flight request',
      );
      current.complete(Uint8List.fromList([2]));
      await Future<void>.delayed(Duration.zero);
      expect(cache.bytesFor('app:test'), [2]);
      cache.dispose();
    },
  );

  CandidateDto action(String id, String title) => CandidateDto(
        id: id,
        kind: CandidateKindDto.action,
        title: title,
        menuPath: const [],
        iconKey: 'file',
        score: 1,
        groupId: 'folder',
        availability: AvailabilityDto.enabled,
      );

  LauncherStateDto scoped({
    int revision = 0,
    int snapshotSeq = 1,
    String input = '',
    required List<CandidateDto> candidates,
    required List<ScopeDto> scopes,
  }) =>
      LauncherStateDto(
        sessionId: 1,
        revision: revision,
        snapshotSeq: snapshotSeq,
        input: input,
        contextComplete: true,
        contexts: [],
        contextCaptures: const [],
        candidates: candidates,
        scopes: scopes,
        diagnostics: [],
      );

  test('current app launcher selects the app scope', () async {
    final copy = action('action:copy', 'Copy Path');
    final git = action('action:git', 'Git Status');
    final controller = open(
      beginActivation: () => 1,
      activate: (_, {promptAccessibility = false}) async => scoped(
        candidates: [copy, git],
        scopes: [
          ScopeDto(
              id: 'app',
              kind: ScopeKindDto.app,
              label: 'Finder',
              candidateIds: [copy.id]),
          ScopeDto(
              id: 'git',
              kind: ScopeKindDto.group,
              label: 'Git',
              candidateIds: [git.id]),
        ],
      ),
      updateInput: (_, __, ___) async => null,
      execute: (_, __, ___) async => throw StateError('not used'),
      deactivate: (_) async {},
    );
    await controller.activate();
    expect(controller.view.scopeId, 'app');
    expect(
        controller.view.visibleCandidates.map((c) => c.title), ['Copy Path']);
    controller.dispose();
  });

  test('update keeps the selected scope', () async {
    final copy = action('action:copy', 'Copy Path');
    final git = action('action:git', 'Git Status');
    final scopes = [
      ScopeDto(
          id: 'app',
          kind: ScopeKindDto.app,
          label: 'Finder',
          candidateIds: [copy.id]),
      ScopeDto(
          id: 'git',
          kind: ScopeKindDto.group,
          label: 'Git',
          candidateIds: [git.id]),
    ];
    final controller = open(
      beginActivation: () => 1,
      activate: (_, {promptAccessibility = false}) async =>
          scoped(candidates: [copy, git], scopes: scopes),
      updateInput: (_, input, revision) async => scoped(
        revision: revision,
        input: input,
        candidates: [copy, git],
        scopes: scopes,
      ),
      execute: (_, __, ___) async => throw StateError('not used'),
      deactivate: (_) async {},
    );
    await controller.activate();
    await controller.update('copy');
    expect(controller.view.scopeId, 'app');
    expect(
        controller.view.visibleCandidates.map((c) => c.title), ['Copy Path']);
    controller.dispose();
  });

  test('escape clears query then scope then dismisses', () async {
    final copy = action('action:copy', 'Copy Path');
    final git = action('action:git', 'Git Status');
    final scopes = [
      ScopeDto(
          id: 'app',
          kind: ScopeKindDto.app,
          label: 'Finder',
          candidateIds: [copy.id]),
      ScopeDto(
          id: 'git',
          kind: ScopeKindDto.group,
          label: 'Git',
          candidateIds: [git.id]),
    ];
    final controller = open(
      beginActivation: () => 1,
      activate: (_, {promptAccessibility = false}) async =>
          scoped(candidates: [copy, git], scopes: scopes),
      updateInput: (_, input, revision) async => scoped(
        revision: revision,
        input: input,
        candidates: [copy, git],
        scopes: scopes,
      ),
      execute: (_, __, ___) async => throw StateError('not used'),
      deactivate: (_) async {},
    );
    await controller.activate();
    await controller.update('copy');
    await controller.escape();
    expect(controller.view.input, isEmpty);
    expect(controller.view.scopeId, 'app');
    await controller.escape();
    expect(controller.view.scopeId, isNull);
    expect(controller.view.visibleCandidates.map((c) => c.title),
        ['Copy Path', 'Git Status']);
    await controller.escape();
    expect(controller.view.phase, LauncherPhase.hidden);
    controller.dispose();
  });

  test('late current app scope is selected while idle', () async {
    final git = action('action:git', 'Git Status');
    final copy = action('action:copy', 'Copy Path');
    final pending = <Completer<LauncherStateDto?>>[];
    final controller = open(
      beginActivation: () => 1,
      activate: (_, {promptAccessibility = false}) async => scoped(
        snapshotSeq: 1,
        candidates: [git],
        scopes: [
          ScopeDto(
              id: 'git',
              kind: ScopeKindDto.group,
              label: 'Git',
              candidateIds: [git.id]),
        ],
      ),
      updateInput: (_, __, ___) async => null,
      execute: (_, __, ___) async => throw StateError('not used'),
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
    );
    await controller.activate();
    expect(controller.view.scopeId, isNull);
    pending[0].complete(scoped(
      snapshotSeq: 2,
      candidates: [copy, git],
      scopes: [
        ScopeDto(
            id: 'app',
            kind: ScopeKindDto.app,
            label: 'Finder',
            candidateIds: [copy.id]),
        ScopeDto(
            id: 'git',
            kind: ScopeKindDto.group,
            label: 'Git',
            candidateIds: [git.id]),
      ],
    ));
    await Future<void>.delayed(Duration.zero);
    expect(controller.view.scopeId, 'app');
    expect(
        controller.view.visibleCandidates.map((c) => c.title), ['Copy Path']);
    controller.dispose();
  });

  test('removing the current app scope shows the remaining actions', () async {
    final menu = action('action:about', 'About');
    final git = action('action:git', 'Git Status');
    final pending = <Completer<LauncherStateDto?>>[];
    final controller = open(
      beginActivation: () => 1,
      activate: (_, {promptAccessibility = false}) async => scoped(
        candidates: [menu, git],
        scopes: [
          ScopeDto(
              id: 'app',
              kind: ScopeKindDto.app,
              label: 'Editor',
              candidateIds: [menu.id]),
          ScopeDto(
              id: 'git',
              kind: ScopeKindDto.group,
              label: 'Git',
              candidateIds: [git.id]),
        ],
      ),
      updateInput: (_, __, ___) async => null,
      execute: (_, __, ___) async => throw StateError('not used'),
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
    );
    await controller.activate();
    expect(controller.view.scopeId, 'app');
    expect(controller.view.visibleCandidates.map((c) => c.title), ['About']);

    pending[0].complete(scoped(
      snapshotSeq: 2,
      candidates: [git],
      scopes: [
        ScopeDto(
            id: 'git',
            kind: ScopeKindDto.group,
            label: 'Git',
            candidateIds: [git.id]),
      ],
    ));
    await Future<void>.delayed(Duration.zero);
    expect(controller.view.scopeId, isNull);
    expect(
        controller.view.visibleCandidates.map((c) => c.title), ['Git Status']);
    controller.dispose();
  });

  test('current app activate asks for accessibility after the window shows',
      () async {
    var activatePrompted = true;
    var prompts = 0;
    final controller = open(
      beginActivation: () => 1,
      activate: (_, {promptAccessibility = false}) async {
        activatePrompted = promptAccessibility;
        return snapshot(1);
      },
      updateInput: (_, __, ___) async => null,
      execute: (_, __, ___) async => throw StateError('not used'),
      deactivate: (_) async {},
      promptAccessibility: () {
        prompts += 1;
        return true;
      },
    );
    await controller.activate();
    expect(activatePrompted, isFalse);
    expect(prompts, 1);
    controller.dispose();
  });

  test('universal activate does not ask for accessibility', () async {
    var activatePrompted = true;
    var prompts = 0;
    final controller = open(
      beginActivation: () => 1,
      activate: (_, {promptAccessibility = false}) async {
        activatePrompted = promptAccessibility;
        return snapshot(1);
      },
      updateInput: (_, __, ___) async => null,
      execute: (_, __, ___) async => throw StateError('not used'),
      deactivate: (_) async {},
      promptAccessibility: () {
        prompts += 1;
        return true;
      },
    );
    await controller.activate(mode: WakeMode.universal);
    expect(activatePrompted, isFalse);
    expect(prompts, 0);
    controller.dispose();
  });

  test('switching to current app prompts accessibility', () async {
    var prompts = 0;
    final controller = open(
      beginActivation: () => 1,
      activate: (_, {promptAccessibility = false}) async => snapshot(1),
      updateInput: (_, __, ___) async => null,
      execute: (_, __, ___) async => throw StateError('not used'),
      deactivate: (_) async {},
      promptAccessibility: () {
        prompts += 1;
        return true;
      },
    );
    await controller.activate(mode: WakeMode.universal);
    expect(prompts, 0);
    await controller.toggle('currentApp');
    expect(prompts, 1);
    expect(controller.view.phase, isNot(LauncherPhase.hidden));
    controller.dispose();
  });
}
