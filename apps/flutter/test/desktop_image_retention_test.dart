import 'dart:ui' as ui;
import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:anywhere/infrastructure/window_host.dart';
import 'package:anywhere/launcher/launcher_controller.dart';
import 'package:anywhere/launcher/launcher_screen.dart';
import 'package:anywhere/launcher/launcher_view_state.dart';
import 'package:anywhere/src/rust/api.dart';
import 'launcher_fixtures.dart';

const _channel = MethodChannel('image-retention-test');

LauncherController _controller(int Function() clock) {
  var issued = 0;
  LauncherStateDto? held;
  LauncherStateDto state(int session, int revision, String input) =>
      held = launcherSnapshot(
          sessionId: session,
          revision: revision,
          snapshotSeq: 1,
          input: input,
          contextComplete: true,
          candidates: [],
          diagnostics: []);
  return LauncherController(
      nowMillis: clock,
      window: WindowHost(_channel),
      backend: LauncherBackend(
          beginActivation: () => ++issued,
          activate: (session,
                  {promptAccessibility = false,
                  capturedEnvironment = false}) async =>
              state(session, 0, ''),
          updateInput: (session, input, revision,
                  {scopeId, inputMode = InputModeDto.environment}) async =>
              state(session, revision, input),
          execute: (_, __, ___) async => throw StateError('unused'),
          deactivate: (_) async {},
          suspend: (_) async {},
          resume: (_) async => held));
}

Future<void> _cacheImage(WidgetTester tester) async {
  final recorder = ui.PictureRecorder();
  Canvas(recorder).drawColor(Colors.blue, BlendMode.src);
  final picture = recorder.endRecording();
  final image = await tester.runAsync(() => picture.toImage(1024, 1024));
  picture.dispose();
  PaintingBinding.instance.imageCache.putIfAbsent(
      'retained-image',
      () => OneFrameImageStreamCompleter(
          SynchronousFuture(ImageInfo(image: image!))));
  await tester.pump();
  expect(PaintingBinding.instance.imageCache.currentSizeBytes, 4 * 1024 * 1024);
}

