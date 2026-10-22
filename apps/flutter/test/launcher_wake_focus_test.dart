import 'dart:async';
import 'package:anywhere/infrastructure/window_host.dart';
import 'package:anywhere/launcher/launcher_controller.dart';
import 'package:anywhere/launcher/launcher_screen.dart';
import 'package:anywhere/src/rust/api.dart';
import 'package:anywhere/ui/app_style.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'launcher_fixtures.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  const channel = MethodChannel('test/wake-input');
  final messenger =
      TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger;
  tearDown(() => messenger.setMockMethodCallHandler(channel, null));

  for (final mode in [WakeMode.currentApp, WakeMode.universal]) {
    testWidgets(
        '$mode reconnects text after delayed Host show on repeated wakes',
        (tester) async {
      var session = 0;
      var gate = Completer<void>();
      final queries = <String>[];
      final shows = <int>[];
      messenger.setMockMethodCallHandler(channel, (call) async {
        if (call.method == 'show') {
          shows.add((call.arguments as Map)['generation'] as int);
          await gate.future;
        }
        return null;
      });
      LauncherStateDto snapshot(int id, int revision, String input) =>
          launcherSnapshot(
              sessionId: id,
              revision: revision,
              snapshotSeq: 1,
              input: input,
              contextComplete: true,
              candidates: [],
              diagnostics: []);
      final controller = LauncherController(
          backend: LauncherBackend(
            beginActivation: () => ++session,
            activate: (id,
                    {promptAccessibility = false,
                    capturedEnvironment = false}) async =>
                snapshot(id, 0, ''),
            updateInput: (id, input, revision,
                {scopeId, inputMode = InputModeDto.environment}) async {
              queries.add(input);
              return snapshot(id, revision, input);
            },
            execute: (_, __, ___) async => throw StateError('unused'),
            deactivate: (_) async {},
          ),
          window: WindowHost(channel));
      addTearDown(controller.dispose);
      await tester.pumpWidget(MaterialApp(
          theme: AppStyle.theme,
          builder: AppStyle.windowBuilder,
          home: LauncherScreen(controller: controller)));

      for (var wake = 0; wake < 3; wake++) {
        gate = Completer<void>();
        final activation = controller.activate(mode: mode);
        await tester.pump(const Duration(milliseconds: 30));
        await tester.pump();
        expect(shows, hasLength(wake + 1));
        expect(tester.testTextInput.hasAnyClients, isTrue);
        expect(
            tester
                .widget<TextField>(find.byType(TextField))
                .focusNode!
                .hasFocus,
            isTrue);
        final presentation = controller.windowPresentation;
        // Simulate Host show replacing the native keyboard receiver while the
        // Dart client and caret still claim focus. Do not click/enterText.
        tester.testTextInput.hide();
        expect(tester.testTextInput.isVisible, isFalse);
        gate.complete();
        await tester.pumpAndSettle();
        await activation;
        expect(controller.windowPresentation, presentation + 1);
        expect(tester.testTextInput.isVisible, isTrue);
        expect(tester.testTextInput.hasAnyClients, isTrue);
        tester.testTextInput.updateEditingValue(const TextEditingValue(
            text: 'files', selection: TextSelection.collapsed(offset: 5)));
        await tester.pumpAndSettle();
        expect(queries.last, 'files');
        tester.testTextInput.updateEditingValue(const TextEditingValue(
            text: '中文',
            selection: TextSelection.collapsed(offset: 2),
            composing: TextRange(start: 0, end: 2)));
        await tester.pumpAndSettle();
        expect(queries.last, 'files',
            reason: 'IME composition is not submitted');
        tester.testTextInput.updateEditingValue(const TextEditingValue(
            text: '中文', selection: TextSelection.collapsed(offset: 2)));
        await tester.pumpAndSettle();
        expect(queries.last, '中文');
        await controller.dismiss();
        await tester.pumpAndSettle();
        expect(tester.testTextInput.hasAnyClients, isFalse);
      }
      // A delayed show reply cannot reopen editing after dismissal.
      gate = Completer<void>();
      final activation = controller.activate(mode: mode);
      await tester.pump(const Duration(milliseconds: 30));
      await tester.pump();
      final presentation = controller.windowPresentation;
      await controller.dismiss();
      await tester.pumpAndSettle();
      gate.complete();
      await tester.pumpAndSettle();
      await activation;
      expect(controller.windowPresentation, presentation);
      expect(tester.testTextInput.hasAnyClients, isFalse);
      expect(tester.takeException(), isNull);
    });
  }
}
