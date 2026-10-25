import 'dart:async';

import 'package:anywhere/features/application_history/registration.dart';
import 'package:anywhere/features/application_history/session.dart';
import 'package:anywhere/infrastructure/application_history_backend.dart';
import 'package:anywhere/infrastructure/desktop_environment.dart';
import 'package:anywhere/infrastructure/window_host.dart';
import 'package:anywhere/l10n/generated/app_localizations.dart';
import 'package:anywhere/settings/application_history_settings_controller.dart';
import 'package:anywhere/settings/application_history_settings_section.dart';
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
      mode: AppHistorySourceModeDto.custom,
      actionEnabled: true,
      path: '/synthetic/recent.xml'),
  AppHistorySourceDto(
      id: 'vscode',
      title: 'Visual Studio Code',
      fileHint: 'state.vscdb',
      mode: AppHistorySourceModeDto.custom,
      actionEnabled: true,
      path: '/synthetic/state.vscdb'),
];

class HistoryBackend implements AppHistoryQueryBackend {
  Future<AppHistoryDto> Function(String, String)? searchOverride;
  final calls = <String>[], used = <String>[];
  int cancelled = 0, closed = 0;
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
  final modes = <AppHistorySourceModeDto>[];
  List<AppHistorySourceDto> rows = sourceRows;
  Completer<void>? gate;
  bool fail = false;
  final actionChanges = <bool>[];
  @override
  Future<String> customDirectory() async => '/synthetic/adapters';
  @override
  Future<void> openCustomDirectory() async {}
  @override
  Future<void> setActionEnabled(String id, bool enabled) async {
    actionChanges.add(enabled);
    rows = [
      for (final row in rows)
        if (row.id != id)
          row
        else
          AppHistorySourceDto(
            id: id,
            title: row.title,
            fileHint: row.fileHint,
            mode: row.mode == AppHistorySourceModeDto.disabled && enabled
                ? AppHistorySourceModeDto.automatic
                : row.mode,
            actionEnabled: enabled,
            path: row.mode == AppHistorySourceModeDto.custom
                ? row.path
                : enabled
                    ? '/automatic/${row.fileHint}'
                    : '',
          ),
    ];
  }

  @override
  Future<List<AppHistorySourceDto>> read() async => rows;
  @override
  Future<void> configure(String id, String path,
      {AppHistorySourceModeDto mode = AppHistorySourceModeDto.custom}) async {
    writes.add('$id:$path');
    modes.add(mode);
    await gate?.future;
    if (fail) throw StateError('synthetic save failure');
    rows = [
      for (final row in rows)
        if (row.id != id)
          row
        else
          AppHistorySourceDto(
            id: id,
            title: row.title,
            fileHint: row.fileHint,
            mode: mode,
            actionEnabled: mode != AppHistorySourceModeDto.disabled,
            path: switch (mode) {
              AppHistorySourceModeDto.automatic => '/automatic/${row.fileHint}',
              AppHistorySourceModeDto.custom => path,
              AppHistorySourceModeDto.disabled => '',
            },
          ),
    ];
  }
}

