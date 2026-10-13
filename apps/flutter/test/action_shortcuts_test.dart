import 'package:flutter/foundation.dart';
import 'package:anywhere/infrastructure/desktop_environment.dart';
import 'dart:async';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:anywhere/infrastructure/window_host.dart';
import 'package:anywhere/launcher/launcher_controller.dart';
import 'package:anywhere/launcher/launcher_screen.dart';
import 'package:anywhere/ui/app_style.dart';
import 'package:anywhere/launcher/launcher_view_state.dart';
import 'package:anywhere/settings/action_shortcut.dart';
import 'package:anywhere/settings/host_settings.dart';
import 'package:anywhere/src/rust/api.dart';
import 'launcher_fixtures.dart';

const binding = ActionShortcut(
    actionId: 'core.directory.terminal',
    title: 'Open Terminal Here',
    keyCode: 23,
    modifiers: HostSettings.altKey);
CandidateDto row(String id) => CandidateDto(
    id: id,
    commandId: binding.actionId,
    title: binding.title,
    subtitle: '/project/$id',
    kind: CandidateKindDto.action,
    contextKeys: [],
    menuPath: [],
    homeEligible: true,
    primaryAction: false,
    iconKey: 'terminal',
    score: 1,
    groupId: 'folder',
    availability: AvailabilityDto.enabled);
LauncherStateDto state({int session = 1}) => launcherSnapshot(
    sessionId: session,
    revision: 0,
    snapshotSeq: 1,
    input: '',
    contextComplete: true,
    searchSources: const [],
    resultActions: const [],
    candidates: [row('one')],
    diagnostics: []);