Future<void> _lifecycle(AppLifecycleState state) =>
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .handlePlatformMessage(SystemChannels.lifecycle.name,
            const StringCodec().encodeMessage(state.toString()), (_) {});

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  setUp(() => TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
      .setMockMethodCallHandler(_channel, (_) async => null));
  tearDown(() {
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(_channel, null);
    PaintingBinding.instance.imageCache.clear();
    PaintingBinding.instance.imageCache.clearLiveImages();
  });

  testWidgets('decoded cache survives quick wake and clears after retention',
      (tester) async {
    var clock = 0;
    final controller = _controller(() => clock);
    await tester
        .pumpWidget(MaterialApp(home: LauncherScreen(controller: controller)));
    await controller.activate(mode: WakeMode.universal);
    await controller.update('kept');
    await tester.pump();
    await _cacheImage(tester);
    await controller.dismiss();
    clock = 14000;
    await tester.pump(const Duration(seconds: 14));
    expect(
        PaintingBinding.instance.imageCache.currentSizeBytes, 4 * 1024 * 1024);
    await controller.activate();
    await tester.pump();
    expect(controller.view.input, 'kept');
    expect(
        PaintingBinding.instance.imageCache.currentSizeBytes, 4 * 1024 * 1024);
    await controller.dismiss();
    clock += 15000;
    await tester.pump(const Duration(seconds: 15));
    await tester.pump();
    expect(controller.hasRetainedState, isFalse);
    expect(PaintingBinding.instance.imageCache.currentSizeBytes, 0);
    controller.dispose();
  });

  testWidgets('idle cache cleanup cannot clear a newly activated window',
      (tester) async {
    var clock = 0;
    final controller = _controller(() => clock);
    await tester
        .pumpWidget(MaterialApp(home: LauncherScreen(controller: controller)));
    await controller.activate(mode: WakeMode.universal);
    await controller.update('kept');
    await tester.pump();
    await _cacheImage(tester);
    // Both transitions happen before the post-frame cleanup can run.
    await controller.dismiss(retain: false);
    await controller.activate(mode: WakeMode.universal);
    await tester.pump();
    expect(controller.view.visible, isTrue);
    expect(
        PaintingBinding.instance.imageCache.currentSizeBytes, 4 * 1024 * 1024);
    controller.dispose();
  });

  testWidgets('editor undo history expires with its retained workspace',
      (tester) async {
    var clock = 0;
    final controller = _controller(() => clock);
    await tester
        .pumpWidget(MaterialApp(home: LauncherScreen(controller: controller)));
    await controller.activate(mode: WakeMode.universal);
    await tester.pump();
    await tester.enterText(find.byType(TextField), 'First payload');
    await tester.pump(const Duration(milliseconds: 600));
    await tester.enterText(find.byType(TextField), 'Second payload');
    await tester.pump(const Duration(milliseconds: 600));
    final history = tester.state<UndoHistoryState<TextEditingValue>>(
        find.byType(UndoHistory<TextEditingValue>));
    expect(history.canUndo, isTrue);
    await controller.dismiss();
    clock += 14000;
    await tester.pump(const Duration(seconds: 14));
    await controller.activate();
    await tester.pump();
    expect(tester.state(find.byType(UndoHistory<TextEditingValue>)),
        same(history));
    expect(history.canUndo, isTrue);
    await controller.dismiss();
    clock += 15000;
    await tester.pump(const Duration(seconds: 15));
    await tester.pump();
    expect(controller.hasRetainedState, isFalse);
    expect(history.mounted, isFalse,
        reason: 'expired queries must not survive in the editor undo stack');
    final fresh = tester.state<UndoHistoryState<TextEditingValue>>(
        find.byType(UndoHistory<TextEditingValue>));
    expect(fresh.canUndo, isFalse);
    controller.dispose();
  });

  testWidgets('expiry releases resources while hidden vsync is paused',
      (tester) async {
    var clock = 0;
    final controller = _controller(() => clock);
    await tester
        .pumpWidget(MaterialApp(home: LauncherScreen(controller: controller)));
    await controller.activate(mode: WakeMode.universal);
    await controller.update('kept');
    await tester.pump();
    await _cacheImage(tester);
    final history = tester.state(find.byType(UndoHistory<TextEditingValue>));
    await controller.dismiss();
    await tester.pump();
    await _lifecycle(AppLifecycleState.hidden);
    try {
      expect(tester.binding.framesEnabled, isFalse);
      clock = 15000;
      await tester.pump(const Duration(seconds: 15));
      expect(controller.hasRetainedState, isFalse);
      expect(history.mounted, isFalse);
      expect(PaintingBinding.instance.imageCache.currentSizeBytes, 0);
      expect(tester.testTextInput.hasAnyClients, isFalse);
      expect(tester.binding.framesEnabled, isFalse,
          reason: 'one cleanup frame must not resume the hidden engine');
    } finally {
      await _lifecycle(AppLifecycleState.resumed);
      controller.dispose();
    }
  });

  testWidgets('retention expiry does not flush the visible Settings cache',
      (tester) async {
    var clock = 0;
    final controller = _controller(() => clock);
    await tester
        .pumpWidget(MaterialApp(home: LauncherScreen(controller: controller)));
    await controller.activate(mode: WakeMode.universal);
    await controller.update('kept');
    await tester.pump();
    await _cacheImage(tester);
    await controller.openSettings();
    await tester.pump();
    clock = 15000;
    await tester.pump(const Duration(seconds: 15));
    await tester.pump();
    expect(controller.hasRetainedState, isFalse);
    expect(controller.view.phase, LauncherPhase.settings);
    expect(
        PaintingBinding.instance.imageCache.currentSizeBytes, 4 * 1024 * 1024);
    controller.dispose();
  });
}
