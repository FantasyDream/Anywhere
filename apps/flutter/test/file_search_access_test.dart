import 'dart:async';
import 'package:anywhere/infrastructure/desktop_environment.dart';
import 'package:anywhere/infrastructure/file_search_access.dart';
import 'package:anywhere/settings/host_settings.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  final messenger =
      TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger;
  setUp(() => DesktopEnvironment.configure(const DesktopEnvironment(
      platform: TargetPlatform.macOS, fileSearchAccess: true)));
  tearDown(() {
    DesktopEnvironment.reset();
    messenger.setMockMethodCallHandler(FileSearchAccess.channel, null);
  });

  test('pending authorization revokes search and publishes only approved roots',
      () async {
    final reply = Completer<Map<String, Object?>>();
    final writes = <(bool, List<String>, bool)>[];
    messenger.setMockMethodCallHandler(FileSearchAccess.channel, (call) {
      expect(call.method, 'prepareFileSearchAccess');
      expect(call.arguments['roots'], ['/Users/me']);
      return reply.future;
    });
    final task = FileSearchAccess.apply(
        HostSettings.defaults.copyWith(fileSearchRoots: ['/Users/me']),
        write: (enabled, roots, restricted) async =>
            writes.add((enabled, roots, restricted)));
    await Future<void>.delayed(Duration.zero);
    expect(writes.length, 1);
    expect(writes.single.$1, false);
    expect(writes.single.$2, isEmpty);
    expect(writes.single.$3, true);
    reply.complete({
      'roots': ['/Users/me/Documents']
    });
    await task;
    expect(writes.last.$1, true);
    expect(writes.last.$2, ['/Users/me/Documents']);
    expect(writes.last.$3, true);
  });

  test('declining all directories never falls back to Home', () async {
    final writes = <(bool, List<String>, bool)>[];
    messenger.setMockMethodCallHandler(
        FileSearchAccess.channel, (_) async => {'roots': []});
    await FileSearchAccess.apply(HostSettings.defaults,
        write: (enabled, roots, restricted) async =>
            writes.add((enabled, roots, restricted)));
    expect(writes.last.$1, false);
    expect(writes.last.$2, isEmpty);
    expect(writes.last.$3, true);
  });

  test('failed authorization leaves the engine disabled', () async {
    final writes = <bool>[];
    messenger.setMockMethodCallHandler(FileSearchAccess.channel,
        (_) async => throw PlatformException(code: 'unavailable'));
    await expectLater(
        FileSearchAccess.apply(HostSettings.defaults,
            write: (enabled, roots, restricted) async => writes.add(enabled)),
        throwsA(isA<PlatformException>()));
    expect(writes, [false]);
  });

  test('hosts without the authorization capability retain their search policy',
      () async {
    DesktopEnvironment.configure(
        const DesktopEnvironment(platform: TargetPlatform.windows));
    messenger.setMockMethodCallHandler(FileSearchAccess.channel,
        (_) async => fail('No macOS authorization call on another host'));
    final writes = <(bool, List<String>, bool)>[];
    await FileSearchAccess.apply(
        HostSettings.defaults.copyWith(fileSearchRoots: [r'C:\Projects']),
        write: (enabled, roots, restricted) async =>
            writes.add((enabled, roots, restricted)));
    expect(writes.single.$1, true);
    expect(writes.single.$2, [r'C:\Projects']);
    expect(writes.single.$3, false);
  });
  test('late approval cannot overwrite a newer disable or narrowed scope',
      () async {
    for (final disabled in [true, false]) {
      final oldReply = Completer<Map<String, Object?>>();
      var calls = 0;
      messenger.setMockMethodCallHandler(FileSearchAccess.channel, (_) {
        return ++calls == 1
            ? oldReply.future
            : Future.value({
                'roots': disabled ? <String>[] : ['/scope/new']
              });
      });
      final writes = <(bool, List<String>)>[];
      Future<void> write(
          bool enabled, List<String> roots, bool restricted) async {
        writes.add((enabled, roots));
      }

      final old = FileSearchAccess.apply(
          HostSettings.defaults.copyWith(fileSearchRoots: ['/scope/old']),
          write: write);
      final stale = expectLater(old, throwsA(isA<SearchAccessSuperseded>()));
      await Future<void>.delayed(Duration.zero);
      await FileSearchAccess.apply(
          HostSettings.defaults.copyWith(
              fileSearchEnabled: !disabled, fileSearchRoots: ['/scope/new']),
          write: write);
      oldReply.complete({
        'roots': ['/scope/old']
      });
      await stale;
      expect(writes.last.$1, !disabled);
      expect(writes.last.$2, disabled ? isEmpty : ['/scope/new']);
      expect(writes.where((v) => v.$1 && v.$2.contains('/scope/old')), isEmpty);
    }
  });
}
