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
import 'package:anywhere/settings/settings_screen.dart';
import 'package:anywhere/src/rust/api.dart';
import 'package:anywhere/tools/tool_page.dart';
import 'package:anywhere/ui/app_components.dart';
import 'package:anywhere/ui/app_style.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

AppHistoryItemDto item(String id,
        {String source = 'idea', bool canOpen = true}) =>
    AppHistoryItemDto(
        id: id,
        objectKey: id,
        sourceId: source,
        title: 'Project $id',
        value: '/synthetic/$id',
        kind: 'directory',
        canOpen: canOpen);
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
  Future<List<CandidateDto>> Function(String)? actionOverride;
  final actionCalls = <String>[];
  @override
  Future<List<CandidateDto>> actions(String id,
      {required int sessionId, required int revision}) async {
    actionCalls.add('$sessionId:$revision:$id');
    return await actionOverride?.call(id) ?? [];
  }

  Future<AppHistoryDto> Function(String, String)? searchOverride;
  Future<void> Function()? useOverride, cancelOverride;
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
    await useOverride?.call();
  }

  @override
  Future<void> cancel() async {
    cancelled++;
    await cancelOverride?.call();
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
            actionEnabled:
                mode != AppHistorySourceModeDto.disabled && row.actionEnabled,
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

  test(
      'saving drafts retains newer edits and does not refresh the catalog for a folder',
      () async {
    final backend = Preferences()..gate = Completer<void>();
    final controller = AppHistorySettingsController(backend);
    addTearDown(controller.dispose);
    await controller.load();
    final source = controller.sources.first;
    controller.editPath(source, '/submitted');
    final saving = controller.saveDrafts();
    await Future<void>.delayed(Duration.zero);
    controller.editPath(source, '/newer');
    backend.gate!.complete();
    expect(await saving, isTrue);
    expect(controller.draftPath(controller.sources.first), '/newer');
    expect(controller.hasDrafts, isTrue);
    var changes = 0;
    controller.onCatalogChanged = () async {
      changes++;
    };
    await controller.openCustomDirectory();
    expect(changes, 0);
    controller.discardDrafts();
    expect(controller.hasDrafts, isFalse);
  });

  testWidgets(
      'source filters and remounts preserve path drafts; revert restores the saved path',
      (tester) async {
    final controller = AppHistorySettingsController(Preferences());
    addTearDown(controller.dispose);
    Widget page(bool visible) => MaterialApp(
        theme: AppStyle.theme,
        home: Scaffold(
            body: SingleChildScrollView(
                child: visible
                    ? AppHistorySettingsSection(controller: controller)
                    : const SizedBox())));
    await tester.pumpWidget(page(true));
    await tester.pumpAndSettle();
    expect(find.byKey(const Key('history-path-idea')), findsNothing);
    await tester.tap(find.text('IntelliJ IDEA'));
    await tester.pumpAndSettle();
    final path = find.byKey(const Key('history-path-idea'));
    await tester.enterText(path, '/unsaved/draft.xml');
    await tester.enterText(
        find.byKey(const Key('history-source-filter')), 'no such app');
    await tester.pumpAndSettle();
    expect(find.text('Clear filters and show all sources'), findsOneWidget);
    await tester.pumpWidget(page(false));
    await tester.pumpWidget(page(true));
    await tester.pumpAndSettle();
    expect(
        tester
            .widget<TextField>(find.byKey(const Key('history-source-filter')))
            .controller!
            .text,
        'no such app');
    await tester.tap(find.byKey(const Key('history-clear-filter')));
    await tester.pumpAndSettle();
    expect(
        tester.widget<TextField>(path).controller!.text, '/unsaved/draft.xml');
    await tester.ensureVisible(find.byKey(const Key('history-revert-idea')));
    await tester.tap(find.byKey(const Key('history-revert-idea')));
    await tester.pumpAndSettle();
    expect(tester.widget<TextField>(path).controller!.text,
        '/synthetic/recent.xml');
    expect(controller.hasDrafts, isFalse);
  });

  for (final choice in [
    'Keep editing',
    'Close without saving',
    'Save changes'
  ]) {
    testWidgets('settings close handles a history draft with $choice',
        (tester) async {
      tester.view.physicalSize = const Size(900, 900);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);
      final backend = Preferences();
      final settings = SettingsController(
          window: WindowHost(const MethodChannel('unused/history-close')),
          appHistoryBackend: backend,
          readExtensions: () => []);
      settings.appHistory.onCatalogChanged = () async {};
      addTearDown(settings.dispose);
      await tester.pumpWidget(MaterialApp(
          theme: AppStyle.theme,
          home: Scaffold(
              body: SettingsScreen(
                  controller: settings, onClose: settings.prepareClose))));
      await tester.tap(find.byKey(const ValueKey('settings-app_history')));
      await tester.pumpAndSettle();
      await tester.tap(find.text('IntelliJ IDEA'));
      await tester.pumpAndSettle();
      await tester.enterText(
          find.byKey(const Key('history-path-idea')), '/draft/history.xml');
      await tester.tap(find.byKey(const ValueKey('settings-general')));
      await tester.pumpAndSettle();
      final closing = settings.prepareClose();
      await tester.pumpAndSettle();
      expect(find.text('Save changes before closing?'), findsOneWidget);
      await tester.tap(find.text(choice));
      await tester.pumpAndSettle();
      expect(await closing, choice != 'Keep editing');
      expect(settings.appHistory.hasDrafts, choice == 'Keep editing');
      expect(backend.writes,
          choice == 'Save changes' ? ['idea:/draft/history.xml'] : isEmpty);
      await tester.pumpWidget(const SizedBox());
    });
  }

  testWidgets(
      'a failed save-and-close reveals the affected source and retains its draft',
      (tester) async {
    final backend = Preferences()..fail = true;
    final settings = SettingsController(
        window: WindowHost(const MethodChannel('unused/history-close-failure')),
        appHistoryBackend: backend,
        readExtensions: () => []);
    settings.appHistory.onCatalogChanged = () async {};
    addTearDown(settings.dispose);
    await settings.appHistory.load();
    settings.appHistory
        .editPath(settings.appHistory.sources.first, '/bad/path.xml');
    await tester.pumpWidget(MaterialApp(
        theme: AppStyle.theme,
        home: Scaffold(
            body: SettingsScreen(
                controller: settings, onClose: settings.prepareClose))));
    final closing = settings.prepareClose();
    await tester.pumpAndSettle();
    await tester.tap(find.text('Save changes'));
    await tester.pumpAndSettle();
    expect(await closing, isFalse);
    expect(find.byKey(const Key('history-path-idea')), findsOneWidget);
    expect(settings.appHistory.hasDrafts, isTrue);
    expect(settings.appHistory.failedSource, 'idea');
    expect(tester.takeException(), isNull);
    await tester.pumpWidget(const SizedBox());
  });

  testWidgets(
      'a disabled source can be configured with Enter and keeps editor focus',
      (tester) async {
    final backend = Preferences();
    final controller = AppHistorySettingsController(backend);
    addTearDown(controller.dispose);
    await controller.load();
    await controller.setActionEnabled('idea', false);
    await tester.pumpWidget(MaterialApp(
        theme: AppStyle.theme,
        home: Scaffold(
            body: SingleChildScrollView(
                child: AppHistorySettingsSection(controller: controller)))));
    await tester.pumpAndSettle();
    await tester.tap(find.text('IntelliJ IDEA'));
    await tester.pumpAndSettle();
    final path = find.byKey(const Key('history-path-idea'));
    await tester.enterText(path, '/prepared/while-off.xml');
    await tester.testTextInput.receiveAction(TextInputAction.done);
    await tester.pumpAndSettle();
    expect(controller.sources.first.actionEnabled, isFalse);
    expect(controller.sources.first.path, '/prepared/while-off.xml');
    expect(controller.hasDrafts, isFalse);
    expect(tester.widget<TextField>(path).focusNode!.hasFocus, isTrue);
    await tester.tap(find.byKey(const Key('history-filter-enabled')));
    await tester.pumpAndSettle();
    expect(find.text('IntelliJ IDEA'), findsNothing);
    await tester.tap(find.byKey(const Key('history-filter-all')));
    await tester.pumpAndSettle();
    expect(find.text('IntelliJ IDEA'), findsOneWidget);
    expect(tester.takeException(), isNull);
  });

  testWidgets(
      'history supports page navigation, clear-then-back Escape and copying remote rows',
      (tester) async {
    final backend = HistoryBackend()
      ..searchOverride = (_, __) async => AppHistoryDto(items: [
            item('remote', canOpen: false),
            ...List.generate(30, (i) => item('$i'))
          ], failedSources: const []);
    final session = AppHistorySession(backend,
        sourceId: 'vscode', title: 'VS Code', rowExtent: 80);
    var backs = 0;
    await tester.pumpWidget(MaterialApp(
        theme: AppStyle.theme,
        home: Scaffold(
            body: ListenableBuilder(
                listenable: session,
                builder: (context, _) => session.build(context,
                    onBack: () => backs++, presentation: 1, visible: true)))));
    await tester.pumpAndSettle();
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pumpAndSettle();
    expect(backend.used, ['remote:true']);
    await tester.sendKeyEvent(LogicalKeyboardKey.pageDown);
    expect(tester.widget<ListView>(find.byType(ListView)).itemExtent, 80);
    expect(session.selectedIndex,
        (session.scroll.position.viewportDimension / 80).floor());
    await tester.sendKeyEvent(LogicalKeyboardKey.pageUp);
    expect(session.selectedIndex, 0);
    session.query.text = 'query';
    await tester.pump(const Duration(milliseconds: 130));
    await tester.pumpAndSettle();
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pump(const Duration(milliseconds: 130));
    await tester.pumpAndSettle();
    expect(session.query.text, isEmpty);
    expect(backs, 0);
    session.query.text = 'another query';
    await tester.pump(const Duration(milliseconds: 130));
    await tester.pumpAndSettle();
    await tester.tap(find.text('esc'));
    await tester.pump(const Duration(milliseconds: 130));
    await tester.pumpAndSettle();
    expect(session.query.text, isEmpty);
    expect(backs, 0,
        reason: 'the Esc control follows the same rule as the key');
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    expect(backs, 1);
    await tester.pumpWidget(const SizedBox());
    session.dispose();
  });

  testWidgets('custom adapters reopen independently of settings scroll state',
      (tester) async {
    final controller = AppHistorySettingsController(Preferences());
    addTearDown(controller.dispose);
    final bucket = PageStorageBucket();
    Widget page({required bool showHistory}) => MaterialApp(
          localizationsDelegates: AppLocalizations.localizationsDelegates,
          supportedLocales: AppLocalizations.supportedLocales,
          theme: AppStyle.theme,
          home: Scaffold(
              body: PageStorage(
                  bucket: bucket,
                  child: SingleChildScrollView(
                      key: const PageStorageKey('settings-content-history'),
                      child: Builder(builder: (context) {
                        if (showHistory) {
                          return AppHistorySettingsSection(
                              controller: controller);
                        }
                        // A settings page can already have a saved scroll offset
                        // before its secondary details are mounted again.
                        bucket.writeState(context, 80.0);
                        return const SizedBox(height: 1200);
                      })))),
        );
    await tester.pumpWidget(page(showHistory: false));
    await tester.pumpWidget(page(showHistory: true));
    await tester.pumpAndSettle();
    expect(tester.takeException(), isNull);
    final details = find.text('Custom adapters');
    await tester.ensureVisible(details);
    await tester.tap(details);
    await tester.pumpAndSettle();
    expect(find.text('/synthetic/adapters'), findsOneWidget);
    await tester.pumpWidget(page(showHistory: false));
    await tester.pumpWidget(page(showHistory: true));
    await tester.pumpAndSettle();
    expect(tester.takeException(), isNull);
    expect(find.text('/synthetic/adapters'), findsOneWidget,
        reason: 'expansion state survives without replacing the scroll offset');
  });

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
      final filter = find.byKey(const Key('history-source-filter'));
      await tester.enterText(filter, 'missing app');
      await tester.pumpAndSettle();
      expect(find.byKey(const Key('history-path-idea')), findsNothing);
      await tester.enterText(filter, 'IDEA');
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
      expect(tester.widget<TextField>(path).enabled, isTrue,
          reason: 'a location can be prepared without enabling its action');
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

  for (final finishBeforeClose in [false, true]) {
    testWidgets(
        'failed source save has one close decision (completed: $finishBeforeClose)',
        (tester) async {
      final backend = Preferences()
        ..gate = Completer<void>()
        ..fail = true;
      final settings = SettingsController(
          window: WindowHost(const MethodChannel('unused/history-close-order')),
          appHistoryBackend: backend,
          readExtensions: () => []);
      settings.appHistory.onCatalogChanged = () async {};
      addTearDown(settings.dispose);
      await tester.pumpWidget(MaterialApp(
          theme: AppStyle.theme,
          home: Scaffold(
              body: SettingsScreen(
                  controller: settings, onClose: settings.prepareClose))));
      final write = settings.appHistory.configure('idea', '/bad/path');
      if (finishBeforeClose) {
        backend.gate!.complete();
        expect(await write, isFalse);
      }
      final closing = settings.prepareClose();
      expect(identical(closing, settings.prepareClose()), isTrue);
      if (!finishBeforeClose) {
        await tester.pump();
        expect(find.byType(AlertDialog), findsNothing);
        backend.gate!.complete();
      }
      await tester.pumpAndSettle();
      expect(await write, isFalse);
      expect(find.byType(AlertDialog), findsOneWidget);
      await tester.tap(find.text('Close without saving'));
      await tester.pumpAndSettle();
      expect(await closing, isTrue);
      expect(tester.takeException(), isNull);
      await tester.pumpWidget(const SizedBox());
    });
  }

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
      'history keeps source navigation without binding the more-actions shortcut',
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
    expect(find.text('Copy path or URI'), findsNothing);
    expect(backend.actionCalls, isEmpty);
    expect(find.text('⌘K'), findsNothing);
    await tester.tap(find.byType(AppMoreAction));
    await tester.pump(const Duration(milliseconds: 350));
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
      'object actions use the host and reject delayed results for an edited query',
      (tester) async {
    const action = CandidateDto(
        id: 'guarded-copy',
        commandId: 'core.directory.copy',
        kind: CandidateKindDto.action,
        title: 'Copy Path',
        score: 1,
        availability: AvailabilityDto.enabled,
        groupId: 'folder',
        iconKey: 'copy',
        menuPath: [],
        contextKeys: [],
        primaryAction: false);
    final backend = HistoryBackend()..actionOverride = (_) async => [action];
    final active =
        AppHistorySession(backend, sourceId: 'idea', title: 'IDEA History');
    final executed = <String>[];
    active.bindHost(ToolPageHost(
        sessionId: 7,
        revision: 3,
        execute: (candidate) async {
          executed.add(candidate.id);
          return true;
        }));
    await tester.pumpWidget(MaterialApp(
        theme: AppStyle.theme,
        home: Scaffold(
            body: ListenableBuilder(
                listenable: active,
                builder: (context, _) => active.build(context,
                    onBack: () {}, presentation: 1, visible: true)))));
    await tester.pumpAndSettle();
    await tester.tap(find.byType(AppMoreAction));
    await tester.pump(const Duration(milliseconds: 350));
    await tester.pumpAndSettle();
    expect(backend.actionCalls, ['7:3:1']);
    expect(find.text('Copy Path'), findsOneWidget);
    expect(find.text('Copy path or URI'), findsNothing);
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pumpAndSettle();
    expect(executed, ['guarded-copy']);
    backend.actionOverride =
        (_) async => throw StateError('unavailable object');
    await tester.tap(find.byType(AppMoreAction));
    await tester.pump(const Duration(milliseconds: 350));
    await tester.pumpAndSettle();
    expect(find.text('Copy path or URI'), findsOneWidget);
    expect(find.text('Refresh'), findsOneWidget);
    final reads = backend.calls.length;
    await tester.sendKeyEvent(LogicalKeyboardKey.arrowDown);
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pumpAndSettle();
    expect(backend.calls.length, reads + 1);
    expect(active.failed, isFalse);
    final pending = Completer<List<CandidateDto>>();
    backend.actionOverride = (_) => pending.future;
    final loading = active.prepareActions();
    active.query.text = 'new query';
    await tester.pump(const Duration(milliseconds: 150));
    pending.complete([action]);
    expect(await loading, isFalse);
    await tester.pumpAndSettle();
    expect(active.objectActions, isEmpty);
    await active.runAction(action);
    expect(executed, ['guarded-copy']);
    expect(backend.calls.last, 'idea:new query');
    await tester.pumpWidget(const SizedBox());
    active.dispose();
  });

  testWidgets(
      'a query committed during an operation runs after the operation finishes',
      (tester) async {
    final operation = Completer<void>();
    final backend = HistoryBackend()..useOverride = () => operation.future;
    final session = AppHistorySession(backend,
        sourceId: 'idea', title: 'IDEA Recent Projects');
    await tester.pump();
    final using = session.useItem();
    session.query.text = 'latest query';
    await tester.pump(const Duration(milliseconds: 150));
    expect(backend.calls, ['idea:']);
    operation.complete();
    await using;
    await tester.pump();
    expect(backend.calls.last, 'idea:latest query');
    expect(session.loading, isFalse);
    session.dispose();
  });

  testWidgets('all pending cancellations finish before the next read',
      (tester) async {
    final firstCancel = Completer<void>();
    final backend = HistoryBackend();
    final session = AppHistorySession(backend,
        sourceId: 'idea', title: 'IDEA Recent Projects');
    await tester.pump();
    backend.cancelOverride =
        () => backend.cancelled == 1 ? firstCancel.future : Future.value();
    session.query.text = 'first';
    await tester.pump();
    session.query.text = 'latest';
    await tester.pump(const Duration(milliseconds: 150));
    expect(backend.calls, ['idea:'],
        reason: 'a delayed old cancel must not invalidate the latest read');
    firstCancel.complete();
    await tester.pump();
    expect(backend.calls.last, 'idea:latest');
    session.dispose();
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

  for (final rowExtent in [52.0, 80.0]) {
    testWidgets(
        'refresh preserves and reveals identity with row height $rowExtent',
        (tester) async {
      var rows = List.generate(40, (index) => item('$index'));
      final backend = HistoryBackend()
        ..searchOverride = (_, __) async =>
            AppHistoryDto(items: rows, failedSources: const []);
      final session = AppHistorySession(backend,
          sourceId: 'idea',
          title: 'IDEA Recent Projects',
          rowExtent: rowExtent);
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
          greaterThanOrEqualTo(31 * rowExtent));
      final viewport = tester.getRect(find.byType(ListView));
      final selected = tester.getRect(find.text(session.selected!.title));
      expect(selected.top, greaterThanOrEqualTo(viewport.top));
      expect(selected.bottom, lessThanOrEqualTo(viewport.bottom));
      session.query.text = 'new query';
      await tester.pump(const Duration(milliseconds: 130));
      await tester.pumpAndSettle();
      expect(session.selectedIndex, 0);
      expect(session.scroll.offset, 0);
      await tester.pumpWidget(const SizedBox());
      session.dispose();
    });
  }

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
