import 'package:anywhere/features/clipboard/clipboard_history_session.dart';
import 'package:anywhere/infrastructure/clipboard_preferences_backend.dart';
import 'package:anywhere/l10n/app_language.dart';
import 'package:anywhere/l10n/clipboard_labels.dart';
import 'package:anywhere/launcher/file_search_controller.dart';
import 'package:anywhere/launcher/widgets/file_search_components.dart';
import 'package:anywhere/settings/clipboard_settings_controller.dart';
import 'package:anywhere/settings/clipboard_settings_section.dart';
import 'package:anywhere/src/rust/api.dart';
import 'package:anywhere/ui/app_style.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'support/clipboard_fixtures.dart';

class NoticePreferences implements ClipboardPreferencesBackend {
  ClipboardNoticeDto notice = ClipboardNoticeDto.accessRequired;
  @override
  Future<ClipboardHistoryStatusDto> read() async => ClipboardHistoryStatusDto(
      recording: true,
      recordingEnabled: true,
      available: true,
      notice: notice,
      retentionDays: 0,
      storedEntries: 0,
      storedBytes: 0);
  @override
  Future<void> setRecording(bool enabled) async {}
  @override
  Future<void> pause(int minutes) async {}
  @override
  Future<void> setRetention(int days) async {}
  @override
  Future<void> clear() async {}
}

const file = CandidateDto(
    id: 'file',
    kind: CandidateKindDto.file,
    title: 'test.txt',
    menuPath: [],
    contextKeys: [],
    homeEligible: false,
    primaryAction: true,
    iconKey: 'file',
    score: 1,
    groupId: 'files',
    availability: AvailabilityDto.enabled);

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  final en = lookupAppLocalizations(const Locale('en'));
  final zh = lookupAppLocalizations(const Locale('zh'));
  setUp(() => AppLanguage.current.apply('zh'));
  tearDown(() => AppLanguage.current.apply('system'));

  test('all backend clipboard notices translate in history and settings',
      () async {
    final preferences = NoticePreferences();
    final settings = ClipboardSettingsController(preferences);
    final backend = History();
    final session = ClipboardHistorySession(backend);
    addTearDown(session.dispose);
    addTearDown(settings.dispose);
    await Future<void>.delayed(Duration.zero);
    for (final notice in ClipboardNoticeDto.values) {
      backend.searchOverride = (_) async => ClipboardHistoryDto(
          recording: true, available: true, notice: notice, items: []);
      preferences.notice = notice;
      await session.refresh();
      await settings.load();
      expect(session.hasNotice, isTrue);
      expect(session.noticeFor(zh), notice.labelFor(zh));
      expect(settings.noticeFor(zh), notice.labelFor(zh));
      expect(session.noticeFor(en), isNot(session.noticeFor(zh)));
      expect(session.noticeFor(zh), matches(RegExp(r'[\u4e00-\u9fff]')));
    }
    // A successful background refresh clears a presentation error too.
    session.reportNotice('temporary failure');
    backend.searchOverride = (_) async =>
        const ClipboardHistoryDto(recording: true, available: true, items: []);
    await session.refresh(background: true);
    expect(session.noticeFor(zh), isNull);
  });

  testWidgets(
      'settings actually displays backend notices and switches language',
      (tester) async {
    final controller = ClipboardSettingsController(NoticePreferences());
    addTearDown(controller.dispose);
    await controller.load();
    await tester.pumpWidget(LocalizedApp(
        theme: AppStyle.theme,
        home: Scaffold(
            body: SingleChildScrollView(
                child: ClipboardSettingsSection(
                    controller: controller,
                    autoFill: true,
                    onAutoFill: (_) {})))));
    await tester.pumpAndSettle();
    expect(find.text(zh.historyAccessRequired), findsOneWidget);
    expect(find.text(en.historyAccessRequired), findsNothing);
    AppLanguage.current.apply('en');
    await tester.pumpAndSettle();
    expect(find.text(en.historyAccessRequired), findsOneWidget);
    expect(find.text(zh.historyAccessRequired), findsNothing);
  });

  testWidgets('file preview failures and reset follow presentation language',
      (tester) async {
    final files = FileSearchController(
        (session, revision, candidate) async => throw StateError('changed'));
    addTearDown(files.dispose);
    await tester.pumpWidget(LocalizedApp(
        theme: AppStyle.theme,
        home: Scaffold(
            body: ListenableBuilder(
                listenable: files,
                builder: (context, child) =>
                    FileSearchPreview(files: files)))));
    files.sync(1, 1, file);
    await tester.pump(const Duration(milliseconds: 200));
    await tester.pumpAndSettle();
    expect(find.text(zh.filePreviewChanged), findsOneWidget);
    AppLanguage.current.apply('en');
    await tester.pumpAndSettle();
    expect(find.text(en.filePreviewChanged), findsOneWidget);
    expect(find.text(zh.filePreviewChanged), findsNothing);
    files.reset();
    expect(files.errorFor(en), isNull);
    final unavailable = FileSearchController(null);
    addTearDown(unavailable.dispose);
    unavailable.sync(1, 1, file);
    expect(unavailable.errorFor(zh), zh.previewUnavailable);
  });

  testWidgets('truncation and encoding facts render translated preview notices',
      (tester) async {
    final files = FileSearchController(null)
      ..preview = FilePreviewDto(
          path: '/test.txt',
          isDirectory: false,
          sizeBytes: BigInt.from(100000),
          text: '中文',
          truncatedAtBytes: 65536,
          encoding: 'GBK',
          encodingGuessed: true);
    addTearDown(files.dispose);
    await tester.pumpWidget(LocalizedApp(
        theme: AppStyle.theme,
        home: Scaffold(body: FileSearchPreview(files: files))));
    await tester.pumpAndSettle();
    expect(find.text('仅显示前 64 KiB · 推测编码为 GBK'), findsOneWidget);
    AppLanguage.current.apply('en');
    await tester.pumpAndSettle();
    expect(find.text('Showing the first 64 KiB · Encoding guessed as GBK'),
        findsOneWidget);
    files.preview = FilePreviewDto(
        path: '/image.bin', isDirectory: false, sizeBytes: BigInt.from(20));
    await tester.pumpWidget(LocalizedApp(
        theme: AppStyle.theme,
        home: Scaffold(body: FileSearchPreview(files: files))));
    await tester.pumpAndSettle();
    expect(find.text(en.filePreviewUnsupported), findsOneWidget);
    AppLanguage.current.apply('zh');
    await tester.pumpAndSettle();
    expect(find.text(zh.filePreviewUnsupported), findsOneWidget);
  });
}
