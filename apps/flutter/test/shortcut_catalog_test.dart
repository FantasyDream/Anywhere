import 'dart:async';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:anywhere/infrastructure/window_host.dart';
import 'package:anywhere/settings/action_shortcut.dart';
import 'package:anywhere/settings/host_settings.dart';
import 'package:anywhere/settings/settings_controller.dart';
import 'package:anywhere/src/rust/api.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  const channel = MethodChannel('test/shortcut-catalog');
  const fixed = ActionShortcut(
      actionId: 'plugin.copy',
      title: 'Copy',
      keyCode: 6,
      modifiers: HostSettings.altKey,
      global: true);
  const intent = ActionShortcut(
      actionId: 'intent:test',
      title: 'Context',
      keyCode: 23,
      modifiers: HostSettings.altKey,
      global: true,
      routes: [
        ShortcutRoute(
            actionId: 'plugin.copy', title: 'Copy', appBundleId: 'test.app'),
        ShortcutRoute(actionId: 'core.directory.terminal', title: 'Terminal'),
      ]);
  late HostSettings stored;
  late List<Map> syncs;
  setUp(() {
    stored = HostSettings.defaults.copyWith(actionShortcuts: [fixed, intent]);
    syncs = [];
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(channel, (call) async {
      if (call.method == 'settings') return stored.toMap();
      if (call.method == 'settingsStatus') return {};
      if (call.method == 'saveSettings') {
        stored = HostSettings.fromMap(call.arguments as Map);
      }
      if (call.method == 'syncActionShortcutCatalog') {
        syncs.add(call.arguments as Map);
      }
      return null;
    });
  });
  tearDown(() => TestDefaultBinaryMessengerBinding
      .instance.defaultBinaryMessenger
      .setMockMethodCallHandler(channel, null));

  SettingsController controller(
          {required Future<List<String>> Function(List<String>) supported,
          Future<List<PluginStatusDto>> Function()? refreshPluginReports,
          Future<List<ShortcutActionDto>> Function()? actions}) =>
      SettingsController(
          window: WindowHost(channel),
          applyExcludedApps: (_) async {},
          applyFileSearch: (_) async {},
          applyContextSources: (_) async {},
          readPlugins: () => [],
          refreshPluginReports: refreshPluginReports ?? () async => [],
          readSupportedShortcutActions: supported,
          readShortcutActions: actions);

  test(
      'rejected save with failed readback retains the previous binding snapshot',
      () async {
    final c = controller(supported: (ids) async => ids);
    addTearDown(c.dispose);
    await c.load();
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(channel, (call) async {
      if (call.method == 'saveSettings') {
        throw PlatformException(
            code: 'shortcuts', message: 'System key is occupied');
      }
      if (call.method == 'settings') {
        throw PlatformException(code: 'read', message: 'Readback unavailable');
      }
      if (call.method == 'settingsStatus') return {};
      if (call.method == 'syncActionShortcutCatalog') {
        syncs.add(call.arguments as Map);
      }
      return null;
    });
    expect(
        await c.save(c.current.copyWith(actionShortcuts: [intent])), isFalse);
    expect(c.current.actionShortcuts.map((b) => b.actionId),
        [fixed.actionId, intent.actionId]);
    expect((syncs.last['settings'] as Map)['actionShortcuts'], hasLength(2));
    expect(c.statusMessage, contains('System key is occupied'));
  });

  test(
      'startup, plugin revoke and restore sync eligibility and refresh action picker',
      () async {
    var available = ['plugin.copy', 'core.directory.terminal'];
    final requests = <List<String>>[];
    final c = controller(
        supported: (ids) async {
          requests.add(ids);
          return available;
        },
        actions: () async => [
              for (final id in available)
                ShortcutActionDto(actionId: id, title: id, contextTypes: [])
            ]);
    addTearDown(c.dispose);
    await c.load(plugins: true);
    expect(requests.single.toSet(), {'plugin.copy', 'core.directory.terminal'});
    expect(syncs.last['supportedActionIds'], available);
    available = ['core.directory.terminal'];
    await c.refreshFeatureData();
    expect(c.shortcutSupported('plugin.copy'), isFalse);
    expect(c.shortcutActions.map((a) => a.actionId), available);
    expect(c.current.actionShortcuts.length, 2);
    expect(syncs.last['supportedActionIds'], available);
    available = ['plugin.copy', 'core.directory.terminal'];
    await c.refreshFeatureData();
    expect(c.shortcutSupported('plugin.copy'), isTrue);
    expect(syncs.length, 3);
    expect(
        stored.actionShortcuts.length, 2); // Never deletes the saved binding.
  });

  test('failed catalog read releases action registrations and reports failure',
      () async {
    var fail = false;
    final c = controller(supported: (ids) async {
      if (fail) throw StateError('catalog unavailable');
      return ids;
    });
    addTearDown(c.dispose);
    await c.load();
    fail = true;
    expect(await c.save(c.current), isFalse);
    expect(syncs.last['supportedActionIds'], isEmpty);
    expect(c.statusMessage, contains('catalog unavailable'));
    expect(c.current.actionShortcuts.length, 2);
    fail = false;
    await c.refreshFeatureData();
    expect(c.statusMessage, isNull);
    expect(syncs.last['supportedActionIds'], isNotEmpty);
  });

  test('pending old configuration cannot sync after a concurrent save',
      () async {
    final pending = Completer<List<String>>();
    final started = Completer<void>();
    var calls = 0;
    final c = controller(supported: (ids) {
      if (calls++ == 0) {
        started.complete();
        return pending.future;
      }
      return Future.value(ids);
    });
    addTearDown(c.dispose);
    final loading = c.load();
    await started.future;
    final saving = c.save(stored.copyWith(actionShortcuts: [intent]));
    // Wait until save readback has replaced the controller's current snapshot.
    while (c.current.actionShortcuts.length != 1) {
      await Future<void>.delayed(Duration.zero);
    }
    pending.complete(['plugin.copy']);
    await loading;
    expect(await saving, isTrue);
    expect(syncs.length, 1);
    expect((syncs.single['settings'] as Map)['actionShortcuts'], hasLength(1));
  });

  test(
      'reload cannot overlap and eligibility still syncs if picker refresh fails',
      () async {
    final gate = Completer<List<PluginStatusDto>>();
    var reloads = 0;
    final c = controller(
        supported: (ids) async => ids,
        refreshPluginReports: () {
          reloads++;
          return gate.future;
        },
        actions: () async => throw StateError('picker unavailable'));
    addTearDown(c.dispose);
    await c.load();
    final refreshing = c.refreshFeatureData();
    expect(c.refreshingPlugins, isTrue);
    await c.refreshFeatureData();
    expect(reloads, 1);
    gate.complete([]);
    await refreshing;
    expect(syncs.length, 2);
    expect(c.refreshingPlugins, isFalse);
    expect(c.statusMessage, contains('picker unavailable'));
  });
}
