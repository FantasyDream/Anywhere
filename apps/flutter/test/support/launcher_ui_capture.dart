import 'dart:io';
import 'dart:ui' as ui;
import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

// Optional offscreen design review. This does not launch the app or imitate
// native glass; the neutral backdrop makes typography and spacing inspectable.
const captureLauncherUi = bool.fromEnvironment('ANYWHERE_CAPTURE_UI');
const _captureKey = Key('launcher-ui-capture');

Future<void> loadCaptureFonts() async {
  if (!captureLauncherUi) return;
  final font = File('/System/Library/Fonts/SFNS.ttf');
  if (await font.exists()) {
    final loader = FontLoader('.AppleSystemUIFont');
    loader.addFont(
        font.readAsBytes().then((bytes) => ByteData.sublistView(bytes)));
    await loader.load();
  }
  final monospace = File('/System/Library/Fonts/Menlo.ttc');
  if (await monospace.exists()) {
    final bytes = await monospace.readAsBytes();
    for (final family in ['monospace', 'Menlo']) {
      final loader = FontLoader(family);
      loader.addFont(Future.value(ByteData.sublistView(bytes)));
      await loader.load();
    }
  }
  final icons = FontLoader('packages/cupertino_icons/CupertinoIcons');
  icons.addFont(
      rootBundle.load('packages/cupertino_icons/assets/CupertinoIcons.ttf'));
  await icons.load();
}

Widget captureRoot(Widget child) => !captureLauncherUi
    ? child
    : RepaintBoundary(
        key: _captureKey,
        child: ColoredBox(color: const Color(0xff343740), child: child));

Future<void> saveLauncherUi(WidgetTester tester, String name) async {
  if (!captureLauncherUi) return;
  await tester.runAsync(() async {
    final root = find.byKey(_captureKey);
    final boundary = tester.renderObject<RenderRepaintBoundary>(root);
    final image = await boundary.toImage(pixelRatio: 2);
    final panel = tester
        .getRect(find.byKey(const Key('launcher-panel')))
        .shift(-tester.getTopLeft(root));
    final recorder = ui.PictureRecorder();
    final canvas = Canvas(recorder);
    canvas.drawImageRect(
        image,
        Rect.fromLTWH(
            panel.left * 2, panel.top * 2, panel.width * 2, panel.height * 2),
        Rect.fromLTWH(0, 0, panel.width * 2, panel.height * 2),
        Paint());
    final picture = recorder.endRecording();
    final cropped = await picture.toImage(
        (panel.width * 2).ceil(), (panel.height * 2).ceil());
    final data = await cropped.toByteData(format: ui.ImageByteFormat.png);
    final directory = Directory('../../build/ui-v0.7-build13');
    await directory.create(recursive: true);
    await File('${directory.path}/$name.png')
        .writeAsBytes(data!.buffer.asUint8List());
    cropped.dispose();
    picture.dispose();
    image.dispose();
  });
}
