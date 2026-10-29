import 'dart:async';

import 'package:anywhere/features/clipboard/clipboard_history_session.dart';
import 'package:anywhere/infrastructure/desktop_environment.dart';
import 'package:anywhere/src/rust/api.dart';
import 'package:anywhere/ui/app_components.dart';
import 'package:anywhere/ui/app_style.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

import 'support/clipboard_fixtures.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  setUp(() {
    mockNativePreview();
    DesktopEnvironment.configure(
        const DesktopEnvironment(platform: TargetPlatform.macOS));
  });
  tearDown(DesktopEnvironment.reset);

  test('clipboard lists do not choose a default or act without a selection',
      () async {
    final backend = History();
    var reads = 0;
    backend.detailOverride = (id) async {
      reads++;
      return previewDetail(item(id), id);
    };
    final session = ClipboardHistorySession(backend);
    await Future<void>.delayed(Duration.zero);
    expect(session.selectedIndex, -1);
    expect(session.selected, isNull);
    expect(await session.copy(), isFalse);
    await session.delete();
    expect(backend.rows.length, 2);
    expect(backend.copied, isEmpty);
    await session.refresh(background: true);
    expect(session.selected, isNull);
    await session.select(session.selectedIndex + 1);
    expect(session.selected?.id, '1');
    backend.rows = [item('new'), ...backend.rows];
    await session.refresh(background: true);
    expect(session.selected?.id, '1');
    session.query.text = 'changed';
    expect(session.selected, isNull);
    session.preview.toggle();
    expect(reads, 0);
    session.dispose();
  });

  test('late search responses never replace the current selection', () async {
    final backend = History();
    final session = ClipboardHistorySession(backend);
    await Future<void>.delayed(Duration.zero);
    final oldSearch = Completer<ClipboardHistoryDto>();
    backend.searchOverride = (_) => oldSearch.future;
    final old = session.refresh();
    backend.searchOverride = (_) async => ClipboardHistoryDto(
        recording: false, available: true, items: [item('3')]);
    final latest = session.refresh();
    oldSearch.complete(ClipboardHistoryDto(
        recording: false, available: true, items: [item('old')]));
    await Future.wait([old, latest]);
    expect(session.items.single.id, '3');
    expect(session.selected, isNull);
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

  testWidgets('composition defers search and rejects pre-composition responses',
      (tester) async {
    final backend = History();
    final session = ClipboardHistorySession(backend);
    await tester.pump();
    final pending = Completer<ClipboardHistoryDto>();
    final searches = <String>[];
    backend.searchOverride = (query) {
      searches.add(query);
      return pending.future;
    };
    session.query.text = 'old';
    await tester.pump(const Duration(milliseconds: 120));
    session.query.value = const TextEditingValue(
        text: 'zhong', composing: TextRange(start: 0, end: 5));
    pending.complete(ClipboardHistoryDto(
        recording: false, available: true, items: [item('stale')]));
    await tester.pump(const Duration(milliseconds: 200));
    await session.refresh(background: true);
    expect(searches, ['old']);
    expect(session.items, isEmpty);
    backend.searchOverride = (query) async {
      searches.add(query);
      return ClipboardHistoryDto(
          recording: false, available: true, items: [item('committed')]);
    };
    session.query.value = const TextEditingValue(text: '中');
    await tester.pump(const Duration(milliseconds: 120));
    expect(searches, ['old', '中']);
    expect(session.items.single.id, 'committed');
    expect(session.selected, isNull);
    session.dispose();
  });

  testWidgets('composition cancels a debounce and resumes an unchanged commit',
      (tester) async {
    final backend = History();
    final session = ClipboardHistorySession(backend);
    await tester.pump();
    final searches = <String>[];
    backend.searchOverride = (query) async {
      searches.add(query);
      return ClipboardHistoryDto(
          recording: false, available: true, items: backend.rows);
    };
    session.query.text = '中';
    session.query.value = const TextEditingValue(
        text: '中', composing: TextRange(start: 0, end: 1));
    await tester.pump(const Duration(milliseconds: 200));
    expect(searches, isEmpty);
    session.query.value = const TextEditingValue(text: '中');
    await tester.pump(const Duration(milliseconds: 120));
    expect(searches, ['中']);
    expect(session.items.length, 2);
    session.dispose();
  });

  test('browsing and reselecting never prepare preview bodies', () async {
    final backend = History();
    var reads = 0;
    backend.detailOverride = (id) async {
      reads++;
      return previewDetail(item(id), id);
    };
    final session = ClipboardHistorySession(backend);
    await Future<void>.delayed(Duration.zero);
    await session.select(1);
    await session.select(20);
    await session.refresh();
    expect(reads, 0);
    expect(session.selected!.id, '2');
    session.dispose();
  });

  test('copying does not require preview or requery history', () async {
    final backend = History();
    final session = ClipboardHistorySession(backend);
    await Future<void>.delayed(Duration.zero);
    backend.detailOverride = (_) async => throw StateError('must not prepare');
    backend.searchOverride = (_) async => throw StateError('must not query');
    await session.select(0);
    expect(await session.copy(), true);
    expect(backend.copied, ['1']);
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
    expect(session.selected!.id, '2');
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

  test('disposing clears metadata without preparing preview bodies', () async {
    final backend = History();
    final session = ClipboardHistorySession(backend);
    await Future<void>.delayed(Duration.zero);
    await session.select(1);
    session.dispose();
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
    expect(find.text('2 entries'), findsNothing);
    await tester.sendKeyEvent(LogicalKeyboardKey.arrowDown);
    await tester.pumpAndSettle();
    expect(find.byKey(const Key('selected-row-primary')), findsOneWidget);
    await session.select(0);
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const Key('selected-row-actions')));
    await tester.pumpAndSettle();
    expect(find.text('Copy · ↵'), findsOneWidget);
    expect(find.text('Delete Entry'), findsOneWidget);
    expect(find.text('Refresh'), findsOneWidget);
    expect(find.text('Clear History…'), findsNothing);
    expect(find.text('Pause Recording'), findsNothing);
    await tester.pumpWidget(const SizedBox.shrink());
    session.dispose();
  });

  testWidgets('hidden history cannot take focus or open its Actions menu',
      (tester) async {
    final session = ClipboardHistorySession(History());
    await tester.pumpWidget(MaterialApp(
        theme: AppStyle.theme,
        home: Scaffold(
            body: ListenableBuilder(
                listenable: session,
                builder: (context, _) => session.build(context,
                    onBack: () {}, presentation: 1, visible: false)))));
    await tester.pumpAndSettle();
    expect(session.focus.hasFocus, isFalse);
    expect(find.byKey(const Key('selected-row-actions')), findsNothing);
    await tester.sendKeyDownEvent(LogicalKeyboardKey.metaLeft);
    await tester.sendKeyEvent(LogicalKeyboardKey.keyK);
    await tester.sendKeyUpEvent(LogicalKeyboardKey.metaLeft);
    await tester.pumpAndSettle();
    expect(find.text('Delete Entry'), findsNothing);
    expect(session.focus.hasFocus, isFalse);
    await tester.pumpWidget(const SizedBox());
    session.dispose();
  });

  testWidgets('empty history keeps Refresh discoverable without a footer',
      (tester) async {
    final backend = History()..rows = [];
    final session = ClipboardHistorySession(backend);
    await tester.pumpWidget(MaterialApp(
        theme: AppStyle.theme,
        home: Scaffold(
            body: ListenableBuilder(
                listenable: session,
                builder: (context, _) => session.build(context,
                    onBack: () {}, presentation: 1, visible: true)))));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const Key('history-empty-actions')));
    await tester.pumpAndSettle();
    backend.rows = [item('new')];
    await tester.tap(find.text('Refresh'));
    await tester.pumpAndSettle();
    expect(session.selected, isNull);
    await tester.sendKeyEvent(LogicalKeyboardKey.arrowDown);
    await tester.pumpAndSettle();
    expect(session.selected?.id, 'new');
    expect(find.byKey(const Key('history-empty-actions')), findsNothing);
    expect(find.byKey(const Key('selected-row-actions')), findsOneWidget);
    expect(session.focus.hasFocus, isTrue);
    await tester.pumpWidget(const SizedBox());
    session.dispose();
  });

  testWidgets('history menu cannot delete a replacement selection',
      (tester) async {
    final backend = History();
    final session = ClipboardHistorySession(backend);
    await tester.pumpWidget(MaterialApp(
        theme: AppStyle.theme,
        home: Scaffold(
            body: ListenableBuilder(
                listenable: session,
                builder: (context, _) => session.build(context,
                    onBack: () {}, presentation: 1, visible: true)))));
    await tester.pumpAndSettle();
    await session.select(0);
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const Key('selected-row-actions')));
    await tester.pumpAndSettle();
    backend.rows = [item('replacement')];
    await session.refresh();
    await tester.pump();
    await tester.tap(find.text('Delete Entry'));
    await tester.pumpAndSettle();
    expect(backend.rows.single.id, 'replacement');
    expect(session.selected, isNull);
    expect(find.text('Delete Entry'), findsNothing);
    await tester.pumpWidget(const SizedBox());
    session.dispose();
  });

  testWidgets('history Actions respect modifiers and IME Return does not copy',
      (tester) async {
    final backend = History();
    final session = ClipboardHistorySession(backend);
    await tester.pumpWidget(MaterialApp(
        theme: AppStyle.theme,
        home: Scaffold(
            body: ListenableBuilder(
                listenable: session,
                builder: (context, _) => session.build(context,
                    onBack: () {}, presentation: 1, visible: true)))));
    await tester.pumpAndSettle();
    await tester.sendKeyDownEvent(LogicalKeyboardKey.metaLeft);
    await tester.sendKeyDownEvent(LogicalKeyboardKey.shiftLeft);
    await tester.sendKeyEvent(LogicalKeyboardKey.keyK);
    await tester.sendKeyUpEvent(LogicalKeyboardKey.shiftLeft);
    await tester.sendKeyUpEvent(LogicalKeyboardKey.metaLeft);
    await tester.pumpAndSettle();
    expect(find.text('Delete Entry'), findsNothing);
    tester.testTextInput.updateEditingValue(const TextEditingValue(
        text: 'zhong',
        selection: TextSelection.collapsed(offset: 5),
        composing: TextRange(start: 0, end: 5)));
    await tester.pump();
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    expect(backend.copied, isEmpty);
    expect(session.query.value.composing.isCollapsed, isFalse);
    await tester.pumpWidget(const SizedBox());
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
    await tester.sendKeyEvent(LogicalKeyboardKey.arrowDown);
    await tester.pumpAndSettle();
    expect(session.selected!.id, '2');
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
  testWidgets('history keeps a single list without a fixed preview pane',
      (tester) async {
    mockNativePreview();
    final session = ClipboardHistorySession(History());
    await tester.pumpWidget(MaterialApp(
        theme: AppStyle.theme,
        home: Scaffold(
            body: ListenableBuilder(
                listenable: session,
                builder: (context, _) => session.build(context,
                    onBack: () {}, presentation: 0, visible: true)))));
    await tester.pumpAndSettle();
    expect(find.byKey(const Key('clipboard-preview-pane')), findsNothing);
    expect(session.selected, isNull);
    expect(find.byKey(const Key('selected-row-preview')), findsNothing);
    await tester.sendKeyEvent(LogicalKeyboardKey.arrowDown);
    await tester.pumpAndSettle();
    expect(find.byKey(const Key('selected-row-preview')), findsOneWidget);
    expect(session.selected?.id, '1');
    expect(previewCalls.where((call) => call.method == 'show'), isEmpty);
    await tester.sendKeyEvent(LogicalKeyboardKey.space);
    await tester.pumpAndSettle();
    expect(previewCalls.last.arguments['paths'], ['/synthetic/1.txt']);
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pumpAndSettle();
    expect(session.selected?.id, '1');
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
    await tester.tap(find.byKey(const Key('selected-row-primary')));
    await tester.pumpAndSettle();
    expect(session.focus.hasFocus, isTrue);
    expect(backend.copied, ['2', '2']);
    expect(tester.takeException(), isNull);
    await tester.pumpWidget(const SizedBox());
    session.dispose();
  });
  testWidgets('search header fits and copying keeps list geometry stable',
      (tester) async {
    tester.view.physicalSize = const Size(960, 600);
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
                    onBack: () {}, presentation: 1, visible: true)))));
    await tester.pumpAndSettle();
    final field = find.byKey(const Key('clipboard-search'));
    final header = tester.getRect(find.byType(AppPageHeader));
    final input = tester.getRect(field);
    expect(input.top, greaterThan(header.top + 20));
    expect(input.bottom, lessThan(header.bottom - 8));
    expect(tester.widget<TextField>(field).decoration!.hintStyle!.fontSize,
        DesignTokens.pageHeadingSize);
    final list = find.byType(ListView);
    expect(find.byKey(const Key('clipboard-preview-pane')), findsNothing);
    final listBefore = tester.getRect(list);
    await tester.sendKeyEvent(LogicalKeyboardKey.arrowDown);
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const Key('selected-row-primary')));
    await tester.pump(const Duration(milliseconds: 30));
    expect(session.status, contains('Copied'));
    await tester.pumpAndSettle();
    expect(find.text('Copied'), findsOneWidget);
    expect(find.byKey(const Key('clipboard-row-status')), findsOneWidget);
    expect(find.byType(AppNotice), findsNothing);
    expect(tester.getRect(list), listBefore);
    expect(find.text('Back'), findsNothing);
    await tester.sendKeyEvent(LogicalKeyboardKey.arrowDown);
    await tester.pumpAndSettle();
    expect(find.text('Copied'), findsNothing);
    expect(tester.takeException(), isNull);
    await tester.pumpWidget(const SizedBox());
    session.dispose();
  });

  testWidgets(
      'minimum shelf size and large text keep preview and controls usable',
      (tester) async {
    tester.view.physicalSize = const Size(720, 420);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    final backend = History();
    backend.detailOverride = (id) async => previewDetail(item(id),
        List.filled(200, 'A long line with readable content').join('\n'));
    final session = ClipboardHistorySession(backend);
    await tester.pumpWidget(MaterialApp(
        theme: AppStyle.theme,
        builder: (context, child) => MediaQuery(
            data: MediaQuery.of(context)
                .copyWith(textScaler: TextScaler.linear(1.5)),
            child: child!),
        home: Scaffold(
            body: ListenableBuilder(
                listenable: session,
                builder: (context, _) => session.build(context,
                    onBack: () {}, presentation: 1, visible: true)))));
    await tester.pumpAndSettle();
    expect(find.byKey(const Key('clipboard-preview-pane')), findsNothing);
    await tester.sendKeyEvent(LogicalKeyboardKey.arrowDown);
    await tester.pumpAndSettle();
    expect(find.byKey(const Key('selected-row-preview')), findsOneWidget);
    await tester.sendKeyEvent(LogicalKeyboardKey.arrowDown);
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pumpAndSettle();
    expect(backend.copied, ['2']);
    expect(tester.takeException(), isNull);
    await tester.pumpWidget(const SizedBox());
    session.dispose();
  });
}
