import 'dart:async';
import 'dart:convert';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:anywhere/infrastructure/desktop_environment.dart';
import 'package:anywhere/src/rust/api.dart';
import 'package:anywhere/features/clipboard/clipboard_history_session.dart';
import 'package:anywhere/ui/app_style.dart';
import 'support/clipboard_fixtures.dart';

void main() {
  setUp(() => DesktopEnvironment.configure(
      const DesktopEnvironment(platform: TargetPlatform.macOS)));
  tearDown(DesktopEnvironment.reset);

  test('late search and detail responses never replace the current selection',
      () async {
    final backend = History();
    final session = ClipboardHistorySession(backend);
    await Future<void>.delayed(Duration.zero);
    final first = Completer<ClipboardDetailDto?>();
    backend.detailOverride = (id) => id == '1'
        ? first.future
        : Future.value(previewDetail(item(id), 'Current'));
    final stale = session.select(0);
    final current = session.select(1);
    first.complete(previewDetail(item('1'), 'Stale'));
    await Future.wait([stale, current]);
    expect(session.preview!.text, 'Current');
    final oldSearch = Completer<ClipboardHistoryDto>();
    backend.searchOverride = (_) => oldSearch.future;
    final old = session.refresh();
    backend.searchOverride = (_) async => ClipboardHistoryDto(
        recording: false, available: true, items: [item('3')]);
    await session.refresh();
    oldSearch.complete(ClipboardHistoryDto(
        recording: false, available: true, items: [item('old')]));
    await old;
    expect(session.items.single.id, '3');
    session.dispose();
  });

  test(
      'refresh preserves identity, management works and hidden pages reject side effects',
      () async {
    final backend = History();
    final session = ClipboardHistorySession(backend);
    await Future<void>.delayed(Duration.zero);
    await session.select(1);
    backend.rows = [item('new'), ...backend.rows];
    await session.refresh();
    expect(session.selected!.id, '2');
    await session.copy();
    expect(backend.copied, ['2']);
    await session.delete();
    expect(session.items.map((i) => i.id), ['new', '1']);
    session.bindInteraction(() => false);
    await session.copy();
    expect(backend.copied, ['2']);
    expect(backend.rows.length, 2);
    session.dispose();
  });

  test('caret-only changes preserve results, preview and selection', () async {
    final session = ClipboardHistorySession(History());
    await Future<void>.delayed(Duration.zero);
    await session.select(1);
    final preview = session.preview;
    session.query.selection = const TextSelection.collapsed(offset: 0);
    expect(session.items.length, 2);
    expect(session.selected!.id, '2');
    expect(session.preview, same(preview));
    expect(session.loading, isFalse);
    session.dispose();
  });

  test('refresh keeps a selection made while its search was in flight',
      () async {
    final backend = History();
    final session = ClipboardHistorySession(backend);
    await Future<void>.delayed(Duration.zero);
    final response = Completer<ClipboardHistoryDto>();
    backend.searchOverride = (_) => response.future;
    final refresh = session.refresh(background: true);
    await session.select(1);
    response.complete(ClipboardHistoryDto(
        recording: false,
        available: true,
        items: [item('new'), ...backend.rows]));
    await refresh;
    expect(session.selected!.id, '2');
    expect(session.preview!.item.id, '2');
    session.dispose();
  });

  test('transient background failure preserves usable rows and preview',
      () async {
    final backend = History();
    final session = ClipboardHistorySession(backend);
    await Future<void>.delayed(Duration.zero);
    await session.select(1);
    final preview = session.preview;
    backend.searchOverride = (_) async => throw StateError('unavailable');
    await session.refresh(background: true);
    expect(session.items.length, 2);
    expect(session.selected!.id, '2');
    expect(session.preview, same(preview));
    expect(session.notice, contains('Refresh'));
    backend.searchOverride = null;
    await session.refresh(background: true);
    expect(session.notice, isNull);
    session.dispose();
  });

  test('rapid selections coalesce previews and dispose rejects late responses',
      () async {
    final backend = History()..rows = [item('1'), item('2'), item('3')];
    final session = ClipboardHistorySession(backend);
    await Future<void>.delayed(Duration.zero);
    final first = Completer<ClipboardDetailDto?>();
    final last = Completer<ClipboardDetailDto?>();
    final requests = <String>[];
    backend.detailOverride = (id) {
      requests.add(id);
      return id == '1' ? first.future : last.future;
    };
    final a = session.select(0);
    final b = session.select(1);
    final c = session.select(2);
    expect(requests, ['1']);
    first.complete(previewDetail(item('1'), 'old'));
    await Future<void>.delayed(Duration.zero);
    expect(requests, ['1', '3']);
    expect(session.preview, isNull);
    session.dispose();
    last.complete(previewDetail(item('3'), 'late'));
    await Future.wait([a, b, c]);
    expect(session.preview, isNull);
    expect(session.items, isEmpty);
  });

  testWidgets(
      'history page exposes usage actions without recording or bulk-clear controls',
      (tester) async {
    tester.view.physicalSize = const Size(880, 560);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    final session = ClipboardHistorySession(History());
    await tester.pumpWidget(MaterialApp(
        theme: AppStyle.theme,
        home: Scaffold(
            body: ListenableBuilder(
                listenable: session,
                builder: (context, _) => session.build(context,
                    onBack: () {}, presentation: 0, visible: true)))));
    await tester.pumpAndSettle();
    expect(find.text('Enable Recording'), findsNothing);
    expect(find.text('History recording is off'), findsNothing);
    await tester.tap(find.text('Actions · ⌘K'));
    await tester.pumpAndSettle();
    expect(find.text('Copy · ↵'), findsOneWidget);
    expect(find.text('Delete Entry'), findsOneWidget);
    expect(find.text('Refresh'), findsOneWidget);
    expect(find.text('Clear History…'), findsNothing);
    expect(find.text('Pause Recording'), findsNothing);
    await tester.pumpWidget(const SizedBox.shrink());
    session.dispose();
  });

  testWidgets(
      'presentation changes restore input without rebuilding during notification',
      (tester) async {
    tester.view.physicalSize = const Size(1000, 700);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    final session = ClipboardHistorySession(History());
    Widget frame(int presentation, bool visible) => MaterialApp(
        theme: AppStyle.theme,
        home: Scaffold(
            body: SizedBox(
                width: 880,
                height: 560,
                child: ListenableBuilder(
                    listenable: session,
                    builder: (context, _) => session.build(context,
                        onBack: () {},
                        presentation: presentation,
                        visible: visible)))));
    await tester.pumpWidget(frame(1, true));
    await tester.pumpAndSettle();
    await tester.enterText(
        find.byKey(const Key('clipboard-search')), 'saved query');
    await tester.pump(const Duration(milliseconds: 160));
    await tester.pumpAndSettle();
    await session.select(1);
    await tester.pumpWidget(frame(2, false));
    await tester.pumpWidget(frame(3, true));
    await tester.pumpAndSettle();
    expect(session.query.text, 'saved query');
    expect(session.selected!.id, '2');
    expect(session.focus.hasFocus, isTrue);
    expect(tester.takeException(), isNull);
    await tester.pumpWidget(const SizedBox());
    session.dispose();
  });

  testWidgets(
      'shared search page supports arrows, Return, Actions and Escape without Tab',
      (tester) async {
    tester.view.physicalSize = const Size(1000, 700);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    final backend = History();
    final session = ClipboardHistorySession(backend);
    var back = 0;
    await tester.pumpWidget(MaterialApp(
        theme: AppStyle.theme.copyWith(platform: TargetPlatform.macOS),
        builder: AppStyle.windowBuilder,
        home: Scaffold(
            body: Center(
                child: SizedBox(
                    width: 880,
                    height: 560,
                    child: ListenableBuilder(
                        listenable: session,
                        builder: (context, _) =>
                            session.build(context, onBack: () {
                              back++;
                            }, presentation: 1, visible: true)))))));
    await tester.pumpAndSettle();
    expect(session.focus.hasFocus, isTrue);
    await tester.enterText(find.byKey(const Key('clipboard-search')), 'query');
    await tester.pump(const Duration(milliseconds: 160));
    await tester.pumpAndSettle();
    await tester.sendKeyEvent(LogicalKeyboardKey.arrowDown);
    await tester.pumpAndSettle();
    expect(session.preview!.text, 'Full text 2');
    await tester.sendKeyEvent(LogicalKeyboardKey.tab);
    expect(session.focus.hasFocus, isTrue);
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pumpAndSettle();
    expect(backend.copied, ['2']);
    await tester.sendKeyDownEvent(LogicalKeyboardKey.metaLeft);
    await tester.sendKeyEvent(LogicalKeyboardKey.keyK);
    await tester.sendKeyUpEvent(LogicalKeyboardKey.metaLeft);
    await tester.pumpAndSettle();
    expect(find.text('Delete Entry'), findsOneWidget);
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pumpAndSettle();
    expect(back, 0);
    expect(session.query.text, 'query');
    expect(session.focus.hasFocus, isTrue);
    expect(session.query.selection, const TextSelection.collapsed(offset: 5));
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    expect(back, 1);
    expect(tester.takeException(), isNull);
    await tester.pumpWidget(const SizedBox());
    session.dispose();
  });
  testWidgets('type icons and bounded image/file previews follow selection',
      (tester) async {
    tester.view.physicalSize = const Size(1000, 700);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    final backend = History();
    final image = ClipboardItemDto(
        id: 'image',
        summary: 'Image',
        kind: 'image',
        copiedAtMs: 1000,
        byteCount: 100);
    final files = ClipboardItemDto(
        id: 'files',
        summary: 'two files',
        kind: 'files',
        copiedAtMs: 1000,
        byteCount: 100);
    backend.rows = [image, files];
    backend.detailOverride = (id) async => id == 'image'
        ? ClipboardDetailDto(
            item: image,
            text: '',
            imagePng: base64Decode(
                'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+aT1cAAAAASUVORK5CYII='),
            files: const [],
            formats: const [
                'public.png'
              ])
        : ClipboardDetailDto(
            item: files,
            text: '',
            imagePng: Uint8List(0),
            files: const ['/tmp/a file.txt', '/tmp/中文.png'],
            formats: const ['File list']);
    final session = ClipboardHistorySession(backend);
    await tester.pumpWidget(MaterialApp(
        theme: AppStyle.theme,
        home: Scaffold(
            body: SizedBox(
                width: 880,
                height: 560,
                child: ListenableBuilder(
                    listenable: session,
                    builder: (context, _) => session.build(context,
                        onBack: () {}, presentation: 1, visible: true))))));
    await tester.pumpAndSettle();
    expect(find.byKey(const Key('clipboard-image-preview')), findsOneWidget);
    await tester.sendKeyEvent(LogicalKeyboardKey.arrowDown);
    await tester.pumpAndSettle();
    expect(find.byKey(const Key('clipboard-image-preview')), findsNothing);
    expect(find.text('/tmp/a file.txt'), findsOneWidget);
    expect(find.text('/tmp/中文.png'), findsOneWidget);
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pumpAndSettle();
    expect(backend.copied, ['files']);
    expect(tester.takeException(), isNull);
    await tester.pumpWidget(const SizedBox());
    session.dispose();
  });
  testWidgets(
      'clicking a history row restores search focus and Return copies it',
      (tester) async {
    tester.view.physicalSize = const Size(1000, 700);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    final backend = History();
    final session = ClipboardHistorySession(backend);
    await tester.pumpWidget(MaterialApp(
        theme: AppStyle.theme.copyWith(platform: TargetPlatform.macOS),
        home: Scaffold(
            body: SizedBox(
                width: 880,
                height: 560,
                child: ListenableBuilder(
                    listenable: session,
                    builder: (context, _) => session.build(context,
                        onBack: () {}, presentation: 1, visible: true))))));
    await tester.pumpAndSettle();
    session.focus.unfocus();
    await tester.pump();
    expect(session.focus.hasFocus, isFalse);
    await tester.tap(find.text('Entry 2'));
    await tester.pumpAndSettle();
    expect(session.focus.hasFocus, isTrue);
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pumpAndSettle();
    expect(backend.copied, ['2']);
    await tester.tap(find.text('Copy ↵'));
    await tester.pumpAndSettle();
    expect(session.focus.hasFocus, isTrue);
    expect(backend.copied, ['2', '2']);
    expect(tester.takeException(), isNull);
    await tester.pumpWidget(const SizedBox());
    session.dispose();
  });
}
