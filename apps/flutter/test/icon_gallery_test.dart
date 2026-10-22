import 'dart:io';
import 'dart:ui' as ui;
import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:anywhere/ui/action_icon_appearance.dart';
import 'package:anywhere/ui/app_components.dart';
import 'package:anywhere/ui/app_icons.dart';
import 'package:anywhere/ui/app_style.dart';
import 'package:anywhere/ui/file_type_appearance.dart';
import 'package:anywhere/ui/semantic_icon.dart';
import 'package:anywhere/infrastructure/desktop_environment.dart';
import 'support/launcher_ui_capture.dart';

// Opt-in, real-font design sheet. No app window, native glass or clipboard I/O.
void main() {
  if (!captureLauncherUi) return;
  setUpAll(loadCaptureFonts);
  setUp(() => DesktopEnvironment.configure(
      const DesktopEnvironment(platform: TargetPlatform.macOS)));
  tearDown(DesktopEnvironment.reset);
  testWidgets('render icon vocabulary at result and compact sizes',
      (tester) async {
    tester.view.physicalSize = const Size(1160, 1110);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    final controls = <String, IconData>{
      'Search': AppIcons.search,
      'Back': AppIcons.back,
      'Enter': AppIcons.enter,
      'Settings': AppIcons.settings,
      'Configuration': AppIcons.configuration,
      'Context': AppIcons.context,
      'Features': AppIcons.features,
      'Plugin': AppIcons.plugin,
      'Package': AppIcons.package,
      'Keyboard': AppIcons.keyboard,
      'Context shortcut': AppIcons.route,
      'Menus': AppIcons.menu,
      'Copy': AppIcons.copy,
      'Clipboard': AppIcons.clipboard,
      'History': AppIcons.history,
      'Shelf': AppIcons.shelf,
      'Preview': AppIcons.preview,
      'Edit': AppIcons.edit,
      'Delete': AppIcons.delete,
      'Close': AppIcons.close,
      'Running': AppIcons.running,
      'Stopped': AppIcons.paused,
      'Warning': AppIcons.warning,
      'Error': AppIcons.error,
    };
    const actions = [
      'file-search',
      'folder-open',
      'file-open',
      'file-reveal',
      'terminal',
      'copy-path',
      'copy-url',
      'copy-selection',
      'clipboard-history',
      'clipboard-shelf',
      'git-status',
      'git-log',
      'git-diff',
      'git-staged',
      'git-branches',
      'copy-branch',
      'copy-commit',
      'copy-short-commit',
      'developer-check',
      'developer-test',
      'developer-build',
      'developer-package',
      'developer-run',
      'developer-script',
    ];
    Widget pair(Widget normal, Widget small) =>
        Row(mainAxisSize: MainAxisSize.min, children: [
          normal,
          const SizedBox(width: 9),
          ColorFiltered(
              colorFilter: const ColorFilter.matrix([
                .2126,
                .7152,
                .0722,
                0,
                0,
                .2126,
                .7152,
                .0722,
                0,
                0,
                .2126,
                .7152,
                .0722,
                0,
                0,
                0,
                0,
                0,
                1,
                0
              ]),
              child: small)
        ]);
    Widget tile(String label, Widget icon, int index) => SizedBox(
        height: 39,
        child: AppSelectionSurface(
            selected: index % 4 == 0,
            child: Padding(
                padding: const EdgeInsets.symmetric(horizontal: 8),
                child: Row(children: [
                  icon,
                  const SizedBox(width: 12),
                  Expanded(
                      child: Text(label,
                          style: AppStyle.body,
                          maxLines: 1,
                          overflow: TextOverflow.ellipsis))
                ]))));
    Widget column(String title, List<Widget> rows) => Expanded(
            child:
                Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
          Text(title, style: AppStyle.heading),
          const SizedBox(height: 12),
          ...rows,
        ]));
    Widget action(String key, double size) {
      final value = ActionIconAppearance.forKey(key);
      return SemanticIcon(
          glyph: value.glyph,
          tint: value.tint,
          size: size,
          mark: value.mark,
          stamp: value.stamp);
    }

    await tester.pumpWidget(MaterialApp(
        theme: AppStyle.theme,
        home: Scaffold(
            body: RepaintBoundary(
                key: const Key('icons-gallery'),
                child: ColoredBox(
                    color: const Color(0xff34383e),
                    child: Padding(
                        padding: const EdgeInsets.all(24),
                        child: Column(
                            crossAxisAlignment: CrossAxisAlignment.start,
                            children: [
                              Text('Anywhere · Icon vocabulary',
                                  style:
                                      AppStyle.heading.copyWith(fontSize: 22)),
                              const SizedBox(height: 8),
                              const Text(
                                  '32 px in color · 24 px in grayscale · transparent selection',
                                  style: AppStyle.caption),
                              const SizedBox(height: 24),
                              Expanded(
                                  child: Row(
                                      crossAxisAlignment:
                                          CrossAxisAlignment.start,
                                      children: [
                                    column('Files', [
                                      for (final type
                                          in FileTypeAppearance.values)
                                        tile(
                                            type.name,
                                            pair(
                                                FileTypeIcon(
                                                    appearance: type, size: 32),
                                                FileTypeIcon(
                                                    appearance: type,
                                                    size: 24)),
                                            type.index)
                                    ]),
                                    const SizedBox(width: 20),
                                    column('Actions', [
                                      for (var index = 0;
                                          index < actions.length;
                                          index++)
                                        tile(
                                            actions[index],
                                            pair(action(actions[index], 32),
                                                action(actions[index], 24)),
                                            index)
                                    ]),
                                    const SizedBox(width: 20),
                                    column('Controls', [
                                      for (var index = 0;
                                          index < controls.length;
                                          index++)
                                        tile(
                                            controls.keys.elementAt(index),
                                            pair(
                                                Icon(
                                                    controls.values
                                                        .elementAt(index),
                                                    size: 24,
                                                    color: AppStyle.primary),
                                                Icon(
                                                    controls.values
                                                        .elementAt(index),
                                                    size: 18,
                                                    color: AppStyle.primary)),
                                            index)
                                    ]),
                                  ])),
                              const Text(
                                  'Bundled vector glyphs · no file reads, image decode or per-file bitmap cache',
                                  style: AppStyle.caption),
                            ])))))));
    await tester.pumpAndSettle();
    expect(tester.takeException(), isNull);
    await tester.runAsync(() async {
      final boundary = tester.renderObject<RenderRepaintBoundary>(
          find.byKey(const Key('icons-gallery')));
      final bitmap = await boundary.toImage(pixelRatio: 2);
      final data = await bitmap.toByteData(format: ui.ImageByteFormat.png);
      final file = File('../../build/icons-build8/icon-vocabulary.png');
      await file.parent.create(recursive: true);
      await file.writeAsBytes(data!.buffer.asUint8List());
      bitmap.dispose();
    });
  });
}
