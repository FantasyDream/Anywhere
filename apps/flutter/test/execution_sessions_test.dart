import 'package:anywhere/infrastructure/desktop_environment.dart';
import 'dart:async';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:anywhere/execution/execution_sessions_controller.dart';
import 'package:anywhere/execution/execution_sessions_screen.dart';
import 'package:anywhere/execution/execution_output_view.dart';
import 'package:anywhere/execution/execution_command.dart';
import 'package:anywhere/execution/execution_sessions_view.dart';
import 'package:anywhere/ui/app_style.dart';
import 'package:anywhere/src/rust/api.dart';

ExecutionSessionSummaryDto summary(String id,
        {ExecutionSessionStatusDto status = ExecutionSessionStatusDto.running,
        bool truncated = false,
        int outputRevision = 0,
        String title = 'Cargo Check',
        String directory = '/tmp/project'}) =>
    ExecutionSessionSummaryDto(
        id: id,
        title: title,
        directory: directory,
        command: const ['/tmp/cargo', 'check'],
        status: status,
        startedAtMs: 1,
        outputRevision: outputRevision,
        outputTruncated: truncated,
        exitCode: status == ExecutionSessionStatusDto.failed ? 7 : null);
ExecutionSessionDetailDto detail(String id,
        {ExecutionSessionStatusDto status = ExecutionSessionStatusDto.running,
        bool truncated = false}) =>
    ExecutionSessionDetailDto(
        summary: summary(id, status: status, truncated: truncated),
        stdout: 'Output $id',
        stderr: 'Diagnostic $id');

