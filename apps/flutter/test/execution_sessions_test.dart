import 'dart:async';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:anywhere/execution/execution_sessions_controller.dart';
import 'package:anywhere/execution/execution_sessions_screen.dart';
import 'package:anywhere/src/rust/api.dart';

ExecutionSessionSummaryDto summary(String id,
        {ExecutionSessionStatusDto status = ExecutionSessionStatusDto.running,
        bool truncated = false}) =>
    ExecutionSessionSummaryDto(
        id: id,
        title: 'Cargo Check',
        directory: '/tmp/project',
        command: const ['/tmp/cargo', 'check'],
        status: status,
        startedAtMs: 1,
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
  TestWidgetsFlutterBinding.ensureInitialized();

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
    controller.dispose();
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
    expect(find.text('Cargo Check · Failed (7)'), findsOneWidget);
    expect(find.text('Diagnostic a'), findsOneWidget);
    expect(find.textContaining('Showing the latest output'), findsOneWidget);
    expect(find.text('Stop'), findsNothing);
    await tester.tap(find.text('Remove'));
    await tester.pump();
    expect(removed, isTrue);
    expect(find.textContaining('No tasks yet.'), findsOneWidget);
    expect(tester.takeException(), isNull);
    await tester.pumpWidget(const SizedBox());
  });
}
