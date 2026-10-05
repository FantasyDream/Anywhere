import 'dart:async';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:anywhere/infrastructure/window_host.dart';
import 'package:anywhere/launcher/launcher_controller.dart';
import 'package:anywhere/launcher/launcher_screen.dart';
import 'package:anywhere/launcher/launcher_style.dart';
import 'package:anywhere/launcher/launcher_view_state.dart';
import 'package:anywhere/settings/action_shortcut.dart';
import 'package:anywhere/settings/host_settings.dart';
import 'package:anywhere/src/rust/api.dart';
import 'launcher_fixtures.dart';

const binding = ActionShortcut(
    actionId: 'core.directory.terminal',
    title: 'Open Terminal Here',
    keyCode: 17,
    modifiers: HostSettings.optionKey);
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
LauncherStateDto state() => launcherSnapshot(
    sessionId: 1,
    revision: 0,
    snapshotSeq: 1,
    input: '',
    contextComplete: true,
    candidates: [row('one')],
    diagnostics: []);

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  const channel = MethodChannel('test/action-shortcuts');
  late HostSettings stored;
  final executions = <String>[];
  setUp(() {
    stored = HostSettings.defaults.copyWith(actionShortcuts: [binding]);
    executions.clear();
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(channel, (call) async {
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

  Future<LauncherController> controller(
      Future<List<CandidateDto>> Function(int, int, String) resolve) async {
    final c = LauncherController(
        backend: LauncherBackend(
            beginActivation: () => 1,
            activate: (_, {promptAccessibility = false}) async => state(),
            updateInput: (session, input, revision, {scopeId}) async =>
                launcherSnapshot(
                    sessionId: session,
                    revision: revision,
                    snapshotSeq: 1,
                    input: input,
                    contextComplete: true,
                    candidates: [row('one')],
                    diagnostics: []),
            execute: (session, revision, id) async {
              executions.add(id);
              return ActionResultDto(
                  sessionId: session,
                  revision: revision,
                  status: ActionStatusDto.succeeded,
                  closeWindow: false);
            },
            deactivate: (_) async {},
            resolveBinding: resolve,
            shortcutActions: () async => const [
                  ShortcutActionDto(
                      actionId: 'core.directory.terminal',
                      title: 'Open Terminal Here',
                      contextTypes: ['core.directory'])
                ]),
        window: WindowHost(channel),
        readPlugins: () => [],
        applyExcludedApps: (_) async {},
        applyContextSources: (_) async {});
    addTearDown(c.dispose);
    await c.settings.load();
    await c.activate();
    return c;
  }

  test(
      'old settings migrate, bindings round-trip, future schema remains protected',
      () {
    expect(HostSettings.fromMap({}).actionShortcuts, isEmpty);
    final restored =
        HostSettings.fromMap(stored.toMap()).actionShortcuts.single;
    expect(restored.actionId, binding.actionId);
    expect(restored.label, '⌥T');
    expect(restored.toMap().keys, isNot(contains('candidateId')));
    final future =
        HostSettings.fromMap({...stored.toMap(), 'actionShortcutVersion': 2});
    expect(future.actionShortcuts, isEmpty);
    expect(
        future.copyWith(launchAtLogin: true).toMap()['actionShortcutVersion'],
        2);
  });

  test(
      'conflicts and editing commands are rejected in both assignment directions',
      () {
    expect(binding.validate(stored), isNull);
    final other = ActionShortcut(
        actionId: 'another',
        title: 'Another',
        keyCode: 17,
        modifiers: HostSettings.optionKey);
    expect(other.validate(stored), contains('assigned'));
    expect(
        binding.validate(stored.copyWith(
            currentAppKeyCode: 17,
            currentAppModifiers: HostSettings.optionKey)),
        contains('opens Anywhere'));
    for (final modifier in [0, HostSettings.shiftKey, HostSettings.cmdKey]) {
      expect(
          ActionShortcut(
                  actionId: 'copy',
                  title: 'Copy',
                  keyCode: 8,
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

  testWidgets(
      'visible Launcher handles the configured chord once, including key repeat',
      (tester) async {
    final c = await controller((_, __, ___) async => [row('one')]);
    await tester.pumpWidget(MaterialApp(
        theme: LauncherStyle.theme, home: LauncherScreen(controller: c)));
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
      'settings can record, save and remove a binding; native close cannot orphan the editor',
      (tester) async {
    stored = HostSettings.defaults;
    tester.view.physicalSize = const Size(648, 508);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    final c = await controller((_, __, ___) async => [row('one')]);
    await tester.pumpWidget(MaterialApp(
        theme: LauncherStyle.theme, home: LauncherScreen(controller: c)));
    await c.openSettings();
    await tester.pumpAndSettle();
    await tester.scrollUntilVisible(find.text('Add action shortcut'), 180);
    await tester.tap(find.text('Add action shortcut'));
    await tester.pumpAndSettle();
    expect(await c.closeSettings(), isFalse);
    await tester.tap(find.text('Open Terminal Here'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Record shortcut'));
    await tester.sendKeyDownEvent(LogicalKeyboardKey.altLeft);
    await tester.sendKeyDownEvent(LogicalKeyboardKey.keyT);
    await tester.sendKeyUpEvent(LogicalKeyboardKey.keyT);
    await tester.sendKeyUpEvent(LogicalKeyboardKey.altLeft);
    await tester.pumpAndSettle();
    await tester.tap(find.text('Save shortcut'));
    await tester.pumpAndSettle();
    expect(stored.actionShortcuts.single.actionId, binding.actionId);
    await tester.tap(find.byTooltip('Remove shortcut for Open Terminal Here'));
    await tester.pumpAndSettle();
    expect(stored.actionShortcuts, isEmpty);
    expect(await c.closeSettings(), isTrue);
  });
}
