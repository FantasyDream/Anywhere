import 'dart:async';

import 'package:anywhere/features/application_history/session.dart';
import 'package:anywhere/infrastructure/application_history_backend.dart';
import 'package:anywhere/infrastructure/desktop_environment.dart';
import 'package:anywhere/infrastructure/window_host.dart';
import 'package:anywhere/settings/application_history_settings_controller.dart';
import 'package:anywhere/settings/settings_controller.dart';
import 'package:anywhere/src/rust/api.dart';
import 'package:anywhere/ui/app_style.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

AppHistoryItemDto item(String id, {String source = 'idea'}) =>
    AppHistoryItemDto(
        id: id,
        objectKey: id,
        sourceId: source,
        title: 'Project $id',
        value: '/synthetic/$id',
        kind: 'directory',
        canOpen: true);
final sourceRows = [
  AppHistorySourceDto(
      id: 'idea',
      title: 'IntelliJ IDEA',
      fileHint: 'recentProjects.xml',
      path: '/synthetic/recent.xml'),
  AppHistorySourceDto(
      id: 'vscode',
      title: 'Visual Studio Code',
      fileHint: 'state.vscdb',
      path: '/synthetic/state.vscdb'),
];

class HistoryBackend implements AppHistoryQueryBackend {
  Future<AppHistoryDto> Function(String, String)? searchOverride;
  final calls = <String>[], used = <String>[];
  int cancelled = 0, closed = 0;
  @override
  Future<List<AppHistorySourceDto>> sources() async => sourceRows;
  @override
  Future<AppHistoryDto> search(String query, String source) async {
    calls.add('$source:$query');
    return searchOverride == null
        ? AppHistoryDto(items: [item('1'), item('2')], failedSources: const [])
        : await searchOverride!(query, source);
  }

  @override
  Future<void> useItem(String id, {required bool copy}) async {
    used.add('$id:$copy');
  }

  @override
  Future<void> cancel() async {
    cancelled++;
  }

  @override
  Future<void> close() async {
    closed++;
  }
}

class Preferences implements AppHistoryPreferencesBackend {
  final writes = <String>[];
  Completer<void>? gate;
  bool fail = false;
  @override
  Future<List<AppHistorySourceDto>> read() async => sourceRows;
  @override
  Future<void> configure(String id, String path) async {
    writes.add('$id:$path');
    await gate?.future;
    if (fail) throw StateError('synthetic save failure');
  }
}

