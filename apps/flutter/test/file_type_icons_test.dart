import 'dart:io';
import 'dart:ui' as ui;
import 'package:anywhere/infrastructure/desktop_environment.dart';
import 'package:anywhere/launcher/file_search_controller.dart';
import 'package:anywhere/launcher/widgets/candidate_icon.dart';
import 'package:anywhere/launcher/widgets/candidate_row.dart';
import 'package:anywhere/launcher/widgets/file_search_components.dart';
import 'package:anywhere/src/rust/api.dart';
import 'package:anywhere/ui/app_icons.dart';
import 'package:anywhere/ui/app_style.dart';
import 'package:anywhere/ui/file_type_appearance.dart';
import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter_test/flutter_test.dart';
import 'support/launcher_ui_capture.dart';

CandidateDto candidate(String name,
        {bool directory = false, bool action = false}) =>
    CandidateDto(
        id: name,
        // Object type and action row role are independent.
        kind: action ? CandidateKindDto.action : CandidateKindDto.file,
        fileKind:
            directory ? FileObjectKindDto.directory : FileObjectKindDto.file,
        title: name,
        menuPath: const [],
        contextKeys: const [],
        primaryAction: true,
        iconKey: action
            ? 'copy'
            : directory
                ? 'folder'
                : 'file',
        commandId: directory ? 'core.directory.open' : 'core.file.open',
        score: 1,
        groupId: 'files',
        subtitle: '/project',
        availability: AvailabilityDto.enabled);

void main() {
  setUpAll(loadCaptureFonts);
  setUp(() => DesktopEnvironment.configure(
      const DesktopEnvironment(platform: TargetPlatform.macOS)));
  tearDown(DesktopEnvironment.reset);

  testWidgets('file hints never replace directory or action semantics',
      (tester) async {
    await tester.pumpWidget(MaterialApp(
        home: Row(children: [
      CandidateIcon(candidate: candidate('照片.PDF', directory: true)),
      CandidateIcon(
          candidate: candidate('Copy archive.zip', action: true),
          fileObject: false),
      CandidateIcon(candidate: candidate('Résumé.PDF')),
      CandidateIcon(candidate: candidate('.unknown')),
    ])));
    expect(find.byIcon(AppIcons.folder), findsOneWidget);
    expect(find.byIcon(AppIcons.copy), findsOneWidget);
    expect(find.byIcon(AppIcons.pdf), findsOneWidget);
    expect(find.byIcon(AppIcons.file), findsOneWidget);
    expect(tester.takeException(), isNull);
  });

  testWidgets('row and preview share type appearance without changing actions',
      (tester) async {
    final files = FileSearchController(null)
      ..preview = FilePreviewDto(
          path: '/project/预算.PDF',
          isDirectory: false,
          sizeBytes: BigInt.from(42));
    addTearDown(files.dispose);
    await tester.pumpWidget(MaterialApp(
        theme: AppStyle.theme,
        home: Column(children: [
          CandidateRow(
              candidate: candidate('预算.PDF'), selected: true, onTap: () {}),
          Expanded(child: FileSearchPreview(files: files)),
        ])));
    final icons = tester.widgetList<FileTypeIcon>(find.byType(FileTypeIcon));
    expect(icons, hasLength(2));
    expect(
        icons.map((icon) => icon.appearance).toSet(), {FileTypeAppearance.pdf});
    expect(find.text('PDF document'), findsOneWidget);
    expect(tester.takeException(), isNull);
  });

  if (captureLauncherUi) {
    testWidgets('capture file icon vocabulary in actual result rows',
        (tester) async {
      tester.view.physicalSize = const Size(900, 770);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);
      const samples = [
        'Project',
        'Report.pdf',
        'Draft.docx',
        'Notes.md',
        'Budget.xlsx',
        'Slides.key',
        'Photo.heic',
        'Song.flac',
        'Clip.mov',
        'Backup.tar.gz',
        'main.rs',
        '.env.production',
        'Book.epub',
        'Font.woff2',
        'Installer.dmg',
        'Unknown.custom'
      ];
      await tester.pumpWidget(MaterialApp(
          theme: AppStyle.theme,
          home: RepaintBoundary(
              key: const Key('file-icons-capture'),
              child: ColoredBox(
                  color: const Color(0xff343740),
                  child: Padding(
                      padding: const EdgeInsets.all(16),
                      child: Row(children: [
                        for (final names in [samples.take(8), samples.skip(8)])
                          Expanded(
                              child: Column(children: [
                            for (final name in names)
                              CandidateRow(
                                  candidate: candidate(name,
                                      directory: name == 'Project'),
                                  selected: name == 'Report.pdf',
                                  onTap: () {}),
                          ])),
                      ]))))));
      await tester.pumpAndSettle();
      expect(tester.takeException(), isNull);
      await tester.runAsync(() async {
        final boundary = tester.renderObject<RenderRepaintBoundary>(
            find.byKey(const Key('file-icons-capture')));
        final image = await boundary.toImage(pixelRatio: 2);
        final bytes = await image.toByteData(format: ui.ImageByteFormat.png);
        final path = File('../../build/ui-v0.7-build16/file-type-icons.png');
        await path.parent.create(recursive: true);
        await path.writeAsBytes(bytes!.buffer.asUint8List());
        image.dispose();
      });
    });
  }
}
