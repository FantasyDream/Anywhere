import 'package:anywhere/features/clipboard/clipboard_history_list.dart';
import 'package:anywhere/features/clipboard/clipboard_history_session.dart';
import 'package:anywhere/features/clipboard/clipboard_shelf_app.dart';
import 'package:anywhere/features/clipboard/clipboard_shelf_controls.dart';
import 'package:anywhere/features/clipboard/clipboard_shelf_page.dart';
import 'package:anywhere/infrastructure/desktop_environment.dart';
import 'package:anywhere/l10n/app_language.dart';
import 'package:anywhere/src/rust/api.dart';
import 'package:anywhere/ui/app_components.dart';
import 'package:anywhere/ui/app_style.dart';
import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

import 'support/clipboard_fixtures.dart';

class InteractionHistory extends History {
  final deleted = <String>[];
  bool failCopy = false;
  @override
  Future<void> copy(String id) async {
    if (failCopy) throw StateError('copy unavailable');
    await super.copy(id);
  }

  @override
  Future<void> delete(String id) async {
    deleted.add(id);
    await super.delete(id);
  }
}

void main() {
  const channel = MethodChannel('anywhere/shelf-interaction-test');
  TestWidgetsFlutterBinding.ensureInitialized();
  final messenger =
      TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger;
  final calls = <String>[];
  final search = find.byKey(const Key('clipboard-search'));

  setUp(() {
    calls.clear();
    AppLanguage.current.apply('en');
    DesktopEnvironment.configure(
        const DesktopEnvironment(platform: TargetPlatform.macOS));
    messenger.setMockMethodCallHandler(channel, (call) async {
      calls.add(call.method);
      return switch (call.method) {
        'ready' => true,
        'returnToTarget' => 'returned',
        _ => null,
      };
    });
  });
  tearDown(() {
    messenger.setMockMethodCallHandler(channel, null);
    AppLanguage.current.apply('system');
    DesktopEnvironment.reset();
  });

  Future<void> mount(WidgetTester tester, History backend) async {
    tester.view.physicalSize = const Size(420, 560);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    await tester.pumpWidget(ClipboardShelfApp(
        backend: backend,
        channel: channel,
        refreshInterval: const Duration(hours: 1)));
    await tester.pumpAndSettle();
    addTearDown(() async => tester.pumpWidget(const SizedBox()));
  }

  ClipboardHistorySession session(WidgetTester tester) => tester
      .widget<ClipboardShelfPage>(find.byType(ClipboardShelfPage))
      .session;

  Future<void> command(WidgetTester tester, LogicalKeyboardKey key,
      {bool shift = false}) async {
    await tester.sendKeyDownEvent(LogicalKeyboardKey.metaLeft);
    if (shift) await tester.sendKeyDownEvent(LogicalKeyboardKey.shiftLeft);
    await tester.sendKeyEvent(key);
    if (shift) await tester.sendKeyUpEvent(LogicalKeyboardKey.shiftLeft);
    await tester.sendKeyUpEvent(LogicalKeyboardKey.metaLeft);
    await tester.pumpAndSettle();
  }

  testWidgets('shelf commands and copy feedback stay in the selected row',
      (tester) async {
    final backend = InteractionHistory();
    await mount(tester, backend);
    expect(find.byType(AppPageFooter), findsNothing);
    expect(find.text('2 entries'), findsNothing);
    expect(find.byKey(const Key('shelf-preview-toggle')), findsNothing);
    expect(find.text('⌘K'), findsOneWidget);
    final copy = find.byKey(const Key('selected-row-primary'));
    final first = tester.getRect(find.text('Entry 1'));
    expect(tester.getRect(copy).center.dy, closeTo(first.center.dy, 12));
    await tester.tap(copy);
    // A command must not wait for the object's double-click recognizer.
    await tester.pump(const Duration(milliseconds: 30));
    expect(backend.copied, ['1']);
    expect(find.byKey(const Key('clipboard-row-status')), findsOneWidget);
    expect(tester.getRect(find.text('Entry 1')).top, first.top);
    expect(session(tester).focus.hasFocus, isTrue);
    expect(calls, isNot(contains('returnToTarget')));
    await tester.sendKeyEvent(LogicalKeyboardKey.arrowDown);
    await tester.pumpAndSettle();
    expect(session(tester).selected?.id, '2');
    expect(find.byKey(const Key('clipboard-row-status')), findsNothing);
    expect(tester.getRect(copy).center.dy,
        closeTo(tester.getRect(find.text('Entry 2')).center.dy, 12));
    await tester.tap(copy);
    await tester.pumpAndSettle();
    expect(backend.copied, ['1', '2']);
    expect(tester.takeException(), isNull);
  });

  testWidgets('row actions open immediately and keep preview discoverable',
      (tester) async {
    final backend = InteractionHistory();
    await mount(tester, backend);
    await tester.tap(find.byKey(const Key('selected-row-actions')));
    await tester.pump(const Duration(milliseconds: 30));
    expect(find.byType(ShelfActionList), findsOneWidget);
    expect(backend.copied, isEmpty);
    await tester.tap(find.text('Preview'));
    await tester.pumpAndSettle();
    expect(session(tester).previewEnabled, isTrue);
    expect(find.byType(ClipboardHistoryList), findsOneWidget);
    expect(find.byType(AppPageFooter), findsNothing);
    expect(session(tester).focus.hasFocus, isTrue);
    expect(tester.takeException(), isNull);
  });

  testWidgets('empty shelf keeps a refresh entry without a footer',
      (tester) async {
    final backend = InteractionHistory()..rows = [];
    await mount(tester, backend);
    expect(find.byType(AppPageFooter), findsNothing);
    expect(find.byKey(const Key('selected-row-primary')), findsNothing);
    expect(find.text('⌘K'), findsOneWidget);
    await tester.tap(find.byKey(const Key('shelf-empty-actions')));
    await tester.pumpAndSettle();
    expect(find.byType(ShelfActionList), findsOneWidget);
    expect(session(tester).selected, isNull);
    backend.rows = [item('fresh')];
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pumpAndSettle();
    expect(find.byType(ShelfActionList), findsNothing);
    expect(find.text('Entry fresh'), findsOneWidget);
    expect(backend.copied, isEmpty);
    expect(tester.takeException(), isNull);
  });

  testWidgets('single click and drag never copy; double click copies its row',
      (tester) async {
    final backend = InteractionHistory();
    await mount(tester, backend);
    await tester.tap(find.text('Entry 2'));
    await tester.pump(const Duration(milliseconds: 400));
    expect(session(tester).selected?.id, '2');
    expect(backend.copied, isEmpty);
    await tester.drag(find.text('Entry 1'), const Offset(0, 30));
    await tester.pumpAndSettle();
    expect(backend.copied, isEmpty);
    await tester.tap(find.text('Entry 2'));
    await tester.pump(const Duration(milliseconds: 60));
    await tester.tap(find.text('Entry 2'));
    await tester.pumpAndSettle();
    expect(backend.copied, ['2']);
    expect(calls, isNot(contains('returnToTarget')));
    expect(session(tester).focus.hasFocus, isTrue);
  });

  testWidgets('held Return copies once; only explicit command returns focus',
      (tester) async {
    final backend = InteractionHistory();
    await mount(tester, backend);
    await tester.sendKeyDownEvent(LogicalKeyboardKey.enter);
    await tester.pumpAndSettle();
    await tester.sendKeyRepeatEvent(LogicalKeyboardKey.enter);
    await tester.pumpAndSettle();
    await tester.sendKeyRepeatEvent(LogicalKeyboardKey.enter);
    await tester.sendKeyUpEvent(LogicalKeyboardKey.enter);
    expect(backend.copied, ['1']);
    expect(calls, isNot(contains('returnToTarget')));
    await tester.sendKeyEvent(LogicalKeyboardKey.arrowDown);
    await command(tester, LogicalKeyboardKey.enter);
    expect(backend.copied, ['1', '2']);
    expect(calls.where((call) => call == 'returnToTarget'), hasLength(1));
    backend.failCopy = true;
    await command(tester, LogicalKeyboardKey.enter);
    expect(calls.where((call) => call == 'returnToTarget'), hasLength(1));
    expect(session(tester).notice, isNotNull);
  });

  testWidgets(
      'actions are fully keyboard operated and modifiers keep them open',
      (tester) async {
    final backend = InteractionHistory();
    await mount(tester, backend);
    await tester.sendKeyEvent(LogicalKeyboardKey.arrowDown);
    await command(tester, LogicalKeyboardKey.keyK);
    expect(find.byType(ShelfActionList), findsOneWidget);
    await tester.sendKeyDownEvent(LogicalKeyboardKey.metaLeft);
    await tester.pump();
    expect(find.byType(ShelfActionList), findsOneWidget);
    await tester.sendKeyUpEvent(LogicalKeyboardKey.metaLeft);
    await tester.sendKeyEvent(LogicalKeyboardKey.arrowDown);
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pumpAndSettle();
    expect(backend.copied, ['2']);
    expect(calls, contains('returnToTarget'));
    expect(find.byType(ShelfActionList), findsNothing);
    await command(tester, LogicalKeyboardKey.keyK);
    await command(tester, LogicalKeyboardKey.keyK);
    expect(find.byType(ShelfActionList), findsNothing);
    await command(tester, LogicalKeyboardKey.keyK);
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pumpAndSettle();
    expect(find.byType(ShelfActionList), findsNothing);
    expect(calls, isNot(contains('close')));
  });

  for (final outcome in ['failed', 'unavailable', 'skipped', 'exception']) {
    testWidgets('copy-and-return reports $outcome without losing the copy',
        (tester) async {
      messenger.setMockMethodCallHandler(channel, (call) async {
        calls.add(call.method);
        if (call.method == 'ready') return true;
        if (call.method == 'returnToTarget') {
          if (outcome == 'exception') {
            throw PlatformException(code: 'focus-unavailable');
          }
          return outcome;
        }
        return null;
      });
      final backend = InteractionHistory();
      await mount(tester, backend);
      await command(tester, LogicalKeyboardKey.enter);
      expect(backend.copied, ['1']);
      final current = session(tester);
      final en = lookupAppLocalizations(const Locale('en'));
      final zh = lookupAppLocalizations(const Locale('zh'));
      if (outcome == 'skipped') {
        expect(find.byType(AppNotice), findsNothing);
      } else {
        final message = outcome == 'unavailable'
            ? en.shelfReturnUnavailable
            : en.shelfReturnFailed;
        expect(find.text(message), findsOneWidget);
        AppLanguage.current.apply('zh');
        await tester.pumpAndSettle();
        expect(
            find.text(outcome == 'unavailable'
                ? zh.shelfReturnUnavailable
                : zh.shelfReturnFailed),
            findsOneWidget);
        expect(current.statusFor(zh), zh.copiedPaste);
        await current.refresh(background: true);
        await tester.pumpAndSettle();
        expect(
            find.text(outcome == 'unavailable'
                ? zh.shelfReturnUnavailable
                : zh.shelfReturnFailed),
            findsOneWidget);
      }
      expect(find.byType(ClipboardShelfPage), findsOneWidget);
      expect(calls, isNot(contains('close')));
    });
  }

  testWidgets('right click acts on the clicked identity even after refresh',
      (tester) async {
    final backend = InteractionHistory();
    await mount(tester, backend);
    await tester.tap(find.text('Entry 2'), buttons: kSecondaryMouseButton);
    await tester.pumpAndSettle();
    expect(find.byType(ShelfActionList), findsOneWidget);
    expect(backend.copied, isEmpty);
    backend.rows = [item('new'), item('1')];
    await session(tester).refresh(background: true);
    await tester.pumpAndSettle();
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pumpAndSettle();
    expect(backend.copied, isEmpty,
        reason: 'A disappeared target must not become the new selection');
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pumpAndSettle();
    await tester.tap(find.text('Entry 1'), buttons: kSecondaryMouseButton);
    await tester.pumpAndSettle();
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pumpAndSettle();
    expect(backend.copied, ['1']);
  });

  testWidgets(
      'inline preview follows selection while list stays keyboard usable',
      (tester) async {
    final backend = InteractionHistory();
    backend.detailOverride = (id) async =>
        previewDetail(item(id), List.filled(70, 'body $id').join('\n'));
    await mount(tester, backend);
    await command(tester, LogicalKeyboardKey.keyY);
    expect(find.byType(ClipboardHistoryList), findsOneWidget);
    expect(session(tester).preview?.item.id, '1');
    await tester.sendKeyDownEvent(LogicalKeyboardKey.altLeft);
    await tester.sendKeyEvent(LogicalKeyboardKey.pageDown);
    await tester.sendKeyUpEvent(LogicalKeyboardKey.altLeft);
    await tester.pumpAndSettle();
    final previewScroll = tester.widget<SingleChildScrollView>(find
        .ancestor(
            of: find.byKey(const Key('clipboard-preview')),
            matching: find.byType(SingleChildScrollView))
        .first);
    expect(previewScroll.controller!.offset, greaterThan(0));
    expect(session(tester).selected?.id, '1');
    await tester.sendKeyEvent(LogicalKeyboardKey.arrowDown);
    await tester.pumpAndSettle();
    expect(session(tester).preview?.item.id, '2');
    await tester.enterText(search, 'Entry ');
    await tester.pump(const Duration(milliseconds: 150));
    await tester.pumpAndSettle();
    await command(tester, LogicalKeyboardKey.keyF);
    expect(session(tester).focus.hasFocus, isTrue);
    expect(session(tester).query.selection,
        const TextSelection(baseOffset: 0, extentOffset: 6));
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pumpAndSettle();
    expect(session(tester).previewEnabled, isFalse);
    expect(session(tester).query.text, 'Entry ');
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pumpAndSettle();
    expect(session(tester).query.text, isEmpty);
    expect(calls, isNot(contains('close')));
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    expect(calls.last, 'close');
  });

  testWidgets('deletion defaults to Cancel and confirms the original identity',
      (tester) async {
    final backend = InteractionHistory();
    await mount(tester, backend);
    await command(tester, LogicalKeyboardKey.backspace, shift: true);
    expect(find.byType(ShelfDeletePrompt), findsOneWidget);
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pumpAndSettle();
    expect(backend.deleted, isEmpty);
    await command(tester, LogicalKeyboardKey.backspace, shift: true);
    backend.rows = [item('new'), item('1'), item('2')];
    await session(tester).refresh(background: true);
    await tester.pumpAndSettle();
    await tester.sendKeyEvent(LogicalKeyboardKey.arrowRight);
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pumpAndSettle();
    expect(backend.deleted, ['1']);
    await command(tester, LogicalKeyboardKey.backspace, shift: true);
    backend.rows = [item('2')];
    await session(tester).refresh(background: true);
    await tester.pumpAndSettle();
    await tester.sendKeyEvent(LogicalKeyboardKey.arrowRight);
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pumpAndSettle();
    expect(backend.deleted, ['1'],
        reason: 'A deleted target must not delete the next row instead');
  });

  testWidgets(
      'IME composition is never copied, deleted, or cleared by commands',
      (tester) async {
    final backend = InteractionHistory();
    await mount(tester, backend);
    session(tester).query.value = const TextEditingValue(
        text: 'ni',
        selection: TextSelection.collapsed(offset: 2),
        composing: TextRange(start: 0, end: 2));
    await tester.pump();
    await command(tester, LogicalKeyboardKey.enter);
    await command(tester, LogicalKeyboardKey.backspace, shift: true);
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    expect(backend.copied, isEmpty);
    expect(backend.deleted, isEmpty);
    expect(session(tester).query.text, 'ni');
    expect(calls, isNot(contains('close')));
    expect(find.byType(ShelfDeletePrompt), findsNothing);
  });

  testWidgets('an empty search never leaves preview stuck on loading',
      (tester) async {
    final backend = InteractionHistory();
    backend.searchOverride = (query) async => ClipboardHistoryDto(
        recording: false,
        available: true,
        items: query == 'missing' ? [] : backend.rows);
    await mount(tester, backend);
    await command(tester, LogicalKeyboardKey.keyY);
    await tester.enterText(search, 'missing');
    await tester.pump(const Duration(milliseconds: 150));
    await tester.pumpAndSettle();
    expect(find.text('No matching entries'), findsOneWidget);
    expect(find.text('Select an entry to preview'), findsOneWidget);
    expect(find.text('Loading preview…'), findsNothing);
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pumpAndSettle();
    expect(session(tester).query.text, 'missing');
    expect(find.text('Select an entry to preview'), findsNothing);
  });

  testWidgets('reopening dismisses transient commands and restores navigation',
      (tester) async {
    final backend = InteractionHistory()
      ..rows = List.generate(40, (index) => item('$index'));
    await mount(tester, backend);
    Future<void> host(String method, [Object? arguments]) =>
        messenger.handlePlatformMessage(
            channel.name,
            const StandardMethodCodec()
                .encodeMethodCall(MethodCall(method, arguments)),
            (_) {});
    await command(tester, LogicalKeyboardKey.keyK);
    await host('visibility', false);
    await tester.pumpAndSettle();
    await command(tester, LogicalKeyboardKey.enter);
    expect(backend.copied, isEmpty);
    await host('present');
    await tester.pumpAndSettle();
    expect(find.byType(ShelfActionList), findsNothing);
    expect(session(tester).focus.hasFocus, isTrue);
    await tester.sendKeyEvent(LogicalKeyboardKey.pageDown);
    await tester.pumpAndSettle();
    expect(session(tester).selectedIndex, greaterThan(1));
    await command(tester, LogicalKeyboardKey.arrowDown);
    expect(session(tester).selected?.id, '39');
    expect(find.text('Entry 39').hitTestable(), findsOneWidget);
    await command(tester, LogicalKeyboardKey.arrowUp);
    expect(session(tester).selected?.id, '0');
    expect(find.text('Entry 0').hitTestable(), findsOneWidget);
  });

  for (final language in ['en', 'zh']) {
    testWidgets('small $language shelf fits preview, actions and confirmation',
        (tester) async {
      AppLanguage.current.apply(language);
      tester.view.physicalSize = const Size(320, 300);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);
      final state = ClipboardHistorySession(History(),
          rowExtent: 48, previewEnabled: false);
      await tester.pumpWidget(LocalizedApp(
          theme: AppStyle.theme,
          builder: (context, child) => MediaQuery(
              data: MediaQuery.of(context)
                  .copyWith(textScaler: TextScaler.linear(1.5)),
              child: child!),
          home: Scaffold(
              body: Padding(
                  padding: const EdgeInsets.only(top: 24),
                  child: ListenableBuilder(
                      listenable: state,
                      builder: (context, _) => ClipboardShelfPage(
                          session: state,
                          visible: true,
                          presentation: 0,
                          onClose: () {},
                          onReturnToTarget: () async {}))))));
      await tester.pumpAndSettle();
      await command(tester, LogicalKeyboardKey.keyY);
      expect(tester.takeException(), isNull);
      await command(tester, LogicalKeyboardKey.keyK);
      for (var index = 0; index < 4; index++) {
        await tester.sendKeyEvent(LogicalKeyboardKey.arrowDown);
        await tester.pumpAndSettle();
      }
      expect(tester.takeException(), isNull);
      await tester.sendKeyEvent(LogicalKeyboardKey.escape);
      await command(tester, LogicalKeyboardKey.backspace, shift: true);
      expect(find.byType(ShelfDeletePrompt), findsOneWidget);
      expect(tester.takeException(), isNull);
      await tester.pumpWidget(const SizedBox());
      state.dispose();
    });
  }
}
