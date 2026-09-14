import 'dart:async';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:context_launcher/launcher_screen.dart';
import 'package:context_launcher/src/rust/api.dart';

LauncherStateDto state(int revision, String title) => LauncherStateDto(
      sessionId: 1,
      revision: revision,
      input: '',
      contexts: [],
      diagnostics: [],
      actions: [
        ActionDto(
            id: 'first-$revision',
            commandId: 'first',
            title: title,
            subtitle: 'from Rust',
            iconKey: 'file',
            score: 100),
        ActionDto(
            id: 'second-$revision',
            commandId: 'second',
            title: 'Second action',
            subtitle: null,
            iconKey: 'copy',
            score: 90),
      ],
    );

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  const channel = MethodChannel('test/launcher-window');
  const codec = StandardMethodCodec();
  late List<String> windowCalls;
  late List<MethodCall> windowMessages;
  final messenger =
      TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger;
  setUp(() {
    windowCalls = [];
    windowMessages = [];
    messenger.setMockMethodCallHandler(channel, (call) async {
      windowCalls.add(call.method);
      windowMessages.add(call);
      return null;
    });
  });
  tearDown(() => messenger.setMockMethodCallHandler(channel, null));

  Future<void> toggle(WidgetTester tester) async {
    final done = Completer<void>();
    messenger.handlePlatformMessage(
        channel.name, codec.encodeMethodCall(const MethodCall('toggle')), (_) {
      done.complete();
    });
    await tester.pumpAndSettle();
    await done.future;
  }

  Future<void> dismiss(WidgetTester tester, int generation) async {
    final done = Completer<void>();
    messenger.handlePlatformMessage(
        channel.name,
        codec.encodeMethodCall(
            MethodCall('dismiss', {'generation': generation})),
        (_) => done.complete());
    await tester.pumpAndSettle();
    await done.future;
  }

  int presentedGeneration() =>
      (windowMessages.lastWhere((call) => call.method == 'show').arguments
          as Map)['generation'] as int;

  LauncherStateDto sizedState(int revision, int count) {
    final sample = state(revision, 'Action');
    return LauncherStateDto(
        sessionId: 1,
        revision: revision,
        input: '',
        contexts: [],
        diagnostics: [],
        actions: List.generate(
            count,
            (i) => ActionDto(
                id: '$i-$revision',
                commandId: 'command.$i',
                title: 'Action $i',
                subtitle: sample.actions[0].subtitle,
                iconKey: 'file',
                score: 100.0 - i)));
  }

  testWidgets(
      'empty results collapse and result rows grow up to a scrollable maximum',
      (tester) async {
    await tester.pumpWidget(LauncherApp(
        window: channel,
        activate: () async => sizedState(0, 0),
        updateInput: (_, input, revision) async =>
            sizedState(revision, int.parse(input)),
        execute: (_, action) async => throw StateError('not invoked'),
        deactivate: (_) async {}));
    await toggle(tester);
    expect(find.byKey(const Key('action-list')), findsNothing);
    expect(tester.getSize(find.byKey(const Key('launcher-panel'))).height, 64);
    await tester.enterText(find.byType(TextField), '2');
    await tester.pumpAndSettle();
    final twoRows =
        tester.getSize(find.byKey(const Key('launcher-panel'))).height;
    await tester.enterText(find.byType(TextField), '12');
    await tester.pumpAndSettle();
    final manyRows =
        tester.getSize(find.byKey(const Key('launcher-panel'))).height;
    expect(manyRows, greaterThan(twoRows));
    expect(manyRows, lessThanOrEqualTo(520));
    final list = tester.widget<ListView>(find.byKey(const Key('action-list')));
    expect(list.controller!.position.maxScrollExtent, greaterThan(0));
    await tester.sendKeyEvent(LogicalKeyboardKey.arrowUp);
    await tester.pumpAndSettle();
    expect(find.text('Action 11'), findsOneWidget);
    await tester.enterText(find.byType(TextField), '0');
    await tester.pumpAndSettle();
    expect(find.byKey(const Key('action-list')), findsNothing);
    expect(tester.getSize(find.byKey(const Key('launcher-panel'))).height, 64);
  });

  testWidgets(
      'blur invalidates pending input and stale blur cannot dismiss a fresh activation',
      (tester) async {
    final delayed = Completer<LauncherStateDto?>();
    var captures = 0;
    var invalidations = 0;
    await tester.pumpWidget(LauncherApp(
        window: channel,
        activate: () async => state(0, 'Capture ${++captures}'),
        updateInput: (_, input, revision) => delayed.future,
        execute: (_, action) async => throw StateError('not invoked'),
        deactivate: (_) async {
          invalidations++;
        }));
    await toggle(tester);
    final oldGeneration = presentedGeneration();
    await tester.enterText(find.byType(TextField), 'old input');
    await tester.pump();
    await dismiss(tester, oldGeneration);
    expect(invalidations, 1);
    expect(find.byKey(const Key('action-list')), findsNothing);
    delayed.complete(state(1, 'Expired action'));
    await tester.pumpAndSettle();
    expect(find.text('Expired action'), findsNothing);
    expect(windowCalls.where((method) => method == 'show').length, 1);
    await toggle(tester);
    expect(find.text('Capture 2'), findsOneWidget);
    expect(tester.widget<TextField>(find.byType(TextField)).controller!.text,
        isEmpty);
    await dismiss(tester, oldGeneration);
    expect(invalidations, 1);
    expect(find.text('Capture 2'), findsOneWidget);
  });

  testWidgets(
      'blur during execution cannot reopen or overwrite the next session',
      (tester) async {
    final pending = Completer<ActionResultDto>();
    var captures = 0;
    await tester.pumpWidget(LauncherApp(
        window: channel,
        activate: () async => state(0, 'Capture ${++captures}'),
        updateInput: (_, input, revision) async => state(revision, input),
        execute: (_, action) => pending.future,
        deactivate: (_) async {}));
    await toggle(tester);
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await dismiss(tester, presentedGeneration());
    await toggle(tester);
    pending.complete(ActionResultDto(
        sessionId: 1,
        revision: 0,
        success: true,
        message: 'Done',
        outputText: null,
        closeWindow: true));
    await tester.pumpAndSettle();
    expect(find.text('Capture 2'), findsOneWidget);
    expect(windowCalls, isNot(contains('hide')));
  });

  testWidgets('window waits for capture and ranking before showing',
      (tester) async {
    final captured = Completer<LauncherStateDto?>();
    await tester.pumpWidget(LauncherApp(
      window: channel,
      activate: () => captured.future,
      updateInput: (_, input, revision) async => state(revision, input),
      execute: (_, action) async => throw StateError('not invoked'),
      deactivate: (_) async {},
    ));
    final activation = Completer<void>();
    messenger.handlePlatformMessage(
        channel.name, codec.encodeMethodCall(const MethodCall('toggle')), (_) {
      activation.complete();
    });
    await tester.pump();
    expect(windowCalls, isNot(contains('show')));
    captured.complete(state(0, 'Captured Finder directory'));
    await tester.pumpAndSettle();
    await activation.future;
    expect(windowCalls.last, 'show');
    expect(find.text('Captured Finder directory'), findsOneWidget);
  });

  testWidgets(
      'keyboard navigation, execution, and Esc use the supplied Rust state',
      (tester) async {
    final executed = <String>[];
    var deactivated = 0;
    await tester.pumpWidget(LauncherApp(
      window: channel,
      activate: () async => state(0, 'Open Terminal Here'),
      updateInput: (_, input, revision) async => state(revision, input),
      execute: (session, action) async {
        executed.add(action);
        return ActionResultDto(
            sessionId: session,
            revision: 0,
            success: true,
            message: 'Done',
            outputText: null,
            closeWindow: false);
      },
      deactivate: (_) async {
        deactivated++;
      },
    ));
    await toggle(tester);
    expect(
        windowCalls.indexOf('show'), greaterThan(windowCalls.indexOf('ready')));
    expect(find.text('Open Terminal Here'), findsOneWidget);
    await tester.sendKeyEvent(LogicalKeyboardKey.arrowDown);
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pumpAndSettle();
    expect(executed, ['second-0']);
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pumpAndSettle();
    expect(deactivated, 1);
    expect(windowCalls.last, 'hide');
  });

  testWidgets('out-of-order input responses never replace the latest revision',
      (tester) async {
    final old = Completer<LauncherStateDto?>();
    final fresh = Completer<LauncherStateDto?>();
    await tester.pumpWidget(LauncherApp(
      window: channel,
      activate: () async => state(0, 'Initial action'),
      updateInput: (_, input, revision) =>
          revision == 1 ? old.future : fresh.future,
      execute: (_, action) async => throw StateError('not invoked'),
      deactivate: (_) async {},
    ));
    await toggle(tester);
    await tester.enterText(find.byType(TextField), 'old');
    await tester.enterText(find.byType(TextField), 'fresh');
    fresh.complete(state(2, 'Fresh action'));
    await tester.pumpAndSettle();
    old.complete(state(1, 'Old action'));
    await tester.pumpAndSettle();
    expect(find.text('Fresh action'), findsOneWidget);
    expect(find.text('Old action'), findsNothing);
  });

  testWidgets('late execution response cannot hide a newer input',
      (tester) async {
    final result = Completer<ActionResultDto>();
    await tester.pumpWidget(LauncherApp(
      window: channel,
      activate: () async => state(0, 'Initial action'),
      updateInput: (_, input, revision) async => state(revision, input),
      execute: (_, action) => result.future,
      deactivate: (_) async {},
    ));
    await toggle(tester);
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.enterText(find.byType(TextField), 'New input');
    result.complete(ActionResultDto(
        sessionId: 1,
        revision: 0,
        success: true,
        message: 'Done',
        outputText: null,
        closeWindow: true));
    await tester.pumpAndSettle();
    expect(find.text('New input'), findsWidgets);
    expect(windowCalls, isNot(contains('hide')));
  });
}
