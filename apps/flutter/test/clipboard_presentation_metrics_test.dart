import 'dart:async';

import 'package:anywhere/features/clipboard/clipboard_history_session.dart';
import 'package:anywhere/features/clipboard/clipboard_shelf_app.dart';
import 'package:anywhere/features/clipboard/clipboard_shelf_page.dart';
import 'package:anywhere/infrastructure/presentation_metrics.dart';
import 'package:anywhere/src/rust/api.dart';
import 'package:anywhere/ui/app_style.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

import 'support/clipboard_fixtures.dart';

void main() {
  final samples = <Map<String, Object>>[];
  setUp(() {
    samples.clear();
    PresentationMetrics.reporter = samples.add;
  });
  tearDown(() => PresentationMetrics.reporter = null);

  Future<void> mount(WidgetTester tester, ClipboardHistorySession session,
      {bool visible = true}) async {
    await tester.pumpWidget(MaterialApp(
        theme: AppStyle.theme,
        home: Scaffold(
            body: ListenableBuilder(
                listenable: session,
                builder: (context, _) => session.build(context,
                    onBack: () {}, presentation: 1, visible: visible)))));
    await tester.pump();
  }

  testWidgets('history measures the current list frame before preview finishes',
      (tester) async {
    final backend = History();
    final session = ClipboardHistorySession(backend);
    addTearDown(session.dispose);
    await mount(tester, session);
    final result = Completer<ClipboardHistoryDto>();
    final preview = Completer<ClipboardDetailDto?>();
    backend.searchOverride = (_) => result.future;
    backend.detailOverride = (_) => preview.future;
    session.query.text = 'private query is not exported';
    await tester.pump(const Duration(milliseconds: 119));
    expect(samples, isEmpty);
    await tester.pump(const Duration(milliseconds: 1));
    expect(samples, isEmpty);
    result.complete(ClipboardHistoryDto(
        recording: false, available: true, items: [item('3')]));
    await tester.pump();
    await tester.pump();
    expect(session.preview, isNull);
    expect(samples, hasLength(1));
    expect(samples.single['operation'], 'clipboard_query');
    expect(samples.single['rows'], 1);
    expect(samples.single.keys,
        unorderedEquals(['operation', 'elapsed_us', 'rows']));
    preview.complete(previewDetail(item('3'), 'private body'));
    await tester.pump();
    await session.refresh(background: true);
    await tester.pump();
    expect(samples, hasLength(1));
    await tester.pumpWidget(const SizedBox());
  });

  testWidgets(
      'superseded searches and IME composition never report old content',
      (tester) async {
    final backend = History();
    final session = ClipboardHistorySession(backend);
    addTearDown(session.dispose);
    await mount(tester, session);
    final old = Completer<ClipboardHistoryDto>();
    backend.searchOverride = (_) => old.future;
    session.query.text = 'old';
    await tester.pump(const Duration(milliseconds: 120));
    session.query.value = const TextEditingValue(
        text: 'zhong', composing: TextRange(start: 0, end: 5));
    old.complete(ClipboardHistoryDto(
        recording: false, available: true, items: [item('old')]));
    await tester.pump();
    expect(samples, isEmpty);
    expect(session.performance.pending, isFalse);
    backend.searchOverride = (_) async => ClipboardHistoryDto(
        recording: false, available: true, items: [item('new')]);
    session.query.value = const TextEditingValue(text: '中');
    await tester.pump(const Duration(milliseconds: 120));
    await tester.pump();
    expect(samples, hasLength(1));
    expect(session.selected?.id, 'new');
    await tester.pumpWidget(const SizedBox());
  });

  testWidgets('hidden and disposed pages cannot publish a pending measurement',
      (tester) async {
    final backend = History();
    final session = ClipboardHistorySession(backend);
    await mount(tester, session);
    final result = Completer<ClipboardHistoryDto>();
    backend.searchOverride = (_) => result.future;
    session.query.text = 'pending';
    await tester.pump(const Duration(milliseconds: 120));
    await mount(tester, session, visible: false);
    result.complete(ClipboardHistoryDto(
        recording: false, available: true, items: [item('3')]));
    await tester.pump();
    await mount(tester, session);
    expect(samples, isEmpty);
    expect(session.performance.pending, isFalse);
    await tester.pumpWidget(const SizedBox());
    session.query.text = 'cancel';
    session.dispose();
    await tester.pump(const Duration(milliseconds: 200));
    expect(samples, isEmpty);
  });

  testWidgets('the independent shelf reports its own query operation',
      (tester) async {
    const channel = MethodChannel('test/shelf-metrics');
    final messenger =
        TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger;
    messenger.setMockMethodCallHandler(channel, (_) async => true);
    await tester.pumpWidget(ClipboardShelfApp(
        backend: History(),
        channel: channel,
        refreshInterval: const Duration(hours: 1)));
    await tester.pumpAndSettle();
    final session = tester
        .widget<ClipboardShelfPage>(find.byType(ClipboardShelfPage))
        .session;
    session.query.text = 'sample';
    await tester.pump(const Duration(milliseconds: 120));
    await tester.pump();
    expect(samples.single['operation'], 'shelf_query');
    await tester.pumpWidget(const SizedBox());
    messenger.setMockMethodCallHandler(channel, null);
  });
}
