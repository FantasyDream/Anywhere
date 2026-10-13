import 'dart:async';
import 'package:anywhere/infrastructure/window_host.dart';
import 'package:anywhere/settings/extensions_section.dart';
import 'package:anywhere/settings/settings_screen.dart';
import 'package:anywhere/settings/settings_controller.dart';
import 'package:anywhere/settings/host_settings.dart';
import 'package:anywhere/settings/action_shortcut.dart';
import 'package:anywhere/src/rust/api.dart';
import 'package:anywhere/ui/app_style.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

ExtensionStatusDto extension(
        {bool active = false, bool firstParty = false, bool enabled = true}) =>
    ExtensionStatusDto(
        key: firstParty ? 'first-party:json' : 'plugin:test',
        id: firstParty ? 'json' : 'test',
        name: firstParty ? 'JSON' : 'Test Copy',
        version: '1.0.0',
        firstParty: firstParty,
        legacy: false,
        enabled: enabled,
        active: active,
        needsApproval: !active && !firstParty,
        configurable: true,
        capabilities: ['clipboard.write'],
        approvalToken: 'inspection-1',
        detail: active ? 'Enabled' : 'Needs permission approval');

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  const channel = MethodChannel('test/extensions-settings');
  final messenger =
      TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger;
  late List<Map> syncs;
  final saved = HostSettings.defaults.copyWith(actionShortcuts: [
    const ActionShortcut(
        actionId: 'test.copy',
        title: 'Copy',
        keyCode: 17,
        modifiers: HostSettings.altKey,
        global: true)
  ]);
  setUp(() {
    syncs = [];
    messenger.setMockMethodCallHandler(channel, (call) async {
      if (call.method == 'settings') return saved.toMap();
      if (call.method == 'settingsStatus') return {};
      if (call.method == 'syncActionShortcutCatalog') {
        syncs.add(call.arguments as Map);
      }
      return null;
    });
  });
  tearDown(() => messenger.setMockMethodCallHandler(channel, null));

  test(
      'committed reload with failed response releases obsolete chords and can recover',
      () async {
    var supported = ['test.copy'];
    var failed = true;
    final c = SettingsController(
        window: WindowHost(channel),
        readExtensions: () => [extension(active: true)],
        reloadExtensions: () async {
          supported = [];
          if (failed) throw StateError('response failed after commit');
          return [extension()];
        },
        readSupportedShortcutActions: (ids) async => supported);
    addTearDown(c.dispose);
    await c.load(plugins: true);
    expect(c.shortcutSupported('test.copy'), isTrue);
    await c.refreshPlugins();
    expect(c.shortcutSupported('test.copy'), isFalse);
    expect(syncs.last['supportedActionIds'], isEmpty);
    expect(c.statusMessage, contains('response failed'));
    failed = false;
    await c.refreshPlugins();
    expect(c.extensions.single.active, isFalse);
    expect(c.statusMessage, isNull);
  });

  test(
      'legacy reload succeeds but status read fails: catalog is still reconciled',
      () async {
    var failed = false, available = true;
    final c = SettingsController(
        window: WindowHost(channel),
        readExtensions: () {
          if (failed) throw StateError('status read failed');
          return [extension(active: true)];
        },
        reload: () async {
          failed = true;
          available = false;
          return [];
        },
        readSupportedShortcutActions: (ids) async => available ? ids : []);
    addTearDown(c.dispose);
    await c.load(plugins: true);
    await c.refreshPlugins();
    expect(c.shortcutSupported('test.copy'), isFalse);
    expect(syncs.last['supportedActionIds'], isEmpty);
    expect(c.statusMessage, contains('status read failed'));
  });

  test('approve and revoke refresh picker and native hotkey eligibility',
      () async {
    var current = extension();
    final approvals = <(String, String)>[];
    final c = SettingsController(
        window: WindowHost(channel),
        readPlugins: () => [],
        readExtensions: () => [current],
        grantExtension: (key, token) async {
          approvals.add((key, token));
          current = extension(active: true);
          return [current];
        },
        removeExtensionGrant: (key) async {
          current = extension();
          return [current];
        },
        readShortcutActions: () async => [
              if (current.active)
                const ShortcutActionDto(
                    actionId: 'test.copy',
                    title: 'Copy',
                    contextTypes: ['core.text'])
            ],
        readSupportedShortcutActions: (ids) async => current.active ? ids : []);
    addTearDown(c.dispose);
    await c.load(plugins: true);
    expect(c.shortcutSupported('test.copy'), isFalse);
    await c.approveExtension(current);
    expect(approvals, [('plugin:test', 'inspection-1')]);
    expect(c.shortcutActions.single.actionId, 'test.copy');
    expect(c.shortcutSupported('test.copy'), isTrue);
    await c.revokeExtension(current);
    expect(c.shortcutActions, isEmpty);
    expect(c.shortcutSupported('test.copy'), isFalse);
    expect(syncs.last['supportedActionIds'], isEmpty);
  });

  test('failed module save preserves UI and revalidates backend catalog',
      () async {
    final old = extension(active: true, firstParty: true);
    final c = SettingsController(
        window: WindowHost(channel),
        readPlugins: () => [],
        readExtensions: () => [old],
        enableExtension: (_, __) async => throw StateError('disk full'),
        readSupportedShortcutActions: (ids) async => ids);
    addTearDown(c.dispose);
    await c.load(plugins: true);
    final count = syncs.length;
    await c.setExtensionEnabled(old, false);
    expect(c.extensions.single, same(old));
    expect(syncs.length, count + 1);
    expect(c.statusMessage, contains('disk full'));
    expect(c.refreshingPlugins, isFalse);
  });

  test(
      'close waits for mutation and catalog sync and rejects overlapping operations',
      () async {
    final gate = Completer<List<ExtensionStatusDto>>();
    final sync = Completer<List<String>>();
    var changes = 0, reads = 0;
    final c = SettingsController(
        window: WindowHost(channel),
        readPlugins: () => [],
        readExtensions: () => [extension(active: true)],
        removeExtensionGrant: (_) {
          changes++;
          return gate.future;
        },
        readSupportedShortcutActions: (ids) =>
            ++reads == 1 ? Future.value(ids) : sync.future);
    addTearDown(c.dispose);
    await c.load(plugins: true);
    final mutation = c.revokeExtension(extension(active: true));
    await c.revokeExtension(extension(active: true));
    expect(changes, 1);
    var closed = false;
    final closing = c.prepareClose().then((value) {
      closed = value;
    });
    gate.complete([extension()]);
    await Future<void>.delayed(Duration.zero);
    expect(closed, isFalse);
    expect(c.refreshingPlugins, isTrue);
    sync.complete([]);
    await mutation;
    await closing;
    expect(closed, isTrue);
    expect(c.refreshingPlugins, isFalse);
  });

  test(
      'a mutation completing after dispose does not publish or restore hotkeys',
      () async {
    final gate = Completer<List<ExtensionStatusDto>>();
    final c = SettingsController(
        window: WindowHost(channel),
        readPlugins: () => [],
        readExtensions: () => [extension()],
        grantExtension: (_, __) => gate.future,
        readSupportedShortcutActions: (ids) async => ids);
    await c.load(plugins: true);
    final count = syncs.length;
    final mutation = c.approveExtension(extension());
    c.dispose();
    gate.complete([extension(active: true)]);
    await mutation;
    expect(syncs.length, count);
    expect(c.extensions.single.active, isFalse);
  });

  test('a slow status read cannot replace a completed module change', () async {
    final old = extension(active: true, firstParty: true);
    final gate = Completer<List<ExtensionStatusDto>>();
    final started = Completer<void>();
    final c = SettingsController(
        window: WindowHost(channel),
        readPlugins: () => [],
        readExtensions: () {
          started.complete();
          return gate.future;
        },
        enableExtension: (_, __) async =>
            [extension(firstParty: true, enabled: false)]);
    addTearDown(c.dispose);
    final load = c.load(plugins: true);
    await started.future;
    await c.setExtensionEnabled(old, false);
    gate.complete([old]);
    await load;
    expect(c.extensions.single.enabled, isFalse);
  });

  testWidgets(
      'permission review defaults to Cancel and approves with Right then Enter',
      (tester) async {
    var grants = 0;
    final c = SettingsController(
        window: WindowHost(channel),
        readPlugins: () => [],
        readExtensions: () =>
            [extension(active: true, firstParty: true), extension()],
        grantExtension: (_, __) async {
          grants++;
          return [];
        });
    addTearDown(c.dispose);
    await c.load(plugins: true);
    await tester.pumpWidget(MaterialApp(
        theme: AppStyle.theme,
        builder: AppStyle.windowBuilder,
        home: Scaffold(
            body: SingleChildScrollView(
                child: ExtensionsSection(controller: c)))));
    await tester.tap(find.text('Review permissions…'));
    await tester.pumpAndSettle();
    expect(find.text('Allow Test Copy?'), findsOneWidget);
    expect(find.text('• Write to the clipboard'), findsOneWidget);
    expect(find.text('test · 1.0.0'), findsOneWidget);
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pumpAndSettle();
    expect(grants, 0);
    await tester.tap(find.text('Review permissions…'));
    await tester.pumpAndSettle();
    await tester.sendKeyEvent(LogicalKeyboardKey.arrowRight);
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pumpAndSettle();
    expect(grants, 1);
    expect(tester.takeException(), isNull);
  });

  testWidgets(
      'all built-in modules fit the existing settings window and errors remain visible',
      (tester) async {
    tester.view.physicalSize = const Size(648, 508);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    final modules = [
      'Git',
      'URL',
      'JSON',
      'Rust',
      'Maven',
      'Gradle',
      'Node',
      'Spring Boot',
      'Saved Directories'
    ];
    final c = SettingsController(
        window: WindowHost(channel),
        readPlugins: () => [],
        readExtensions: () => [
              for (final name in modules)
                ExtensionStatusDto(
                    key: 'first-party:$name',
                    id: name,
                    name: name,
                    version: '0.8.0',
                    firstParty: true,
                    legacy: false,
                    enabled: true,
                    active: name != 'Saved Directories',
                    needsApproval: false,
                    configurable: true,
                    capabilities: name == 'Saved Directories'
                        ? ['storage', 'file.open']
                        : ['process.start'],
                    approvalToken: '',
                    detail: name == 'Saved Directories'
                        ? 'Storage unavailable'
                        : 'Enabled')
            ]);
    addTearDown(c.dispose);
    await c.load(plugins: true);
    await tester.pumpWidget(MaterialApp(
        theme: AppStyle.theme,
        builder: (context, child) => AppStyle.windowBuilder(
            context,
            MediaQuery(
                data: MediaQuery.of(context)
                    .copyWith(textScaler: TextScaler.linear(1.3)),
                child: child!)),
        home: Scaffold(
            body: SettingsScreen(controller: c, onClose: () async => false))));
    await tester.tap(find.byKey(const ValueKey('settings-plugins')));
    await tester.pumpAndSettle();
    await tester.scrollUntilVisible(find.text('Saved Directories'), 220,
        scrollable: find.descendant(
            of: find.byKey(const PageStorageKey('settings-content-plugins')),
            matching: find.byType(Scrollable)));
    expect(find.text('0.8.0 · Could not start · Storage unavailable'),
        findsOneWidget);
    expect(find.text('Save extension data · Open files'), findsOneWidget);
    expect(tester.takeException(), isNull);
  });
}