void main() {
  setUp(() => DesktopEnvironment.configure(
      const DesktopEnvironment(platform: TargetPlatform.macOS)));
  tearDown(DesktopEnvironment.reset);

  test('a failed history save prevents settings close', () async {
    final backend = Preferences()
      ..gate = Completer<void>()
      ..fail = true;
    final settings = SettingsController(
        window: WindowHost(const MethodChannel('unused/history-settings')),
        appHistoryBackend: backend,
        readExtensions: () => []);
    addTearDown(settings.dispose);
    final write = settings.appHistory.configure('idea', '/synthetic');
    final close = settings.prepareClose();
    backend.gate!.complete();
    expect(await write, isFalse);
    expect(await close, isFalse);
  });

  testWidgets(
      'Tab filters, arrow selection and Return share the launcher keyboard model',
      (tester) async {
    final backend = HistoryBackend();
    final active = AppHistorySession(backend);
    await tester.pumpWidget(MaterialApp(
        theme: AppStyle.theme,
        home: Scaffold(
            body: ListenableBuilder(
                listenable: active,
                builder: (context, _) => active.build(context,
                    onBack: () {}, presentation: 1, visible: true)))));
    await tester.pumpAndSettle();
    await tester.sendKeyEvent(LogicalKeyboardKey.tab);
    await tester.pumpAndSettle();
    expect(active.filter, 'idea');
    expect(backend.calls.last, 'idea:');
    await tester.sendKeyEvent(LogicalKeyboardKey.arrowDown);
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pumpAndSettle();
    expect(backend.used, ['2:false']);
    await tester.sendKeyDownEvent(LogicalKeyboardKey.metaLeft);
    await tester.sendKeyEvent(LogicalKeyboardKey.keyK);
    await tester.sendKeyUpEvent(LogicalKeyboardKey.metaLeft);
    await tester.pumpAndSettle();
    expect(find.text('Copy path or URI'), findsOneWidget);
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pumpAndSettle();
    expect(backend.used, ['2:false', '2:true']);
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pumpAndSettle();
    expect(active.query.text, '');
    expect(tester.takeException(), isNull);
    await tester.pumpWidget(const SizedBox());
    active.dispose();
  });

  testWidgets(
      'IME composition blocks search and stale response publication then resumes committed input',
      (tester) async {
    final backend = HistoryBackend();
    final session = AppHistorySession(backend);
    await tester.pump();
    final pending = Completer<AppHistoryDto>();
    backend.searchOverride = (_, __) => pending.future;
    session.query.text = 'old';
    await tester.pump(const Duration(milliseconds: 130));
    session.query.value = const TextEditingValue(
        text: 'peizhi', composing: TextRange(start: 0, end: 6));
    pending.complete(
        AppHistoryDto(items: [item('stale')], failedSources: const []));
    await tester.pump(const Duration(milliseconds: 200));
    expect(session.items.any((i) => i.id == 'stale'), isFalse);
    final before = backend.calls.length;
    await session.refresh();
    expect(backend.calls.length, before);
    backend.searchOverride = (_, __) async =>
        AppHistoryDto(items: [item('committed')], failedSources: const []);
    session.query.value = const TextEditingValue(
        text: '配置验收测试', selection: TextSelection.collapsed(offset: 6));
    await tester.pump(const Duration(milliseconds: 150));
    await tester.pump();
    expect(session.selected?.id, 'committed');
    session.dispose();
  });

  test(
      'hidden pages reject results and actions, resume retains query and close releases the backend',
      () async {
    final backend = HistoryBackend();
    final session = AppHistorySession(backend);
    await Future<void>.delayed(Duration.zero);
    session.query.text = 'project';
    session.setVisible(false);
    await session.useItem();
    expect(backend.used, isEmpty);
    session.setVisible(true);
    await session.refresh();
    await Future<void>.delayed(Duration.zero);
    expect(session.query.text, 'project');
    expect(session.items.length, 2);
    session.dispose();
    await Future<void>.delayed(Duration.zero);
    expect(backend.closed, 1);
  });

  testWidgets(
      'new queries reset selection while refresh preserves and reveals its identity',
      (tester) async {
    var rows = List.generate(40, (index) => item('$index'));
    final backend = HistoryBackend()
      ..searchOverride =
          (_, __) async => AppHistoryDto(items: rows, failedSources: const []);
    final session = AppHistorySession(backend);
    await tester.pumpWidget(MaterialApp(
        theme: AppStyle.theme,
        home: Scaffold(
            body: ListenableBuilder(
                listenable: session,
                builder: (context, _) => session.build(context,
                    onBack: () {}, presentation: 1, visible: true)))));
    await tester.pumpAndSettle();
    session.select(20);
    rows = [...List.generate(10, (index) => item('new-$index')), ...rows];
    await session.refresh();
    await tester.pumpAndSettle();
    expect(session.selected?.objectKey, '20');
    expect(session.selectedIndex, 30);
    expect(session.scroll.offset, greaterThan(0));
    expect(session.scroll.offset + session.scroll.position.viewportDimension,
        greaterThanOrEqualTo(31 * 64));
    session.query.text = 'new query';
    await tester.pump(const Duration(milliseconds: 130));
    await tester.pumpAndSettle();
    expect(session.selectedIndex, 0);
    expect(session.scroll.offset, 0);
    session.select(20);
    session.selectFilter('idea');
    await tester.pumpAndSettle();
    expect(session.selectedIndex, 0);
    expect(session.scroll.offset, 0);
    await tester.pumpWidget(const SizedBox());
    session.dispose();
  });

  test('settings serializes writes and closing waits for the final readback',
      () async {
    final backend = Preferences();
    final active = AppHistorySettingsController(backend);
    backend.gate = Completer<void>();
    final first = active.configure('idea', '/one');
    final second = active.configure('vscode', '/two');
    var complete = false;
    final closing = active.waitForPending().then((value) {
      complete = value;
    });
    await Future<void>.delayed(Duration.zero);
    expect(backend.writes, ['idea:/one']);
    expect(complete, isFalse);
    backend.gate!.complete();
    await Future.wait([first, second, closing]);
    expect(backend.writes, ['idea:/one', 'vscode:/two']);
    expect(complete, isTrue);
    active.dispose();
  });
}