void main() {
  setUp(() => DesktopEnvironment.configure(
      const DesktopEnvironment(platform: TargetPlatform.macOS)));
  tearDown(DesktopEnvironment.reset);
  TestWidgetsFlutterBinding.ensureInitialized();
  const channel = MethodChannel('test/action-shortcuts');
  late HostSettings stored;
  final executions = <String>[];
  final hostCalls = <String>[];
  setUp(() {
    stored = HostSettings.defaults.copyWith(actionShortcuts: [binding]);
    executions.clear();
    hostCalls.clear();
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(channel, (call) async {
      hostCalls.add(call.method);
      if (call.method == 'settings') return stored.toMap();
      if (call.method == 'settingsStatus') return {};
      if (call.method == 'saveSettings') {
        stored = HostSettings.fromMap(call.arguments as Map);
      }
      return null;
    });
  });
  tearDown(() => TestDefaultBinaryMessengerBinding
      .instance.defaultBinaryMessenger
      .setMockMethodCallHandler(channel, null));

  test('shortcut limits count UTF-8 bytes consistently with the Host', () {
    ActionShortcut withLabels(String id, String title) => ActionShortcut(
        actionId: id,
        title: title,
        keyCode: binding.keyCode,
        modifiers: binding.modifiers);
    final title = List.filled(128, '😀').join();
    final id = List.filled(512, '😀').join();
    expect(withLabels(id, title).chordError, isNull);
    expect(withLabels('${id}a', title).chordError, isNotNull);
    expect(withLabels(id, '${title}a').chordError, isNotNull);
  });

  Future<LauncherController> controller(
      Future<List<CandidateDto>> Function(int, int, String) resolve,
      {HostViewDto? hostView,
      Future<BindingResolutionDto> Function(int, int, String)? wait,
      Future<void> Function()? shutdown,
      Future<BindingResolutionDto> Function(int, int, List<ShortcutRouteDto>)?
          intent,
      Future<BindingResolutionDto> Function(int, int, List<ShortcutRouteDto>)?
          waitIntent}) async {
    var sessionCount = 0;
    final c = LauncherController(
        backend: LauncherBackend(
            beginActivation: () => ++sessionCount,
            activate: (session,
                {promptAccessibility = false,
                capturedEnvironment = false}) async {
              hostCalls.add('capture:$session');
              return state(session: session);
            },
            updateInput: (session, input, revision,
                    {scopeId, inputMode = InputModeDto.environment}) async =>
                launcherSnapshot(
                    sessionId: session,
                    revision: revision,
                    snapshotSeq: 1,
                    input: input,
                    contextComplete: true,
                    searchSources: const [],
                    resultActions: const [],
                    candidates: [row('one')],
                    diagnostics: []),
            execute: (session, revision, id) async {
              executions.add(id);
              return ActionResultDto(
                  sessionId: session,
                  revision: revision,
                  status: ActionStatusDto.succeeded,
                  closeWindow: false,
                  view: hostView);
            },
            deactivate: (_) async {
              hostCalls.add('deactivate');
            },
            resolveBinding: resolve,
            waitBinding: wait,
            resolveIntent: intent,
            waitIntent: waitIntent,
            shortcutApplications: () async => const [
                  ShortcutApplicationDto(
                      bundleId: 'com.apple.finder', title: 'Finder')
                ],
            shortcutActions: () async => const [
                  ShortcutActionDto(
                      actionId: 'core.directory.terminal',
                      title: 'Open Terminal Here',
                      contextTypes: ['core.directory']),
                  ShortcutActionDto(
                      actionId: 'core.files.search',
                      title: 'File Search',
                      contextTypes: ['core.launcher'])
                ]),
        window: WindowHost(channel),
        readPlugins: () => [],
        applyExcludedApps: (_) async {},
        applyFileSearch: (_) async {},
        applyContextSources: (_) async {},
        shutdownTasks: shutdown ?? () async {});
    addTearDown(c.dispose);
    await c.settings.load();
    await c.activate();
    return c;
  }

  test(
      'File Search uses ordinary local and global Action shortcuts with a hidden entry',
      () async {
    const files = ActionShortcut(
        actionId: 'core.files.search',
        title: 'File Search',
        keyCode: 9,
        modifiers: HostSettings.altKey,
        global: true);
    final offer = CandidateDto(
        id: 'file-search-binding',
        commandId: files.actionId,
        title: files.title,
        kind: CandidateKindDto.action,
        contextKeys: [],
        menuPath: [],
        homeEligible: false,
        primaryAction: false,
        iconKey: 'search',
        score: 1,
        groupId: 'tools',
        availability: AvailabilityDto.enabled);
    stored = stored.copyWith(actionShortcuts: [files]);
    final c = await controller(
        (_, __, action) async {
          expect(action, files.actionId);
          return [offer];
        },
        hostView: HostViewDto.fileSearch,
        wait: (session, revision, action) async {
          expect(action, files.actionId);
          return BindingResolutionDto(
              state: state(session: session),
              candidates: [offer],
              requiresChoice: false);
        });
    await c.navigateRoot(WakeMode.universal);
    expect(c.view.visibleEntries, isEmpty);
    await c.executeShortcut(files);
    expect(c.view.isFileSearch, isTrue);
    await c.escape();
    expect(c.view.inputMode, InputModeDto.search);
    expect(c.view.visibleEntries, isEmpty);
    await c.dismiss();
    await c.triggerGlobalShortcut(files.actionId);
    expect(c.view.isFileSearch, isTrue);
    expect(executions, [offer.id, offer.id]);
    await c.escape();
    expect(c.view.inputMode, InputModeDto.search);
    expect(c.view.visibleEntries, isEmpty);
    await c.escape();
    expect(c.view.phase, LauncherPhase.hidden);
  });

  test(
      'old settings migrate, bindings round-trip, future schema remains protected',
      () {
    expect(HostSettings.fromMap({}).actionShortcuts, isEmpty);
    final restored =
        HostSettings.fromMap(stored.toMap()).actionShortcuts.single;
    expect(restored.actionId, binding.actionId);
    expect(restored.label, '⌥T');
    expect(restored.global, isFalse);
    expect(ActionShortcut.fromMap(binding.withGlobal(true).toMap())!.global,
        isTrue);
    expect(
        ActionShortcut.fromMap({...binding.toMap(), 'global': 'yes'}), isNull);
    expect(restored.toMap().keys, isNot(contains('candidateId')));
    final future =
        HostSettings.fromMap({...stored.toMap(), 'actionShortcutVersion': 5});
    expect(future.actionShortcuts, isEmpty);
    expect(
        future.copyWith(launchAtLogin: true).toMap()['actionShortcutVersion'],
        5);
    final localMigration = HostSettings.fromMap({
      ...stored.toMap(),
      'actionShortcutVersion': 1,
      'actionShortcuts': [
        {...binding.withGlobal(true).toMap(), 'keyCode': 17, 'modifiers': 2048}
      ]
    });
    expect(localMigration.actionShortcutVersion, 4);
    expect(localMigration.actionShortcuts.single.actionId, binding.actionId);
    expect(localMigration.actionShortcuts.single.global, isFalse);
  });

  test(
      'conflicts and editing commands are rejected in both assignment directions',
      () {
    expect(binding.validate(stored), isNull);
    final other = ActionShortcut(
        actionId: 'another',
        title: 'Another',
        keyCode: 23,
        modifiers: HostSettings.altKey);
    expect(other.validate(stored), contains('assigned'));
    expect(
        binding.validate(stored.copyWith(
            currentAppKeyCode: 23, currentAppModifiers: HostSettings.altKey)),
        contains('opens Anywhere'));
    for (final modifier in [0, HostSettings.shiftKey, HostSettings.metaKey]) {
      expect(
          ActionShortcut(
                  actionId: 'copy',
                  title: 'Copy',
                  keyCode: 6,
                  modifiers: modifier)
              .chordError,
          isNotNull);
    }
  });

  test('multiple targets require explicit selection and Escape cancels choices',
      () async {
    final c = await controller((_, __, ___) async => [row('one'), row('two')]);
    await c.executeShortcut(binding);
    expect(executions, isEmpty);
    expect(c.view.visibleCandidates.map((a) => a.id), ['one', 'two']);
    await c.escape();
    expect(c.view.bindingChoices, isNull);
    await c.executeShortcut(binding);
    await c.selectEntry(1);
    expect(executions, ['two']);
  });

  test('dismissal revokes a delayed shortcut resolution', () async {
    final pending = Completer<List<CandidateDto>>();
    final c = await controller((_, __, ___) => pending.future);
    final executing = c.executeShortcut(binding);
    await c.dismiss();
    pending.complete([row('one')]);
    await executing;
    expect(executions, isEmpty);
    expect(c.view.phase, LauncherPhase.hidden);
  });

  test('missing context reports an error and does not execute', () async {
    final c = await controller(
        (_, __, ___) async => throw StateError('No current context'));
    await c.executeShortcut(binding);
    expect(c.view.message, contains('No current context'));
    expect(executions, isEmpty);
  });

  test(
      'global trigger captures a new session before showing and ignores repeats while waiting',
      () async {
    stored = stored.copyWith(actionShortcuts: [binding.withGlobal(true)]);
    final pending = Completer<BindingResolutionDto>();
    var waitCalls = 0;
    final c = await controller((_, __, ___) async => [row('one')],
        wait: (session, revision, id) {
      waitCalls++;
      expect(session, 2);
      expect(revision, 0);
      expect(id, binding.actionId);
      return pending.future;
    });
    await c.dismiss();
    hostCalls.clear();
    final trigger = c.triggerGlobalShortcut(binding.actionId);
    await Future<void>.delayed(Duration.zero);
    expect(c.view.phase, LauncherPhase.resolving);
    expect(hostCalls.indexOf('capture:2'), lessThan(hostCalls.indexOf('show')));
    await c.triggerGlobalShortcut(binding.actionId);
    expect(waitCalls, 1);
    pending.complete(BindingResolutionDto(
        requiresChoice: false,
        state: state(session: 2),
        candidates: [row('fresh')]));
    await trigger;
    expect(executions, ['fresh']);
  });

  test(
      'Escape cancels pending global capture and a late completion cannot execute',
      () async {
    stored = stored.copyWith(actionShortcuts: [binding.withGlobal(true)]);
    final pending = Completer<BindingResolutionDto>();
    final c = await controller((_, __, ___) async => [row('one')],
        wait: (_, __, ___) => pending.future);
    await c.dismiss();
    final trigger = c.triggerGlobalShortcut(binding.actionId);
    await Future<void>.delayed(Duration.zero);
    await c.escape();
    pending.complete(BindingResolutionDto(
        requiresChoice: false,
        state: state(session: 2),
        candidates: [row('late')]));
    await trigger;
    expect(executions, isEmpty);
    expect(c.view.phase, LauncherPhase.hidden);
  });

  test(
      'typing revokes pending global execution; timeout and missing targets give feedback',
      () async {
    stored = stored.copyWith(actionShortcuts: [binding.withGlobal(true)]);
    final pending = Completer<BindingResolutionDto>();
    final c = await controller((_, __, ___) async => [row('one')],
        wait: (_, __, ___) => pending.future);
    await c.dismiss();
    final trigger = c.triggerGlobalShortcut(binding.actionId);
    await Future<void>.delayed(Duration.zero);
    await c.update('copy');
    pending.complete(BindingResolutionDto(
        requiresChoice: false,
        state: state(session: 2),
        candidates: [row('old revision')]));
    await trigger;
    expect(executions, isEmpty);
    expect(c.view.input, 'copy');
    await c.dismiss();
    final failure = await controller((_, __, ___) async => [],
        wait: (_, __, ___) async =>
            throw StateError('Context capture timed out'));
    await failure.dismiss();
    await failure.triggerGlobalShortcut(binding.actionId);
    expect(failure.view.message, contains('timed out'));
    expect(failure.view.phase, LauncherPhase.ready);
    expect(executions, isEmpty);
  });

  test('quit cancels capture before stopping tasks and rejects new activations',
      () async {
    stored = stored.copyWith(actionShortcuts: [binding.withGlobal(true)]);
    final pending = Completer<BindingResolutionDto>();
    final stopping = Completer<void>();
    final stopStarted = Completer<void>();
    final c = await controller((_, __, ___) async => [row('one')],
        wait: (_, __, ___) => pending.future,
        shutdown: () async {
          hostCalls.add('shutdownTasks');
          stopStarted.complete();
          await stopping.future;
        });
    await c.dismiss();
    hostCalls.clear();
    final trigger = c.triggerGlobalShortcut(binding.actionId);
    await Future<void>.delayed(Duration.zero);
    final quitting = c.lifecycle.prepareToQuit();
    await stopStarted.future;
    expect(hostCalls.indexOf('deactivate'),
        lessThan(hostCalls.indexOf('shutdownTasks')));
    await c.triggerGlobalShortcut(binding.actionId);
    await c.toggle();
    expect(c.view.phase, LauncherPhase.hidden);
    pending.complete(BindingResolutionDto(
        requiresChoice: false,
        state: state(session: 2),
        candidates: [row('late')]));
    await trigger;
    stopping.complete();
    expect(await quitting, isTrue);
    await c.triggerGlobalShortcut(
        binding.actionId); // A queued event after shutdown is also rejected.
    await c.toggle();
    await c.openSettings();
    expect(c.view.phase, LauncherPhase.hidden);
    expect(executions, isEmpty);
  });

  test(
      'global multi-object capture requires selection; local and removed event IDs do nothing',
      () async {
    final c = await controller((_, __, ___) async => [row('one')],
        wait: (session, _, __) async => BindingResolutionDto(
            requiresChoice: false,
            state: state(session: session),
            candidates: [row('one'), row('two')]));
    await c.dismiss();
    await c.triggerGlobalShortcut(binding.actionId); // local-only configuration
    expect(c.view.phase, LauncherPhase.hidden);
    stored = stored.copyWith(actionShortcuts: [binding.withGlobal(true)]);
    await c.settings.load();
    await c.triggerGlobalShortcut('removed.action');
    expect(c.view.phase, LauncherPhase.hidden);
    await c.triggerGlobalShortcut(binding.actionId);
    expect(executions, isEmpty);
    expect(c.view.visibleCandidates.map((a) => a.id), ['one', 'two']);
    await c.selectEntry(1);
    expect(executions, ['two']);
  });

  test(
      'intent roundtrip preserves conditions, rejects duplicates, and upgrades fixed global v2',
      () {
    const rule = ShortcutRoute(
        actionId: 'core.directory.terminal',
        title: 'Open Terminal Here',
        appBundleId: 'com.apple.finder',
        appName: 'Finder',
        contextType: 'core.directory');
    const intent = ActionShortcut(
        actionId: 'intent:test',
        title: 'Terminal',
        keyCode: 23,
        modifiers: HostSettings.altKey,
        global: true,
        routes: [rule]);
    final settings = HostSettings.defaults.copyWith(actionShortcuts: [intent]);
    final restored =
        HostSettings.fromMap(settings.toMap()).actionShortcuts.single;
    expect(restored.validate(settings), isNull);
    expect(restored.routes.single.toMap(), rule.toMap());
    expect(restored.routes.single.toDto().contextType, 'core.directory');
    expect(restored.toMap().toString(), isNot(contains('contextKey')));
    const duplicated = ActionShortcut(
        actionId: 'intent:duplicate',
        title: 'Duplicate',
        keyCode: 21,
        modifiers: HostSettings.altKey,
        routes: [rule, rule]);
    expect(duplicated.configurationError, contains('different'));
    final v2 = HostSettings.fromMap({
      ...stored.toMap(),
      'actionShortcutVersion': 2,
      'actionShortcuts': [
        {...binding.withGlobal(true).toMap(), 'keyCode': 17, 'modifiers': 2048}
      ]
    });
    expect(v2.actionShortcutVersion, 4);
    expect(v2.actionShortcuts.single.global, isTrue);
    expect(v2.actionShortcuts.single.isIntent, isFalse);
  });

  test('ambiguous intent forces a chooser even for one deduplicated target',
      () async {
    const configured = ActionShortcut(
        actionId: 'intent:test',
        title: 'Terminal',
        keyCode: 23,
        modifiers: HostSettings.altKey,
        routes: [
          ShortcutRoute(
              actionId: 'core.directory.terminal',
              title: 'Terminal',
              contextType: 'core.directory'),
          ShortcutRoute(
              actionId: 'core.directory.terminal',
              title: 'Terminal',
              appBundleId: 'com.apple.finder')
        ]);
    stored = HostSettings.defaults.copyWith(actionShortcuts: [configured]);
    var fixedCalls = 0;
    final c = await controller((_, __, ___) async {
      fixedCalls++;
      return [row('wrong')];
    }, intent: (session, revision, routes) async {
      expect(routes.length, 2);
      return BindingResolutionDto(
          state: state(), candidates: [row('one')], requiresChoice: true);
    });
    await c.executeShortcut(configured);
    expect(executions, isEmpty);
    expect(fixedCalls, 0);
    expect(c.view.bindingChoices!.single.id, 'one');
    await c.selectEntry(0);
    expect(executions, ['one']);
  });

  test('global intent uses fresh session and late resolution is cancelled',
      () async {
    const configured = ActionShortcut(
        actionId: 'intent:global',
        title: 'Terminal',
        keyCode: 23,
        modifiers: HostSettings.altKey,
        global: true,
        routes: [
          ShortcutRoute(
              actionId: 'core.directory.terminal',
              title: 'Terminal',
              appBundleId: 'com.apple.finder')
        ]);
    stored = HostSettings.defaults.copyWith(actionShortcuts: [configured]);
    final pending = Completer<BindingResolutionDto>();
    final c = await controller(
        (_, __, ___) async =>
            throw StateError('Fixed resolver must not be called'),
        waitIntent: (session, revision, rules) {
      expect(session, 2);
      expect(revision, 0);
      expect(rules.single.appBundleId, 'com.apple.finder');
      return pending.future;
    });
    await c.dismiss();
    final trigger = c.triggerGlobalShortcut(configured.actionId);
    await Future<void>.delayed(Duration.zero);
    await c.escape();
    pending.complete(BindingResolutionDto(
        state: state(session: 2),
        candidates: [row('late')],
        requiresChoice: false));
    await trigger;
    expect(executions, isEmpty);
  });

  test(
      'an old host event cannot run an edited intent with the same binding identity',
      () async {
    const old = ActionShortcut(
        actionId: 'intent:event',
        title: 'Terminal',
        keyCode: 23,
        modifiers: HostSettings.altKey,
        global: true,
        routes: [
          ShortcutRoute(
              actionId: 'core.directory.terminal',
              title: 'Terminal',
              appBundleId: 'com.apple.finder')
        ]);
    const edited = ActionShortcut(
        actionId: 'intent:event',
        title: 'Terminal',
        keyCode: 23,
        modifiers: HostSettings.altKey,
        global: true,
        routes: [
          ShortcutRoute(
              actionId: 'core.directory.terminal',
              title: 'Terminal',
              contextType: 'core.directory')
        ]);
    stored = HostSettings.defaults.copyWith(actionShortcuts: [edited]);
    var calls = 0;
    final c = await controller((_, __, ___) async => [],
        waitIntent: (session, _, routes) async {
      calls++;
      return BindingResolutionDto(
          state: state(session: session),
          candidates: [row('current')],
          requiresChoice: false);
    });
    await c.dismiss();
    await c.triggerGlobalShortcut(old.actionId, requestedBinding: old);
    expect(calls, 0);
    expect(c.view.phase, LauncherPhase.hidden);
    await c.triggerGlobalShortcut(edited.actionId, requestedBinding: edited);
    expect(calls, 1);
    expect(executions, ['current']);
  });

  testWidgets(
      'settings assigns File Search normally and keeps its capability out of context conditions',
      (tester) async {
    stored = HostSettings.defaults;
    tester.view.physicalSize = const Size(648, 508);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    final c = await controller((_, __, ___) async => []);
    await tester.pumpWidget(MaterialApp(
        theme: AppStyle.theme, home: LauncherScreen(controller: c)));
    await c.openSettings();
    await tester.pumpAndSettle();
    await tester.scrollUntilVisible(find.text('Add action shortcut'), 180);
    await tester.tap(find.text('Add action shortcut'));
    await tester.pumpAndSettle();
    expect(find.text('Anywhere tool'), findsOneWidget);
    expect(find.text('core.launcher'), findsNothing);
    await tester.tap(find.text('File Search'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Record shortcut'));
    await tester.sendKeyDownEvent(LogicalKeyboardKey.altLeft);
    await tester.sendKeyDownEvent(LogicalKeyboardKey.keyF);
    await tester.sendKeyUpEvent(LogicalKeyboardKey.keyF);
    await tester.sendKeyUpEvent(LogicalKeyboardKey.altLeft);
    await tester.pumpAndSettle();
    await tester.tap(find.text('Global shortcut'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Save shortcut'));
    await tester.pumpAndSettle();
    final saved = stored.actionShortcuts.single;
    expect(saved.actionId, 'core.files.search');
    expect(saved.label, '⌥F');
    expect(saved.global, isTrue);
    await tester.scrollUntilVisible(find.text('Add context shortcut'), 180);
    await tester.tap(find.text('Add context shortcut'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Add rule'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('File Search').last);
    await tester.pumpAndSettle();
    final conditions = tester.widget<DropdownButton<String>>(find.descendant(
        of: find.byKey(const ValueKey('rule-context')),
        matching: find.byType(DropdownButton<String>)));
    expect(conditions.items!.map((item) => item.value), ['']);
    await tester.tap(find.text('Cancel').last);
    await tester.pumpAndSettle();
    await tester.tap(find.text('Cancel').last);
    await tester.pumpAndSettle();
    expect(tester.takeException(), isNull);
  });

  testWidgets(
      'settings can create, edit and remove a context shortcut with app and context rules',
      (tester) async {
    stored = HostSettings.defaults;
    tester.view.physicalSize = const Size(648, 508);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    final c = await controller((_, __, ___) async => [row('one')]);
    await tester.pumpWidget(MaterialApp(
        theme: AppStyle.theme, home: LauncherScreen(controller: c)));
    await c.openSettings();
    await tester.pumpAndSettle();
    await tester.scrollUntilVisible(find.text('Add context shortcut'), 180);
    await tester.ensureVisible(
        find.widgetWithText(OutlinedButton, 'Add context shortcut'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Add context shortcut'));
    await tester.pumpAndSettle();
    expect(await c.closeSettings(), isFalse);
    await tester.enterText(find.byKey(const ValueKey('context-shortcut-name')),
        'Terminal by context');
    await tester.tap(find.text('Add rule'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Open Terminal Here'));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('rule-app')));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Finder').last);
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('rule-context')));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Folder').last);
    await tester.pumpAndSettle();
    await tester.tap(find.text('Save rule'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Record shortcut'));
    await tester.sendKeyDownEvent(LogicalKeyboardKey.altLeft);
    await tester.sendKeyDownEvent(LogicalKeyboardKey.keyT);
    await tester.sendKeyUpEvent(LogicalKeyboardKey.keyT);
    await tester.sendKeyUpEvent(LogicalKeyboardKey.altLeft);
    await tester.pumpAndSettle();
    await tester.tap(find.text('Save context shortcut'));
    await tester.pumpAndSettle();
    final configured = stored.actionShortcuts.single;
    expect(configured.isIntent, isTrue);
    expect(configured.global, isTrue);
    expect(configured.routes.single.appBundleId, 'com.apple.finder');
    expect(configured.routes.single.contextType, 'core.directory');
    await tester.tap(find.text('⌥T'));
    await tester.pumpAndSettle();
    await tester.enterText(find.byKey(const ValueKey('context-shortcut-name')),
        'Renamed terminal');
    await tester.tap(find.text('Save context shortcut'));
    await tester.pumpAndSettle();
    expect(stored.actionShortcuts.single.actionId, configured.actionId);
    expect(stored.actionShortcuts.single.title, 'Renamed terminal');
    await tester.tap(find.byTooltip('Remove shortcut for Renamed terminal'));
    await tester.pumpAndSettle();
    expect(stored.actionShortcuts, isEmpty);
  });

  testWidgets(
      'visible Launcher handles the configured chord once, including key repeat',
      (tester) async {
    final c = await controller((_, __, ___) async => [row('one')]);
    await tester.pumpWidget(MaterialApp(
        theme: AppStyle.theme, home: LauncherScreen(controller: c)));
    await tester.pumpAndSettle();
    await tester.sendKeyDownEvent(LogicalKeyboardKey.altLeft);
    await tester.sendKeyDownEvent(LogicalKeyboardKey.keyT);
    await tester.pumpAndSettle();
    await tester.sendKeyRepeatEvent(LogicalKeyboardKey.keyT);
    await tester.pumpAndSettle();
    await tester.sendKeyUpEvent(LogicalKeyboardKey.keyT);
    await tester.sendKeyUpEvent(LogicalKeyboardKey.altLeft);
    expect(executions, ['one']);
  });

  testWidgets(
      'an undetected saved script can change keys and scope without changing action identity',
      (tester) async {
    const script = ActionShortcut(
        actionId: 'developer.node.script.test',
        title: 'npm test',
        keyCode: 23,
        modifiers: HostSettings.altKey);
    stored = HostSettings.defaults.copyWith(actionShortcuts: [script]);
    tester.view.physicalSize = const Size(648, 508);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    final c = await controller((_, __, ___) async => []);
    await tester.pumpWidget(MaterialApp(
        theme: AppStyle.theme, home: LauncherScreen(controller: c)));
    await c.openSettings();
    await tester.pumpAndSettle();
    await tester.tap(find.text('⌥T'));
    await tester.pumpAndSettle();
    expect(
        find.descendant(
            of: find.byType(AlertDialog), matching: find.text('npm test')),
        findsOneWidget);
    expect(find.text('Choose another action'), findsNothing);
    // A selected action form should fit without the old empty list area.
    final dialogSurface = find.descendant(
        of: find.byType(AlertDialog), matching: find.byType(Material));
    expect(tester.getSize(dialogSurface.first).height, lessThan(440));
    await tester.tap(find.text('⌥T').last);
    await tester.sendKeyDownEvent(LogicalKeyboardKey.altLeft);
    await tester.sendKeyDownEvent(LogicalKeyboardKey.shiftLeft);
    await tester.sendKeyEvent(LogicalKeyboardKey.keyT);
    await tester.sendKeyUpEvent(LogicalKeyboardKey.shiftLeft);
    await tester.sendKeyUpEvent(LogicalKeyboardKey.altLeft);
    await tester.pumpAndSettle();
    await tester.tap(find.text('Global shortcut'));
    await tester.tap(find.text('Save shortcut'));
    await tester.pumpAndSettle();
    expect(stored.actionShortcuts.single.actionId, script.actionId);
    expect(stored.actionShortcuts.single.title, script.title);
    expect(stored.actionShortcuts.single.modifiers,
        HostSettings.altKey | HostSettings.shiftKey);
    expect(stored.actionShortcuts.single.global, isTrue);
  });

  testWidgets(
      'settings records native chords, saves and removes bindings; close cannot orphan the editor',
      (tester) async {
    debugDefaultTargetPlatformOverride = TargetPlatform.macOS;
    DesktopEnvironment.configure(const DesktopEnvironment(
        platform: TargetPlatform.macOS, nativeShortcutRecording: true));
    try {
      const recorderChannel = MethodChannel('anywhere/shortcut-recorder');
      final nativeChord = Completer<Map<String, int>>();
      TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
          .setMockMethodCallHandler(recorderChannel, (call) async {
        return call.method == 'record' ? nativeChord.future : null;
      });
      addTearDown(() => TestDefaultBinaryMessengerBinding
          .instance.defaultBinaryMessenger
          .setMockMethodCallHandler(recorderChannel, null));
      stored = HostSettings.defaults;
      tester.view.physicalSize = const Size(648, 508);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);
      final c = await controller((_, __, ___) async => [row('one')]);
      await tester.pumpWidget(MaterialApp(
          theme: AppStyle.theme, home: LauncherScreen(controller: c)));
      await c.openSettings();
      await tester.pumpAndSettle();
      await tester.scrollUntilVisible(find.text('Add action shortcut'), 180);
      await tester.tap(find.text('Add action shortcut'));
      await tester.pumpAndSettle();
      expect(await c.closeSettings(), isFalse);
      await tester.tap(find.text('Open Terminal Here'));
      await tester.pumpAndSettle();
      await tester.tap(find.text('Record shortcut'));
      await tester.pump();
      nativeChord.complete({'keyCode': 23, 'modifiers': HostSettings.altKey});
      await tester.pumpAndSettle();
      await tester.tap(find.text('Global shortcut'));
      await tester.pumpAndSettle();
      expect(
          tester
              .widget<FilledButton>(
                  find.widgetWithText(FilledButton, 'Save shortcut'))
              .onPressed,
          isNotNull);
      await tester.tap(find.text('Save shortcut'));
      await tester.pumpAndSettle();
      expect(stored.actionShortcuts, isNotEmpty,
          reason: c.settings.statusMessage);
      expect(stored.actionShortcuts.single.actionId, binding.actionId);
      expect(stored.actionShortcuts.single.global, isTrue);
      await tester
          .tap(find.byTooltip('Remove shortcut for Open Terminal Here'));
      await tester.pumpAndSettle();
      expect(stored.actionShortcuts, isEmpty);
      expect(await c.closeSettings(), isTrue);
    } finally {
      debugDefaultTargetPlatformOverride = null;
    }
  });
}
