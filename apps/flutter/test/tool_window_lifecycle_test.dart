import 'dart:async';

import 'package:anywhere/execution/execution_sessions_controller.dart';
import 'package:anywhere/execution/execution_sessions_screen.dart';
import 'package:anywhere/features/clipboard/clipboard_shelf_app.dart';
import 'package:anywhere/infrastructure/tool_window_lifecycle.dart';
import 'package:anywhere/src/rust/api.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

import 'support/clipboard_fixtures.dart';

class OwnedEditor extends StatefulWidget {
  const OwnedEditor(this.onDispose, {super.key});
  final VoidCallback onDispose;
  @override
  State<OwnedEditor> createState() => _OwnedEditorState();
}

class _OwnedEditorState extends State<OwnedEditor> {
  final input = TextEditingController();
  @override
  Widget build(BuildContext context) => MaterialApp(
      home: Scaffold(body: TextField(controller: input, autofocus: true)));
  @override
  void dispose() {
    input.dispose();
    widget.onDispose();
    super.dispose();
  }
}

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  final messenger =
      TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger;
  const channel = MethodChannel('anywhere/tool-close-test');
  setUp(() {
    messenger.setMockMethodCallHandler(
        channel, (call) async => call.method == 'ready' ? true : null);
  });
  tearDown(() => messenger.setMockMethodCallHandler(channel, null));

  testWidgets('close acknowledgement waits until the editor has unmounted',
      (tester) async {
    var disposed = false;
    var acknowledged = false;
    var disposedWhenAcknowledged = false;
    await tester.pumpWidget(OwnedEditor(() => disposed = true));
    await tester.enterText(find.byType(TextField), 'Synthetic close test');
    unawaited(prepareToolWindowToClose().then((_) {
      disposedWhenAcknowledged = disposed;
      acknowledged = true;
    }));
    expect(acknowledged, isFalse);
    await tester.pumpAndSettle();
    expect(disposed, isTrue);
    expect(acknowledged, isTrue);
    expect(disposedWhenAcknowledged, isTrue);
    expect(find.byType(TextField), findsNothing);
  });

  Future<void> closeFromHost(WidgetTester tester) async {
    var replied = false;
    unawaited(messenger.handlePlatformMessage(
        channel.name,
        const StandardMethodCodec()
            .encodeMethodCall(const MethodCall('prepareToClose')),
        (_) => replied = true));
    await tester.pumpAndSettle();
    expect(replied, isTrue);
  }

  testWidgets('shelf unmount cancels its polling before the host reply',
      (tester) async {
    var reads = 0;
    final backend = History()
      ..searchOverride = (_) async {
        ++reads;
        return ClipboardHistoryDto(
            recording: false, available: true, items: [item('1')]);
      };
    await tester.pumpWidget(ClipboardShelfApp(
        backend: backend,
        channel: channel,
        refreshInterval: const Duration(milliseconds: 100)));
    await tester.pumpAndSettle();
    await closeFromHost(tester);
    final stoppedAt = reads;
    await tester.pump(const Duration(seconds: 2));
    expect(reads, stoppedAt);
    expect(find.byType(ClipboardShelfApp), findsNothing);
    expect(backend.enabled, isFalse);
  });

  testWidgets('task view close stops polling without stopping running tasks',
      (tester) async {
    messenger.setMockMethodCallHandler(channel, (_) async => null);
    var reads = 0;
    var stops = 0;
    final controller = ExecutionSessionsController(ExecutionSessionsBackend(
      list: () async {
        ++reads;
        return [];
      },
      stop: (_) async => ++stops,
    ));
    await tester.pumpWidget(
        ExecutionSessionsApp(channel: channel, controller: controller));
    await tester.pumpAndSettle();
    await closeFromHost(tester);
    final stoppedAt = reads;
    await tester.pump(const Duration(seconds: 10));
    expect(reads, stoppedAt);
    expect(stops, 0);
    expect(find.byType(ExecutionSessionsApp), findsNothing);
  });
}
