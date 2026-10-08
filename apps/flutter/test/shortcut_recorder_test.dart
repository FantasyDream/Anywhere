import 'dart:async';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:anywhere/settings/host_settings.dart';
import 'package:anywhere/settings/shortcut_recorder.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  const channel = MethodChannel('test/shortcut-recorder');
  final requests = <Completer<Map<String, int>?>>[];
  final ids = <int>[];
  final cancellations = <int>[];
  setUp(() {
    requests.clear();
    ids.clear();
    cancellations.clear();
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(channel, (call) async {
      final id = (call.arguments as Map)['id'] as int;
      if (call.method == 'cancel') {
        cancellations.add(id);
        return null;
      }
      ids.add(id);
      final pending = Completer<Map<String, int>?>();
      requests.add(pending);
      return pending.future;
    });
  });
  tearDown(() => TestDefaultBinaryMessengerBinding
      .instance.defaultBinaryMessenger
      .setMockMethodCallHandler(channel, null));

  testWidgets(
      'native chord uses event modifiers without Flutter modifier state',
      (tester) async {
    final recorder = ShortcutRecorder(channel: channel);
    final accepted = <int>[];
    recorder.start(
        onChord: (code, mask) {
          accepted.addAll([code, mask]);
          return true;
        },
        onCancel: () => fail('unexpected cancel'),
        onError: () => fail('unexpected error'));
    await tester.pump();
    expect(recorder.nativePending, isTrue);
    requests.single.complete({'keyCode': 23, 'modifiers': HostSettings.altKey});
    await tester.pump();
    expect(accepted, [23, HostSettings.altKey]);
    expect(recorder.nativePending, isFalse);
  });

  testWidgets('invalid chord records again; native Escape cancels',
      (tester) async {
    final recorder = ShortcutRecorder(channel: channel);
    var cancelled = false;
    recorder.start(
        onChord: (_, mask) => mask != 0,
        onCancel: () => cancelled = true,
        onError: () => fail('unexpected error'));
    await tester.pump();
    requests.first.complete({'keyCode': 23, 'modifiers': 0});
    await tester.pump();
    expect(requests, hasLength(2));
    requests.last.complete(null);
    await tester.pump();
    expect(cancelled, isTrue);
    expect(recorder.nativePending, isFalse);
  });

  testWidgets(
      'late response after dispose cannot change the dialog or newer request',
      (tester) async {
    final recorder = ShortcutRecorder(channel: channel);
    var accepted = 0;
    void start() => recorder.start(
        onChord: (_, __) {
          accepted++;
          return true;
        },
        onCancel: () => fail('late cancel'),
        onError: () => fail('late error'));
    start();
    await tester.pump();
    recorder.cancel();
    start();
    await tester.pump();
    expect(cancellations, [ids.first]);
    requests.first.complete({'keyCode': 23, 'modifiers': HostSettings.altKey});
    await tester.pump();
    expect(accepted, 0);
    expect(recorder.nativePending, isTrue);
    recorder.cancel();
    requests.last.complete(null);
    await tester.pump();
    expect(accepted, 0);
  });

  testWidgets('unsupported native channel leaves Flutter fallback available',
      (tester) async {
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(
            channel, (_) async => throw MissingPluginException());
    final recorder = ShortcutRecorder(channel: channel);
    recorder.start(
        onChord: (_, __) => true,
        onCancel: () => fail('unexpected cancel'),
        onError: () => fail('missing plugin is fallback'));
    await tester.pump();
    expect(recorder.nativePending, isFalse);
  });

  test(
      'Portable event identity follows physical position rather than layout character',
      () {
    expect(
        shortcutEventKeyCode(const KeyDownEvent(
            physicalKey: PhysicalKeyboardKey.keyZ,
            logicalKey: LogicalKeyboardKey.keyY,
            timeStamp: Duration.zero)),
        29);
    expect(
        shortcutEventKeyCode(KeyDownEvent(
            physicalKey: PhysicalKeyboardKey.keyT,
            logicalKey: LogicalKeyboardKey.findKeyByKeyId(0x2020) ??
                const LogicalKeyboardKey(0x2020),
            character: '†',
            timeStamp: Duration.zero)),
        23);
  });
}
