import 'dart:async';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:anywhere/infrastructure/application_icon_cache.dart';
import 'package:anywhere/infrastructure/window_host.dart';
import 'package:anywhere/launcher/launcher_controller.dart';
import 'package:anywhere/launcher/launcher_view_state.dart';
import 'package:anywhere/settings/host_settings.dart';
import 'package:anywhere/src/rust/api.dart';
import 'launcher_fixtures.dart';

LauncherStateDto snapshot(int session) => launcherSnapshot(
      sessionId: session,
      revision: 0,
      snapshotSeq: 1,
      input: '',
      contextComplete: true,
      contexts: [],
      contextCaptures: const [],
      candidates: [],
      diagnostics: [],
    );

CandidateDto appCandidate() => const CandidateDto(
      contextKeys: [],
      homeEligible: true,
      primaryAction: false,
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

  test('capture status survives dismissal without retaining context contents',
      () async {
    final controller = open(
      beginActivation: () => 1,
      activate: (_, {promptAccessibility = false}) async => launcherSnapshot(
          sessionId: 1,
          revision: 0,
          snapshotSeq: 1,
          input: '',
          contextComplete: true,
          contextCaptures: const [
            ContextCaptureDto(
                source: ContextSourceDto.clipboard,
                status: ContextCaptureStatusDto.permissionDenied,
                capturedAtMs: 1)
          ],
          candidates: [],
          diagnostics: []),
      updateInput: (_, input, revision, {scopeId}) async => null,
      execute: (_, revision, candidate) async => throw StateError('not used'),
      deactivate: (_) async {},
    );
    await controller.activate();
    await controller.dismiss();
    expect(controller.view.snapshot, isNull);
    final clipboard = controller.contextSourceStatuses
        .firstWhere((item) => item.source == ContextSourceDto.clipboard);
    expect(clipboard.statusText, 'Permission needed');
    expect(clipboard.settingsPane, 'clipboard');
    controller.dispose();
  });

  test('failure to open privacy settings leaves the current actions usable',
      () async {
    messenger.setMockMethodCallHandler(channel, (call) async {
      if (call.method == 'openPrivacySettings') {
        throw PlatformException(code: 'privacy');
      }
      return null;
    });
    final controller = open(
      beginActivation: () => 1,
      activate: (_, {promptAccessibility = false}) async => snapshot(1),
      updateInput: (_, input, revision, {scopeId}) async => null,
      execute: (_, revision, candidate) async => throw StateError('not used'),
      deactivate: (_) async {},
    );
    await controller.activate();
    await controller.openPrivacySettings('clipboard');
    expect(controller.view.phase, LauncherPhase.ready);
    expect(controller.view.notice, contains('could not be opened'));
    expect(controller.view.snapshot, isNotNull);
    controller.dispose();
  });

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
      updateInput: (_, __, ___, {scopeId}) async => null,
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
        updateInput: (_, __, ___, {scopeId}) async => null,
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
      updateInput: (_, __, ___, {scopeId}) async => null,
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
      updateInput: (_, __, ___, {scopeId}) async => null,
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
      updateInput: (_, __, ___, {scopeId}) async => null,
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
      updateInput: (_, __, ___, {scopeId}) async => null,
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

  CandidateDto action(String id, String title, String group,
          {List<String> path = const [],
          bool recommended = true,
          AvailabilityDto availability = AvailabilityDto.enabled}) =>
      CandidateDto(
          id: id,
          title: title,
          kind: CandidateKindDto.action,
          groupId: group,
          menuPath: path,
          contextKeys: const [],
          homeEligible: recommended,
          primaryAction: false,
          iconKey: 'file',
          score: 100,
          availability: availability);
  const appScope =
      ScopeDto(id: 'app.editor', label: 'Editor', isCurrentApp: true);
  const gitScope = ScopeDto(id: 'git', label: 'Git', isCurrentApp: false);
  const folderScope =
      ScopeDto(id: 'folder', label: 'Folder', isCurrentApp: false);
  LauncherStateDto captured(
          {int revision = 0,
          int seq = 1,
          String input = '',
          required List<CandidateDto> offers,
          List<CandidateDto>? results,
          List<ScopeDto> scopes = const [appScope, gitScope, folderScope],
          String? selectedScope}) =>
      launcherSnapshot(
          sessionId: 1,
          revision: revision,
          snapshotSeq: seq,
          input: input,
          contextComplete: true,
          scopes: scopes,
          contextCandidates: offers,
          candidates: results ?? offers,
          diagnostics: [],
          selectedScope: selectedScope);

  test('Current App aggregates actions and domain cycling preserves the query',
      () async {
    final menu =
        action('app.terminal', 'Toggle Terminal', 'app.editor', path: ['View']);
    final git = action('git', 'Git Status', 'git');
    final copy = action('copy', 'Copy Path', 'folder');
    final offers = [menu, git, copy];
    final requests = <String?>[];
    final controller = open(
        beginActivation: () => 1,
        activate: (_, {promptAccessibility = false}) async =>
            captured(offers: offers),
        updateInput: (_, input, revision, {scopeId}) async {
          requests.add(scopeId);
          return captured(
              revision: revision,
              input: input,
              offers: offers,
              selectedScope: scopeId,
              results: offers
                  .where((offer) =>
                      scopeId == null ||
                      scopeId == appScope.id ||
                      offer.groupId == scopeId)
                  .toList());
        },
        execute: (_, __, ___) async => throw StateError('unused'),
        deactivate: (_) async {});
    await controller.activate();
    expect(controller.view.scopeId, 'app.editor');
    expect(controller.view.visibleCandidates.map((candidate) => candidate.id),
        ['app.terminal', 'git', 'copy']);
    expect(controller.view.visibleEntries.last.title, 'Browse Menus…');
    await controller.update('term');
    expect(requests.last, 'app.editor');
    expect(controller.view.visibleCandidates.map((candidate) => candidate.id),
        ['app.terminal', 'git', 'copy']);
    await controller.selectScope('git');
    expect(controller.view.input, 'term');
    expect(controller.view.visibleCandidates.single.id, 'git');
    controller.cycleScope(1);
    await Future<void>.delayed(Duration.zero);
    expect(controller.view.scopeId, 'folder');
    controller.cycleScope(1);
    await Future<void>.delayed(Duration.zero);
    expect(controller.view.scopeId, 'app.editor');
    await controller.toggle('universal');
    await Future<void>.delayed(Duration.zero);
    expect(controller.view.scopeId, isNull);
    expect(controller.view.input, 'term');
    expect(requests.last, isNull);
    controller.dispose();
  });

  test('browse preserves its path while queries search the entire app group',
      () async {
    final sidebar = action('app.sidebar', 'Toggle Sidebar', 'app.editor',
        path: ['View', 'Appearance']);
    final export = action('app.export', 'Export', 'app.editor',
        path: ['File'], recommended: false);
    final offers = [sidebar, export];
    var queries = 0;
    final executed = <String>[];
    final controller = open(
        beginActivation: () => 1,
        activate: (_, {promptAccessibility = false}) async =>
            captured(offers: offers),
        updateInput: (_, input, revision, {scopeId}) async {
          queries++;
          return captured(
              revision: revision,
              input: input,
              offers: offers,
              results: input.isEmpty ? offers : [export]);
        },
        execute: (session, revision, id) async {
          executed.add(id);
          return ActionResultDto(
              sessionId: session,
              revision: revision,
              status: ActionStatusDto.succeeded,
              closeWindow: false);
        },
        deactivate: (_) async {});
    await controller.activate();
    await controller.selectEntry(controller.view.visibleEntries
        .indexWhere((e) => e.title == 'Browse Menus…'));
    await controller.selectEntry(
        controller.view.visibleEntries.indexWhere((e) => e.title == 'View'));
    await controller.selectEntry(0); // Appearance
    expect(controller.view.actionPath, ['View', 'Appearance']);
    expect(queries, 0);
    expect(executed, isEmpty);
    await controller.update('export');
    expect(controller.view.visibleCandidates.single.id, 'app.export');
    expect(controller.view.actionPath, ['View', 'Appearance']);
    await controller.escape();
    expect(controller.view.input, isEmpty);
    expect(controller.view.visibleCandidates.single.id, 'app.sidebar');
    await controller.executeSelected();
    expect(executed, ['app.sidebar']);
    await controller.escape();
    expect(controller.view.actionPath, ['View']);
    await controller.escape();
    expect(controller.view.actionPath, isNull);
    expect(controller.view.scopeId, 'app.editor');
    await controller.escape();
    expect(controller.view.scopeId, isNull);
    await controller.escape();
    expect(controller.view.phase, LauncherPhase.hidden);
    controller.dispose();
  });

  test('empty scoped results do not implicitly leave Scope on Enter', () async {
    final copy = action('copy', 'Copy Path', 'folder');
    final controller = open(
        beginActivation: () => 1,
        activate: (_, {promptAccessibility = false}) async =>
            captured(offers: [copy], scopes: [folderScope]),
        updateInput: (_, input, revision, {scopeId}) async => captured(
            revision: revision,
            input: input,
            offers: [copy],
            scopes: [folderScope],
            results: input.isEmpty ? [copy] : []),
        execute: (_, __, ___) async => throw StateError('unused'),
        deactivate: (_) async {});
    await controller.activate();
    expect(controller.view.scopeId, isNull, reason: 'No Current App actions');
    await controller.selectScope('folder');
    await controller.update('absent');
    await controller.executeSelected();
    expect(controller.view.scopeId, 'folder');
    await controller.escape();
    expect(controller.view.scopeId, 'folder');
    expect(controller.view.input, isEmpty);
    await controller.escape();
    expect(controller.view.scopeId, isNull);
    await controller.escape();
    expect(controller.view.phase, LauncherPhase.hidden);
    controller.dispose();
  });

  test('late app scope may become default only before user interaction',
      () async {
    final copy = action('copy', 'Copy Path', 'folder');
    final menu =
        action('app.menu', 'Toggle Terminal', 'app.editor', path: ['View']);
    final queue = <Completer<SnapshotWaitDto>>[];
    final controller = open(
        beginActivation: () => 1,
        activate: (_, {promptAccessibility = false}) async =>
            captured(offers: [copy], scopes: [folderScope]),
        updateInput: (_, input, revision, {scopeId}) async => captured(
            revision: revision,
            input: input,
            offers: [copy, menu],
            scopes: [folderScope, appScope]),
        execute: (_, __, ___) async => throw StateError('unused'),
        deactivate: (_) async {},
        waitForSnapshot: (_, __) {
          final pending = Completer<SnapshotWaitDto>();
          queue.add(pending);
          return pending.future;
        });
    await controller.activate();
    expect(controller.view.scopeId, isNull);
    queue.last.complete(SnapshotWaitDto(
        kind: SnapshotWaitKindDto.updated,
        state: captured(
            seq: 2, offers: [copy, menu], scopes: [folderScope, appScope])));
    await Future<void>.delayed(Duration.zero);
    expect(controller.view.scopeId, 'app.editor');
    await controller.selectScope(null);
    queue.last.complete(SnapshotWaitDto(
        kind: SnapshotWaitKindDto.updated,
        state: captured(
            revision: 1,
            seq: 2,
            offers: [copy, menu],
            scopes: [folderScope, appScope])));
    await Future<void>.delayed(Duration.zero);
    expect(controller.view.scopeId, isNull);
    controller.dispose();
    queue.last
        .complete(const SnapshotWaitDto(kind: SnapshotWaitKindDto.closed));
  });

  test('losing menus returns to Current App home while domain actions remain',
      () async {
    final copy = action('copy', 'Copy Path', 'folder');
    final menu = action('app.menu', 'Menu', 'app.editor', path: ['View']);
    final pending = <Completer<SnapshotWaitDto>>[];
    final controller = open(
        beginActivation: () => 1,
        activate: (_, {promptAccessibility = false}) async =>
            captured(offers: [copy, menu]),
        updateInput: (_, input, revision, {scopeId}) async =>
            captured(revision: revision, input: input, offers: [copy, menu]),
        execute: (_, __, ___) async => throw StateError('unused'),
        deactivate: (_) async {},
        waitForSnapshot: (_, __) {
          final next = Completer<SnapshotWaitDto>();
          pending.add(next);
          return next.future;
        });
    await controller.activate();
    await controller.browseMenus();
    expect(controller.view.browsing, isTrue);
    pending.first.complete(SnapshotWaitDto(
        kind: SnapshotWaitKindDto.updated,
        state:
            captured(seq: 2, offers: [copy], scopes: [appScope, folderScope])));
    await Future<void>.delayed(Duration.zero);
    expect(controller.view.scopeId, 'app.editor');
    expect(controller.view.actionPath, isNull);
    expect(controller.view.visibleCandidates.single.id, 'copy');
    expect(controller.view.canBrowseMenus, isFalse);
    controller.dispose();
  });

  test('removing an active scope exits browsing without adopting another scope',
      () async {
    final copy = action('copy', 'Copy Path', 'folder');
    final menu = action('app.menu', 'Menu', 'app.editor', path: ['View']);
    final pending = Completer<SnapshotWaitDto>();
    final controller = open(
        beginActivation: () => 1,
        activate: (_, {promptAccessibility = false}) async =>
            captured(offers: [copy, menu]),
        updateInput: (_, input, revision, {scopeId}) async =>
            captured(revision: revision, input: input, offers: [copy]),
        execute: (_, __, ___) async => throw StateError('unused'),
        deactivate: (_) async {},
        waitForSnapshot: (_, __) => pending.isCompleted
            ? Future.value(
                const SnapshotWaitDto(kind: SnapshotWaitKindDto.closed))
            : pending.future);
    await controller.activate();
    await controller.browseMenus();
    pending.complete(SnapshotWaitDto(
        kind: SnapshotWaitKindDto.updated,
        state: captured(seq: 2, offers: [copy], scopes: [folderScope])));
    await Future<void>.delayed(Duration.zero);
    expect(controller.view.scopeId, isNull);
    expect(controller.view.actionPath, isNull);
    expect(controller.view.visibleCandidates.single.id, 'copy');
    controller.dispose();
  });
}
