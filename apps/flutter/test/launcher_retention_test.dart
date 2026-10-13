import 'dart:async';
import 'package:flutter_test/flutter_test.dart';
import 'package:flutter/services.dart';
import 'package:anywhere/launcher/launcher_controller.dart';
import 'package:anywhere/launcher/launcher_view_state.dart';
import 'package:anywhere/infrastructure/window_host.dart';
import 'package:anywhere/src/rust/api.dart';
import 'launcher_fixtures.dart';
import 'package:anywhere/launcher/file_search_controller.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  const channel = MethodChannel('retention-test');
  setUp(() => TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
      .setMockMethodCallHandler(channel, (_) async => null));
  tearDown(() => TestDefaultBinaryMessengerBinding
      .instance.defaultBinaryMessenger
      .setMockMethodCallHandler(channel, null));
  testWidgets(
      'native focus loss resumes the same session and expires after 15s',
      (tester) async {
    var issued = 0, clock = 0, expired = 0, suspended = 0;
    int? presentedGeneration;
    LauncherStateDto? state;
    final messenger =
        TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger;
    messenger.setMockMethodCallHandler(channel, (call) async {
      if (call.method == 'show') {
        presentedGeneration = (call.arguments as Map)['generation'] as int;
      }
      return null;
    });
    Future<void> native(String method, Map<String, Object?> args) async {
      final done = Completer<void>();
      unawaited(messenger.handlePlatformMessage(
          channel.name,
          const StandardMethodCodec()
              .encodeMethodCall(MethodCall(method, args)), (data) {
        try {
          const StandardMethodCodec().decodeEnvelope(data!);
          done.complete();
        } catch (error, stack) {
          done.completeError(error, stack);
        }
      }));
      await done.future;
    }

    LauncherStateDto make(int session, int revision, String input) =>
        launcherSnapshot(
            sessionId: session,
            revision: revision,
            snapshotSeq: 1,
            input: input,
            contextComplete: true,
            candidates: [],
            diagnostics: []);
    final controller = LauncherController(
      nowMillis: () => clock,
      backend: LauncherBackend(
          beginActivation: () => ++issued,
          activate: (session,
                  {promptAccessibility = false,
                  capturedEnvironment = false}) async =>
              state = make(session, 0, ''),
          updateInput: (session, input, revision,
                  {scopeId, inputMode = InputModeDto.environment}) async =>
              state = make(session, revision, input),
          execute: (_, __, ___) async => throw StateError('not used'),
          deactivate: (_) async {
            expired++;
          },
          suspend: (_) async {
            suspended++;
          },
          resume: (_) async => state),
      window: WindowHost(channel),
      applyFileSearch: (_) async {},
    );
    controller.attach();
    await tester.pump();
    await controller.activate(mode: WakeMode.universal);
    await controller.update('kept after clicking desktop');
    final originalGeneration = presentedGeneration!;
    await native('dismiss', {'generation': originalGeneration});
    expect(controller.hasRetainedState, isTrue);
    expect(controller.view.phase, LauncherPhase.hidden);
    expect(expired, 0);
    clock = 14000;
    await tester.pump(const Duration(seconds: 14));
    await native('toggle', {'mode': 'currentApp'});
    expect(issued, 1);
    expect(controller.view.searchText, 'kept after clicking desktop');
    expect(controller.view.phase, LauncherPhase.ready);
    await native('dismiss', {'generation': originalGeneration});
    expect(controller.view.phase, LauncherPhase.ready,
        reason: 'late focus event cannot hide the restored presentation');
    await native('dismiss', {'generation': presentedGeneration!});
    clock += 15000;
    await tester.pump(const Duration(seconds: 15));
    await tester.pump();
    expect(expired, 1);
    await native('toggle', {'mode': 'currentApp'});
    expect(issued, 2);
    expect(controller.view.searchText, isEmpty);
    expect(suspended, 2);
    controller.dispose();
  });
  testWidgets(
      'hide resumes query, type, preview and parent within 15 seconds; expiry starts fresh',
      (tester) async {
    var issued = 0, clock = 0, suspended = 0, expired = 0;
    LauncherStateDto? state;
    LauncherStateDto make(int session, int revision, String input) =>
        launcherSnapshot(
            sessionId: session,
            revision: revision,
            snapshotSeq: 1,
            input: input,
            contextComplete: true,
            candidates: [],
            diagnostics: []);
    final controller = LauncherController(
      nowMillis: () => clock,
      backend: LauncherBackend(
        beginActivation: () => ++issued,
        activate: (session,
                {promptAccessibility = false,
                capturedEnvironment = false}) async =>
            state = make(session, 0, ''),
        updateInput: (session, input, revision,
                {scopeId, inputMode = InputModeDto.environment}) async =>
            state = make(session, revision, input),
        updateFiles: (session, input, revision, filter) async =>
            state = make(session, revision, input),
        execute: (session, revision, candidate) async =>
            throw StateError('not used'),
        deactivate: (_) async {
          expired++;
        },
        suspend: (_) async {
          suspended++;
        },
        resume: (_) async => state,
      ),
      window: WindowHost(channel),
      applyFileSearch: (_) async {},
    );
    await controller.activate(mode: WakeMode.universal);
    await controller.update('parent query');
    await controller.openFileSearch();
    controller.files.setFilter(FileFilter.code);
    await tester.pump();
    await controller.update('settings');
    await controller.dismiss();
    expect(controller.view.phase, LauncherPhase.hidden);
    expect(controller.hasRetainedState, isTrue);
    expect(expired, 0);
    clock = 14000;
    await tester.pump(const Duration(seconds: 14));
    await controller.activate(mode: WakeMode.currentApp);
    expect(issued, 1);
    expect(controller.view.searchText, 'settings');
    expect(controller.view.isFileSearch, isTrue);
    expect(controller.files.filter, FileFilter.code);
    expect(controller.view.hasFilePreview, isTrue);
    await tester.pump(const Duration(seconds: 1));
    expect(expired, 0, reason: 'old expiry cannot clear resumed state');
    await controller.closeFileSearch();
    expect(controller.view.searchText, 'parent query');
    await controller.dismiss();
    clock += 15000;
    await tester.pump(const Duration(seconds: 15));
    await tester.pump();
    expect(controller.hasRetainedState, isFalse);
    expect(controller.view.snapshot, isNull);
    expect(controller.view.searchText, isEmpty);
    expect(expired, 1);
    await controller.activate(mode: WakeMode.universal);
    expect(issued, 2);
    expect(controller.view.searchText, isEmpty);
    expect(suspended, 2);
    controller.dispose();
  });
}