void main() {
  setUp(() => DesktopEnvironment.configure(
      const DesktopEnvironment(platform: TargetPlatform.macOS)));
  tearDown(DesktopEnvironment.reset);

  for (final language in ['en', 'zh']) {
    testWidgets('history settings auto, custom and off modes work in $language',
        (tester) async {
      tester.view.physicalSize = const Size(900, 900);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);
      final backend = Preferences()
        ..rows = [
          AppHistorySourceDto(
              id: 'idea',
              title: 'IntelliJ IDEA',
              fileHint: 'recentProjects.xml',
              path: '',
              actionEnabled: true,
              mode: AppHistorySourceModeDto.automatic),
        ];
      final controller = AppHistorySettingsController(backend);
      addTearDown(controller.dispose);
      await tester.pumpWidget(MaterialApp(
        locale: Locale(language),
        localizationsDelegates: AppLocalizations.localizationsDelegates,
        supportedLocales: AppLocalizations.supportedLocales,
        theme: AppStyle.theme,
        builder: AppStyle.windowBuilder,
        home: Scaffold(
            body: SingleChildScrollView(
                child: AppHistorySettingsSection(controller: controller))),
      ));
      await tester.pumpAndSettle();
      final path = find.byKey(const Key('history-path-idea'));
      String value() => tester.widget<TextField>(path).controller!.text;
      expect(value(), isEmpty);
      expect(
          tester
              .widget<TextButton>(find.byKey(const Key('history-save-idea')))
              .onPressed,
          isNull);
      await tester.tap(find.byKey(const Key('history-auto-idea')));
      await tester.pumpAndSettle();
      expect(value(), '/automatic/recentProjects.xml');
      expect(controller.sources.single.mode, AppHistorySourceModeDto.automatic);
      await tester.enterText(path, '/custom/recent.xml');
      await tester.tap(find.byKey(const Key('history-save-idea')));
      await tester.pumpAndSettle();
      expect(value(), '/custom/recent.xml');
      expect(controller.sources.single.mode, AppHistorySourceModeDto.custom);
      await tester.enterText(path, '/unsaved/draft.xml');
      await tester.tap(find.byKey(const Key('history-auto-idea')));
      await tester.pumpAndSettle();
      expect(value(), '/automatic/recentProjects.xml',
          reason:
              'explicit auto detection replaces the draft with the resolved path');
      await tester.tap(find.byKey(const Key('history-action-idea')));
      await tester.pumpAndSettle();
      expect(value(), isEmpty);
      expect(controller.sources.single.actionEnabled, isFalse);
      expect(tester.widget<TextField>(path).enabled, isFalse);
      await tester.tap(find.byKey(const Key('history-action-idea')));
      await tester.pumpAndSettle();
      expect(value(), '/automatic/recentProjects.xml');
      expect(backend.modes, [
        AppHistorySourceModeDto.automatic,
        AppHistorySourceModeDto.custom,
        AppHistorySourceModeDto.automatic
      ]);
      expect(backend.actionChanges, [false, true]);
      expect(tester.takeException(), isNull);
    });
  }

  testWidgets('native Action input fixes the source and labels the page',
      (tester) async {
    final backend = HistoryBackend();
    final session = openAppHistory(
        '{"source_id":"vscode","title":"VS Code Recent Projects and Files","localized_titles":{"zh":"VS Code 最近项目与文件"}}',
        backend);
    await tester.pump();
    expect(session.sourceId, 'vscode');
    expect(session.labelFor('zh'), 'VS Code 最近项目与文件');
    expect(backend.calls, ['vscode:']);
    session.query.text = 'project';
    await tester.pump(const Duration(milliseconds: 130));
    await tester.pump();
    expect(backend.calls.last, 'vscode:project');
    session.dispose();
    expect(() => openAppHistory('{}', backend), throwsFormatException);
    expect(
        () => openAppHistory(
            '{"source_id":"","title":"Wrong","localized_titles":{}}', backend),
        throwsFormatException);
  });

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

  test('history changes reconcile system hotkeys even after a failed response',
      () async {
    const channel = MethodChannel('test/history-action-switch');
    final messenger =
        TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger;
    final calls = <String>[];
    messenger.setMockMethodCallHandler(channel, (call) async {
      calls.add(call.method);
      return call.method == 'settingsStatus' ? <String, Object?>{} : null;
    });
    addTearDown(() => messenger.setMockMethodCallHandler(channel, null));
    final backend = Preferences();
    final settings = SettingsController(
        window: WindowHost(channel),
        appHistoryBackend: backend,
        readExtensions: () => [],
        readShortcutActions: () async => [],
        readSupportedShortcutActions: (_) async => []);
    addTearDown(settings.dispose);
    expect(await settings.appHistory.setActionEnabled('idea', false), isTrue);
    expect(calls.where((m) => m == 'syncActionShortcutCatalog').length, 1);
    backend.fail = true;
    expect(await settings.appHistory.configure('idea', '/failed'), isFalse);
    expect(calls.where((m) => m == 'syncActionShortcutCatalog').length, 2);
  });

  testWidgets(
      'each app page stays on its source and shares row keyboard actions',
      (tester) async {
    final backend = HistoryBackend();
    final active = AppHistorySession(backend,
        sourceId: 'idea', title: 'IDEA Recent Projects');
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
    expect(active.sourceId, 'idea');
    expect(backend.calls, ['idea:']);
    expect(find.text('All apps'), findsNothing);
    expect(find.text('Visual Studio Code'), findsNothing);
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
    final session = AppHistorySession(backend,
        sourceId: 'idea', title: 'IDEA Recent Projects');
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
    final session = AppHistorySession(backend,
        sourceId: 'idea', title: 'IDEA Recent Projects');
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
    final session = AppHistorySession(backend,
        sourceId: 'idea', title: 'IDEA Recent Projects');
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