void main() {
  setUp(() => DesktopEnvironment.configure(
      const DesktopEnvironment(platform: TargetPlatform.macOS)));
  tearDown(DesktopEnvironment.reset);
  TestWidgetsFlutterBinding.ensureInitialized();

  test('unchanged polls retain output and do not rebuild the task window',
      () async {
    var outputReads = 0;
    var notifications = 0;
    var revision = 1;
    var status = ExecutionSessionStatusDto.running;
    final controller = ExecutionSessionsController(ExecutionSessionsBackend(
      // Fresh command lists reproduce allocations made by the Rust bridge.
      list: () async =>
          [summary('a', outputRevision: revision, status: status)],
      detail: (_) async {
        outputReads++;
        return ExecutionSessionDetailDto(
            summary: summary('a', outputRevision: revision, status: status),
            stdout: 'x' * (128 * 1024),
            stderr: '');
      },
    ));
    controller.addListener(() => notifications++);
    controller.setVisible(true);
    await Future<void>.delayed(Duration.zero);
    final originalOutput = controller.detail;
    final originalRows = controller.visibleSessions;
    notifications = 0;
    for (var poll = 0; poll < 100; poll++) {
      await controller.refresh();
    }
    expect(outputReads, 1);
    expect(notifications, 0);
    expect(identical(controller.detail, originalOutput), isTrue);
    expect(identical(controller.visibleSessions, originalRows), isTrue);
    expect(controller.pollInterval,
        ExecutionSessionsController.activePollInterval);
    revision++;
    await controller.refresh();
    expect(outputReads, 2);
    expect(controller.detail?.summary.outputRevision, 2);
    status = ExecutionSessionStatusDto.succeeded;
    await controller.refresh();
    expect(outputReads, 3);
    expect(controller.detail?.summary.status, status);
    expect(
        controller.pollInterval, ExecutionSessionsController.idlePollInterval);
    controller.dispose();
  });

  test(
      'completed task polling still discovers new tasks and filtering is cached',
      () async {
    var tasks = [summary('a', status: ExecutionSessionStatusDto.succeeded)];
    final controller = ExecutionSessionsController(ExecutionSessionsBackend(
      list: () async => tasks,
      detail: (id) async => ExecutionSessionDetailDto(
          summary: tasks.firstWhere((task) => task.id == id),
          stdout: id,
          stderr: ''),
    ));
    controller.setVisible(true);
    await Future<void>.delayed(Duration.zero);
    controller.setFilter('rust project');
    expect(controller.visibleSessions, isEmpty);
    tasks = [summary('b', title: 'Rust Check'), ...tasks];
    await controller.refresh();
    await Future<void>.delayed(Duration.zero);
    expect(controller.visibleSessions.single.id, 'b');
    expect(controller.selectedId, 'b');
    expect(controller.detail?.stdout, 'b');
    expect(identical(controller.visibleSessions, controller.visibleSessions),
        isTrue);
    expect(controller.pollInterval,
        ExecutionSessionsController.activePollInterval);
    controller.dispose();
  });

  test('empty task lists and idempotent visibility do not generate UI updates',
      () async {
    var reads = 0, updates = 0;
    final controller = ExecutionSessionsController(ExecutionSessionsBackend(
      list: () async {
        reads++;
        return [];
      },
    ));
    controller.addListener(() => updates++);
    controller.setVisible(true);
    await Future<void>.delayed(Duration.zero);
    for (var poll = 0; poll < 10; poll++) {
      await controller.refresh();
      controller.setVisible(true);
    }
    expect(reads, 11);
    expect(updates, 0);
    controller.dispose();
  });

  test('late output from a previous selection cannot replace the current task',
      () async {
    final first = Completer<ExecutionSessionDetailDto?>();
    var firstRead = false;
    final controller = ExecutionSessionsController(ExecutionSessionsBackend(
      list: () async => [summary('a'), summary('b')],
      detail: (id) {
        if (id == 'a') {
          firstRead = true;
          return first.future;
        }
        return Future.value(detail(id));
      },
    ));
    controller.setVisible(true);
    await Future<void>.delayed(Duration.zero);
    expect(firstRead, isTrue);
    controller.select('b');
    first.complete(detail('a'));
    await Future<void>.delayed(Duration.zero);
    expect(controller.selectedId, 'b');
    expect(controller.detail?.summary.id, 'b');
    controller.dispose();
  });

  test('hidden task window rejects in-flight output and does not poll',
      () async {
    final pending = Completer<ExecutionSessionDetailDto?>();
    var reads = 0;
    final controller = ExecutionSessionsController(ExecutionSessionsBackend(
      list: () async {
        reads++;
        return [summary('a')];
      },
      detail: (_) => pending.future,
    ));
    controller.setVisible(true);
    await Future<void>.delayed(Duration.zero);
    controller.setVisible(false);
    pending.complete(detail('a'));
    await Future<void>.delayed(const Duration(milliseconds: 550));
    expect(controller.detail, isNull);
    expect(reads, 1);
    controller.dispose();
  });

  test('failed stop reports a recoverable error and retains the task',
      () async {
    final controller = ExecutionSessionsController(ExecutionSessionsBackend(
      list: () async => [summary('a')],
      detail: (id) async => detail(id),
      stop: (_) async => throw StateError('failure'),
    ));
    controller.setVisible(true);
    await Future<void>.delayed(Duration.zero);
    await controller.stopSelected();
    expect(controller.error, contains('could not be updated'));
    expect(controller.detail?.summary.id, 'a');
    expect(controller.busy, isFalse);
    await controller.refresh();
    expect(controller.error, contains('could not be updated'));
    controller.dismissError();
    expect(controller.error, isNull);
    controller.dispose();
  });

  testWidgets(
      'visibility changes during startup survive the delayed ready response',
      (tester) async {
    const channel = MethodChannel('test/executions-startup');
    final ready = Completer<String?>();
    final messenger =
        TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger;
    messenger.setMockMethodCallHandler(channel, (_) => ready.future);
    addTearDown(() => messenger.setMockMethodCallHandler(channel, null));
    var reads = 0;
    final controller = ExecutionSessionsController(ExecutionSessionsBackend(
      list: () async {
        reads++;
        return [];
      },
    ));
    await tester.pumpWidget(
        ExecutionSessionsApp(controller: controller, channel: channel));
    await messenger.handlePlatformMessage(
        channel.name,
        const StandardMethodCodec()
            .encodeMethodCall(const MethodCall('visibility', false)),
        (_) {});
    ready.complete(null);
    await tester.pump();
    await tester.pump(const Duration(seconds: 1));
    expect(reads, 0);
    await messenger.handlePlatformMessage(
        channel.name,
        const StandardMethodCodec()
            .encodeMethodCall(const MethodCall('visibility', true)),
        (_) {});
    await tester.pump();
    expect(reads, 1);
    await tester.pumpWidget(const SizedBox());
  });

  testWidgets(
      'task view shows failures, bounded-output notice and remove control',
      (tester) async {
    tester.view.physicalSize = const Size(920, 600);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    const channel = MethodChannel('test/executions');
    final messenger =
        TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger;
    messenger.setMockMethodCallHandler(channel, (_) async => 'a');
    addTearDown(() => messenger.setMockMethodCallHandler(channel, null));
    var removed = false;
    final controller = ExecutionSessionsController(ExecutionSessionsBackend(
      list: () async => removed
          ? []
          : [summary('a', status: ExecutionSessionStatusDto.failed)],
      detail: (id) async =>
          detail(id, status: ExecutionSessionStatusDto.failed, truncated: true),
      remove: (_) async {
        removed = true;
      },
    ));
    await tester.pumpWidget(
        ExecutionSessionsApp(controller: controller, channel: channel));
    await tester.pump();
    expect(find.text('Cargo Check'), findsNWidgets(2));
    expect(find.text('Failed'), findsNWidgets(2));
    expect(find.text('Exit 7'), findsNWidgets(2));
    expect(find.text('Diagnostic a'), findsOneWidget);
    expect(find.textContaining('Showing the latest output'), findsOneWidget);
    expect(find.text('Stop'), findsNothing);
    await tester.sendKeyDownEvent(LogicalKeyboardKey.metaLeft);
    await tester.sendKeyEvent(LogicalKeyboardKey.keyK);
    await tester.sendKeyUpEvent(LogicalKeyboardKey.metaLeft);
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 300));
    await tester.tap(find.text('Remove result'));
    await tester.pump(const Duration(milliseconds: 300));
    expect(removed, isTrue);
    expect(find.textContaining('No tasks yet.'), findsOneWidget);
    expect(tester.takeException(), isNull);
    await tester.pumpWidget(const SizedBox());
  });
  testWidgets(
      'long task metadata keeps output and Stop usable at minimum window size',
      (tester) async {
    tester.view.physicalSize = const Size(760, 420);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    const channel = MethodChannel('test/executions-compact');
    final messenger =
        TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger;
    messenger.setMockMethodCallHandler(channel, (_) async => 'long');
    addTearDown(() => messenger.setMockMethodCallHandler(channel, null));
    final task = ExecutionSessionSummaryDto(
        id: 'long',
        title: 'pnpm Run · ${List.filled(12, 'check:types').join(':')}',
        directory:
            '/Users/example/${List.filled(30, 'project folder').join('/')}',
        command: ['pnpm', 'run', List.filled(30, 'long argument').join(' ')],
        status: ExecutionSessionStatusDto.running,
        startedAtMs: 1,
        outputRevision: 0,
        outputTruncated: false);
    var stopped = false;
    final controller = ExecutionSessionsController(ExecutionSessionsBackend(
      list: () async => [task],
      detail: (_) async => ExecutionSessionDetailDto(
          summary: task, stdout: 'Ready for requests', stderr: ''),
      stop: (_) async {
        stopped = true;
      },
    ));
    await tester.pumpWidget(
        ExecutionSessionsApp(controller: controller, channel: channel));
    await tester.pump();
    await tester.tap(find.text('Command details'));
    await tester.pump(const Duration(milliseconds: 300));
    expect(tester.takeException(), isNull);
    expect(find.text('Ready for requests'), findsOneWidget);
    await tester.tap(find.text('Stop'));
    await tester.pump();
    expect(stopped, isTrue);
    expect(tester.takeException(), isNull);
    await tester.pumpWidget(const SizedBox());
  });
  testWidgets('minimum task window supports larger text and long project names',
      (tester) async {
    tester.view.physicalSize = const Size(760, 420);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    final task = summary('large',
        status: ExecutionSessionStatusDto.failed,
        title: 'Build a project with a deliberately long task title',
        directory: '/tmp/a-project-with-a-deliberately-long-name');
    final controller = ExecutionSessionsController(ExecutionSessionsBackend(
        list: () async => [task],
        detail: (_) async => ExecutionSessionDetailDto(
            summary: task,
            stdout: 'Build output',
            stderr: 'Build diagnostic')));
    addTearDown(controller.dispose);
    controller.setVisible(true);
    await tester.pumpWidget(MaterialApp(
        theme: AppStyle.theme,
        builder: (context, child) => MediaQuery(
            data: MediaQuery.of(context)
                .copyWith(textScaler: const TextScaler.linear(1.4)),
            child: child!),
        home: ListenableBuilder(
            listenable: controller,
            builder: (_, __) => ExecutionSessionsView(
                controller: controller,
                onOpenLauncher: () async {},
                onHide: () async {}))));
    await tester.pump();
    expect(tester.takeException(), isNull);
    expect(find.text('Build diagnostic'), findsOneWidget);
    await tester.tap(find.text('Output'));
    await tester.pump();
    expect(find.text('Build output'), findsOneWidget);
    expect(tester.takeException(), isNull);
    controller.setVisible(false);
    await tester.pumpWidget(const SizedBox());
  });

  test('task filtering and explicit action targets preserve user intent',
      () async {
    var tasks = [
      summary('a',
          status: ExecutionSessionStatusDto.succeeded, directory: '/tmp/alpha'),
      summary('b',
          status: ExecutionSessionStatusDto.succeeded,
          title: 'npm Test',
          directory: '/tmp/beta')
    ];
    final removed = <String>[];
    final controller = ExecutionSessionsController(ExecutionSessionsBackend(
      list: () async => tasks,
      detail: (id) async =>
          detail(id, status: ExecutionSessionStatusDto.succeeded),
      remove: (id) async {
        removed.add(id);
        tasks = tasks.where((task) => task.id != id).toList();
      },
    ));
    controller.setVisible(true);
    await Future<void>.delayed(Duration.zero);
    controller.setFilter('NPM beta');
    await Future<void>.delayed(Duration.zero);
    expect(controller.visibleSessions.map((s) => s.id), ['b']);
    controller.select(
        'a'); // A newly opened task must not be hidden by an old filter.
    expect(controller.filter, '');
    controller.select('b');
    await controller.remove(
        'a'); // An open menu belongs to A even after selection changes to B.
    expect(removed, ['a']);
    expect(controller.selectedId, 'b');
    controller.dispose();
  });

  testWidgets(
      'keyboard task navigation, search, cancellation and window actions are discoverable',
      (tester) async {
    tester.view.physicalSize = const Size(920, 600);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    const channel = MethodChannel('test/ergonomic-actions');
    final calls = <String>[];
    final messenger =
        TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger;
    messenger.setMockMethodCallHandler(channel, (call) async {
      calls.add(call.method);
      return call.method == 'ready' ? 'a' : null;
    });
    addTearDown(() => messenger.setMockMethodCallHandler(channel, null));
    final stopped = <String>[];
    final controller = ExecutionSessionsController(ExecutionSessionsBackend(
      list: () async => [
        summary('a'),
        summary('b',
            status: ExecutionSessionStatusDto.failed, title: 'npm Test')
      ],
      detail: (id) async => detail(id,
          status: id == 'a'
              ? ExecutionSessionStatusDto.running
              : ExecutionSessionStatusDto.failed),
      stop: (id) async {
        stopped.add(id);
      },
    ));
    Future<void> command(LogicalKeyboardKey key) async {
      await tester.sendKeyDownEvent(LogicalKeyboardKey.metaLeft);
      await tester.sendKeyEvent(key);
      await tester.sendKeyUpEvent(LogicalKeyboardKey.metaLeft);
      await tester.pump();
    }

    await tester.pumpWidget(
        ExecutionSessionsApp(controller: controller, channel: channel));
    await tester.pump();
    await tester.tap(find.byKey(const ValueKey('task:a')));
    await tester.sendKeyEvent(LogicalKeyboardKey.arrowDown);
    await tester.pump();
    expect(controller.selectedId, 'b');
    await command(LogicalKeyboardKey.period);
    expect(stopped,
        isEmpty); // No cancelling a completed task; Return also does not remove it.
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pump();
    expect(controller.selectedId, 'b');
    await tester.sendKeyEvent(LogicalKeyboardKey.arrowUp);
    await tester.pump();
    await command(LogicalKeyboardKey.period);
    expect(stopped, ['a']);
    await command(LogicalKeyboardKey.keyF);
    expect(
        tester
            .widget<TextField>(find.byKey(const Key('task-search')))
            .focusNode!
            .hasFocus,
        isTrue);
    await tester.enterText(find.byKey(const Key('task-search')), 'missing');
    await tester.pump();
    expect(find.text('No matching tasks'), findsOneWidget);
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pump();
    expect(controller.filter, '');
    expect(calls, isNot(contains('hide')));
    // AppKit menu key equivalents dispatch through the native channel rather
    // than synthesizing Flutter key events (notably macOS's cancelOperation).
    Future<void> nativeCommand(String method) async {
      await messenger.handlePlatformMessage(
          channel.name,
          const StandardMethodCodec().encodeMethodCall(MethodCall(method)),
          (_) {});
      await tester.pump();
    }

    await nativeCommand('find');
    expect(
        tester
            .widget<TextField>(find.byKey(const Key('task-search')))
            .focusNode!
            .hasFocus,
        isTrue);
    await nativeCommand('stop');
    expect(stopped, ['a', 'a']);
    await nativeCommand('refresh');
    expect(controller.selectedId, 'a');
    await command(LogicalKeyboardKey.keyL);
    expect(calls, contains('showLauncher'));
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pump();
    expect(calls, contains('hide'));
    expect(tester.takeException(), isNull);
    await tester.pumpWidget(const SizedBox());
  });

  testWidgets(
      'live output follows the tail but preserves the reader position until Latest',
      (tester) async {
    final text = List.generate(80, (i) => 'Line $i').join('\n');
    Widget view(String output) => MaterialApp(
        home: Scaffold(
            body: SizedBox(
                height: 220,
                child: ExecutionOutputView(
                    detail: ExecutionSessionDetailDto(
                        summary: summary('a'), stdout: output, stderr: ''),
                    onError: (_) {}))));
    await tester.pumpWidget(view(text));
    await tester.pump();
    ScrollController scroll() => tester
        .widget<SingleChildScrollView>(
            find.byKey(const Key('task-output-scroll')))
        .controller!;
    expect(scroll().position.extentAfter, 0);
    await tester.drag(
        find.byKey(const Key('task-output-scroll')), const Offset(0, 220));
    await tester.pump();
    final readingAt = scroll().offset;
    expect(find.text('Latest'), findsOneWidget);
    await tester.pumpWidget(view('$text\nNew output'));
    await tester.pump();
    expect(scroll().offset, closeTo(readingAt, 1));
    await tester.tap(find.byKey(const Key('follow-output')));
    await tester.pump();
    expect(scroll().position.extentAfter, 0);
    await tester.pumpWidget(view('$text\nNew output\nAnother line'));
    await tester.pump();
    expect(scroll().position.extentAfter, 0);
    expect(tester.takeException(), isNull);
    await tester.pumpWidget(const SizedBox());
  });

  test('copied commands preserve literal spaces and shell metacharacters', () {
    expect(
        formatExecutionCommand(['/tmp/tool path', 'run', "it's a \$(literal)"]),
        "'/tmp/tool path' run 'it'\"'\"'s a \$(literal)'");
  });
  testWidgets(
      'failed tasks open diagnostics and Copy uses only the selected stream',
      (tester) async {
    final messenger =
        TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger;
    String? copied;
    messenger.setMockMethodCallHandler(SystemChannels.platform, (call) async {
      if (call.method == 'Clipboard.setData') {
        copied = (call.arguments as Map)['text'] as String;
      }
      return null;
    });
    addTearDown(() =>
        messenger.setMockMethodCallHandler(SystemChannels.platform, null));
    await tester.pumpWidget(MaterialApp(
        home: Scaffold(
            body: SizedBox(
                height: 220,
                child: ExecutionOutputView(
                    detail: detail('failed',
                        status: ExecutionSessionStatusDto.failed),
                    onError: (_) {})))));
    await tester.pump();
    expect(find.text('Diagnostic failed'), findsOneWidget);
    expect(find.text('Output failed'), findsNothing);
    await tester.tap(find.text('Output'));
    await tester.pump();
    await tester.tap(find.byTooltip('Copy output'));
    await tester.pump();
    expect(copied, 'Output failed');
    expect(find.byTooltip('Copied'), findsOneWidget);
    expect(tester.takeException(), isNull);
    await tester.pumpWidget(const SizedBox());
  });
}
