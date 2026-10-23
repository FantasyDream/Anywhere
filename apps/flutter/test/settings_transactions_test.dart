import 'dart:async';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:anywhere/settings/settings_controller.dart';
import 'package:anywhere/settings/host_settings.dart';
import 'package:anywhere/infrastructure/window_host.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  test('old authorization readback cannot replace a completed save', () async {
    const channel = MethodChannel('boundary-review/readback');
    var host = HostSettings.defaults.copyWith(fileSearchRoots: ['/old']);
    final oldRead = Completer<Map<String, Object?>>();
    final started = Completer<void>();
    var holdNextRead = false;
    final messenger =
        TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger;
    messenger.setMockMethodCallHandler(channel, (call) async {
      if (call.method == 'settings') {
        if (holdNextRead) {
          holdNextRead = false;
          started.complete();
          return oldRead.future;
        }
        return host.toMap();
      }
      if (call.method == 'saveSettings') {
        host = HostSettings.fromMap(call.arguments as Map);
      }
      if (call.method == 'settingsStatus') return <String, Object?>{};
      return null;
    });
    final settings = SettingsController(
        window: WindowHost(channel),
        applyExcludedApps: (_) async {},
        applyContextSources: (_) async {},
        applyFileSearch: (_) async {},
        authorizeFileSearch: (_) async {},
        readExtensions: () => []);
    await settings.load();
    final authorizationStates = <bool>[];
    settings
        .addListener(() => authorizationStates.add(settings.authorizingFiles));
    final captured = host.toMap();
    holdNextRead = true;
    final approval = settings.authorizeFileSearch();
    await started.future;
    final next =
        host.copyWith(fileSearchEnabled: false, fileSearchRoots: ['/new']);
    expect(await settings.save(next), isTrue);
    expect(settings.current.fileSearchEnabled, isFalse);
    oldRead.complete(captured);
    await approval;
    expect(authorizationStates.last, isFalse);
    expect(host.fileSearchEnabled, isFalse);
    expect(settings.current.fileSearchEnabled, isFalse);
    expect(settings.current.fileSearchRoots, ['/new']);
    settings.dispose();
    messenger.setMockMethodCallHandler(channel, null);
  });

  for (final dispose in [false, true]) {
    test(
        'authorization status arriving after ${dispose ? 'dispose' : 'save'} is ignored',
        () async {
      const channel = MethodChannel('boundary-review/status');
      final messenger =
          TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger;
      var host = HostSettings.defaults;
      final status = Completer<Map<String, Object?>>(),
          started = Completer<void>();
      var held = false;
      messenger.setMockMethodCallHandler(channel, (call) async {
        if (call.method == 'settings') return host.toMap();
        if (call.method == 'saveSettings') {
          host = HostSettings.fromMap(call.arguments as Map);
        }
        if (call.method == 'settingsStatus') {
          if (held) {
            held = false;
            started.complete();
            return status.future;
          }
          return <String, Object?>{};
        }
        return null;
      });
      final settings = SettingsController(
          window: WindowHost(channel),
          applyExcludedApps: (_) async {},
          applyContextSources: (_) async {},
          applyFileSearch: (_) async {},
          authorizeFileSearch: (_) async {},
          readExtensions: () => []);
      await settings.load();
      held = true;
      final pending = settings.authorizeFileSearch();
      await started.future;
      if (dispose) {
        settings.dispose();
      } else {
        expect(await settings.save(host.copyWith(fileSearchEnabled: false)),
            isTrue);
      }
      final message = settings.statusMessage;
      status.complete({'fileAccessError': 'obsolete warning'});
      await pending;
      expect(settings.statusMessage, message);
      if (!dispose) {
        expect(settings.current.fileSearchEnabled, isFalse);
        settings.dispose();
      }
      messenger.setMockMethodCallHandler(channel, null);
    });
  }
  test('concurrent direct saves serialize writes and apply their own values',
      () async {
    const channel = MethodChannel('boundary-review/saves');
    final messenger =
        TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger;
    var host = HostSettings.defaults, writes = 0;
    final release = Completer<void>(), started = Completer<void>();
    messenger.setMockMethodCallHandler(channel, (call) async {
      if (call.method == 'settings') return host.toMap();
      if (call.method == 'settingsStatus') return <String, Object?>{};
      if (call.method == 'saveSettings') {
        if (++writes == 1) {
          started.complete();
          await release.future;
        }
        host = HostSettings.fromMap(call.arguments as Map);
      }
      return null;
    });
    final applied = <List<String>>[];
    final settings = SettingsController(
        window: WindowHost(channel),
        applyExcludedApps: (_) async {},
        applyContextSources: (_) async {},
        applyFileSearch: (s) async {
          applied.add(s.fileSearchRoots);
        },
        readExtensions: () => []);
    final first = settings.save(host.copyWith(fileSearchRoots: ['/first']));
    await started.future;
    final second = settings.save(host.copyWith(fileSearchRoots: ['/second']));
    await Future<void>.delayed(Duration.zero);
    expect(writes, 1);
    release.complete();
    expect(await first, isTrue);
    expect(await second, isTrue);
    expect(applied, [
      ['/first'],
      ['/second']
    ]);
    expect(settings.current.fileSearchRoots, ['/second']);
    settings.dispose();
    messenger.setMockMethodCallHandler(channel, null);
  });
}
