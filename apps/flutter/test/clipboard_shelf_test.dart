import 'dart:async';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:anywhere/infrastructure/desktop_environment.dart';
import 'package:anywhere/src/rust/api.dart';
import 'package:anywhere/features/clipboard/clipboard_history_page.dart';
import 'package:anywhere/features/clipboard/clipboard_history_session.dart';
import 'package:anywhere/features/clipboard/clipboard_shelf_app.dart';
import 'package:anywhere/tools/tool_page.dart';
import 'support/clipboard_fixtures.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  const channel = MethodChannel('anywhere/test-shelf');
  final messenger =
      TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger;
  final calls = <String>[];
  setUp(() {
    calls.clear();
    DesktopEnvironment.configure(
        const DesktopEnvironment(platform: TargetPlatform.macOS));
    messenger.setMockMethodCallHandler(channel, (call) async {
      calls.add(call.method);
      return call.method == 'ready' ? true : null;
    });
  });
  tearDown(() {
    messenger.setMockMethodCallHandler(channel, null);
    DesktopEnvironment.reset();
  });

  test(
      'window registrations reject duplicate identities and route only native windows',
      () async {
    var shown = 0;
    final registration = ToolWindowRegistration(
        id: 'clipboard.shelf',
        show: () async {
          ++shown;
        });
    expect(() => ToolPageRegistry([], windows: [registration, registration]),
        throwsArgumentError);
    expect(
        () => ToolPageRegistry([
              ToolPageRegistration(
                  id: registration.id, open: (_) => throw StateError('unused'))
            ], windows: [
              registration
            ]),
        throwsArgumentError);
    final registry = ToolPageRegistry([], windows: [registration]);
    const result = ActionResultDto(
        sessionId: 1,
        revision: 1,
        status: ActionStatusDto.succeeded,
        closeWindow: false,
        view: HostViewDto.nativeTool,
        nativeTool: NativeToolDto(id: 'clipboard.shelf', input: ''));
    await registry.windowFor(result)!();
    expect(shown, 1);
    expect(registry.hostViews, contains(HostViewDto.nativeTool));
  });

  test(
      'unchanged background history does not decode or notify; new copies preserve selection',
      () async {
    final backend = History();
    var details = 0;
    backend.detailOverride = (id) async {
      ++details;
      return previewDetail(item(id), id);
    };
    final session = ClipboardHistorySession(backend);
    await Future<void>.delayed(Duration.zero);
    await session.select(1);
    final previousDetails = details;
    var notifications = 0;
    session.addListener(() => ++notifications);
    await session.refresh(background: true);
    expect(details, previousDetails);
    expect(notifications, 0);
    backend.rows = [item('3'), ...backend.rows];
    await session.refresh(background: true);
    expect(session.selected!.id, '2');
    expect(session.items.length, 3);
    expect(details, previousDetails,
        reason: 'New rows do not re-decode the selected body');
    expect(backend.enabled, isFalse);
    session.dispose();
  });

  Future<void> host(String method, [Object? arguments]) =>
      messenger.handlePlatformMessage(
          channel.name,
          const StandardMethodCodec()
              .encodeMethodCall(MethodCall(method, arguments)),
          (_) {});

  testWidgets(
      'shelf copies repeatedly, keeps its query and closes only explicitly',
      (tester) async {
    tester.view.physicalSize = const Size(820, 540);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    final backend = History();
    await tester
        .pumpWidget(ClipboardShelfApp(backend: backend, channel: channel));
    await tester.pumpAndSettle();
    await tester.enterText(find.byKey(const Key('clipboard-search')), 'Entry');
    await tester.pump(const Duration(milliseconds: 150));
    await tester.pumpAndSettle();
    await tester.sendKeyEvent(LogicalKeyboardKey.arrowDown);
    await tester.pumpAndSettle();
    final selectedTop = tester.getTopLeft(find.text('Entry 2')).dy;
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pumpAndSettle();
    expect(tester.getTopLeft(find.text('Entry 2')).dy, selectedTop);
    expect(find.text('2 entries · Paused · Copied'), findsOneWidget);
    expect(backend.copied, ['2']);
    expect(calls, contains('returnToTarget'));
    expect(calls, isNot(contains('close')));
    expect(find.byType(ClipboardHistoryPage), findsOneWidget);
    expect(
        tester
            .widget<TextField>(find.byKey(const Key('clipboard-search')))
            .controller!
            .text,
        'Entry');
    await tester.sendKeyEvent(LogicalKeyboardKey.arrowUp);
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pumpAndSettle();
    expect(backend.copied, ['2', '1']);
    expect(backend.enabled, isFalse);
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pumpAndSettle();
    expect(calls.last, 'close');
    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets('late ready cannot restart refresh after the shelf is minimized',
      (tester) async {
    tester.view.physicalSize = const Size(820, 540);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    final ready = Completer<bool>();
    messenger.setMockMethodCallHandler(channel, (_) => ready.future);
    final backend = History();
    await tester
        .pumpWidget(ClipboardShelfApp(backend: backend, channel: channel));
    await tester.pump();
    await host('visibility', false);
    await tester.pump();
    final page =
        tester.widget<ClipboardHistoryPage>(find.byType(ClipboardHistoryPage));
    ready.complete(true);
    await tester.pumpAndSettle();
    expect(page.session.canInteract, isFalse);
    backend.rows = [item('unexpected')];
    await tester.pump(const Duration(seconds: 2));
    expect(page.session.items.any((row) => row.id == 'unexpected'), isFalse);
    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets(
      'shelf refreshes new records but pauses while minimized and releases its session on close',
      (tester) async {
    tester.view.physicalSize = const Size(820, 540);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    final backend = History();
    await tester
        .pumpWidget(ClipboardShelfApp(backend: backend, channel: channel));
    await tester.pumpAndSettle();
    final session = tester
        .widget<ClipboardHistoryPage>(find.byType(ClipboardHistoryPage))
        .session;
    expect(session.selected?.id, '1',
        reason: 'Initial selection before inserting a new record');
    backend.rows = [item('new'), ...backend.rows];
    await tester.pump(const Duration(seconds: 1));
    await tester.pumpAndSettle();
    expect(session.items.first.id, 'new');
    expect(session.selected!.id, '1');
    await host('visibility', false);
    await tester.pump();
    backend.rows = [item('later')];
    await tester.pump(const Duration(seconds: 2));
    expect(session.items.length, 3);
    await host('present');
    await tester.pumpAndSettle();
    expect(session.items.single.id, 'later');
    await tester.pumpWidget(const SizedBox.shrink());
    expect(session.disposed, isTrue);
    expect(session.items, isEmpty);
    await tester.pump(const Duration(seconds: 2));
    expect(tester.takeException(), isNull);
  });
}
