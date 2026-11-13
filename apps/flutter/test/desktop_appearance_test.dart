import 'dart:async';

import 'package:anywhere/infrastructure/desktop_appearance.dart';
import 'package:anywhere/l10n/app_language.dart';
import 'package:anywhere/ui/action_icon_appearance.dart';
import 'package:anywhere/ui/app_style.dart';
import 'package:anywhere/ui/file_type_appearance.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:material_ui/material_ui.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  const channel = MethodChannel('test/appearance');
  final messenger =
      TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger;

  Map<String, Object?> snapshot({
    String source = 'source-1',
    String token = 'window-1',
    int revision = 1,
    String brightness = 'dark',
    String contrast = 'standard',
    bool reduceTransparency = false,
    bool reduceMotion = false,
    String surface = 'systemManaged',
    int? accentColor,
    Map<String, int> systemColors = const {},
  }) =>
      {
        'sourceId': source,
        'windowToken': token,
        'revision': revision,
        'brightness': brightness,
        'contrast': contrast,
        'reduceTransparency': reduceTransparency,
        'reduceMotion': reduceMotion,
        'surface': surface,
        if (accentColor != null) 'accentColor': accentColor,
        'systemColors': systemColors,
        'capabilities': {
          'protocolVersion': 1,
          'systemSurface': true,
          'brightness': true,
          'contrast': true,
          'reduceTransparency': true,
          'reduceMotion': true,
          'systemColors': false,
        },
      };

  tearDown(() => messenger.setMockMethodCallHandler(channel, null));

  test('missing Host falls back to a readable opaque appearance', () async {
    messenger.setMockMethodCallHandler(channel, (_) async {
      throw MissingPluginException();
    });
    final controller = DesktopAppearanceController(
        host: MethodChannelDesktopAppearanceHost(channel));
    addTearDown(controller.dispose);

    await controller.bind();

    expect(controller.binding, isFalse);
    expect(controller.snapshot.sourceId, 'local-fallback');
    expect(controller.snapshot.surface, WindowSurfaceMode.opaque);
    expect(controller.snapshot.brightness, AppearanceBrightness.dark);
  });

  test('event received before the initial reply wins the same binding',
      () async {
    final initial = Completer<Object?>();
    messenger.setMockMethodCallHandler(channel, (call) {
      if (call.method == 'watchAppearance') return initial.future;
      return Future<Object?>.value(null);
    });
    final controller = DesktopAppearanceController(
        host: MethodChannelDesktopAppearanceHost(channel));
    addTearDown(controller.dispose);

    final binding = controller.bind();
    await Future<void>.delayed(Duration.zero);
    await messenger.handlePlatformMessage(
        channel.name,
        const StandardMethodCodec().encodeMethodCall(
            MethodCall('appearanceChanged', snapshot(revision: 2))),
        (_) {});
    initial.complete(snapshot(revision: 1));
    await binding;

    expect(controller.snapshot.revision, 2);
  });

  test('duplicate and older revisions cannot revert appearance', () async {
    messenger.setMockMethodCallHandler(
        channel, (_) async => snapshot(revision: 4, brightness: 'light'));
    final controller = DesktopAppearanceController(
        host: MethodChannelDesktopAppearanceHost(channel));
    addTearDown(controller.dispose);
    await controller.bind();
    var notifications = 0;
    controller.addListener(() => ++notifications);

    for (final revision in [4, 3]) {
      await messenger.handlePlatformMessage(
          channel.name,
          const StandardMethodCodec().encodeMethodCall(
              MethodCall('appearanceChanged', snapshot(revision: revision))),
          (_) {});
    }

    expect(controller.snapshot.revision, 4);
    expect(controller.snapshot.brightness, AppearanceBrightness.light);
    expect(notifications, 0);
  });

  test('new source is accepted while its late predecessor is retired',
      () async {
    messenger.setMockMethodCallHandler(channel, (_) async => snapshot());
    final controller = DesktopAppearanceController(
        host: MethodChannelDesktopAppearanceHost(channel));
    addTearDown(controller.dispose);
    await controller.bind();

    await messenger.handlePlatformMessage(
        channel.name,
        const StandardMethodCodec().encodeMethodCall(MethodCall(
            'appearanceChanged',
            snapshot(
                source: 'source-2',
                token: 'window-2',
                revision: 1,
                brightness: 'light'))),
        (_) {});
    await messenger.handlePlatformMessage(
        channel.name,
        const StandardMethodCodec().encodeMethodCall(MethodCall(
            'appearanceChanged',
            snapshot(source: 'source-1', token: 'window-1', revision: 9))),
        (_) {});

    expect(controller.snapshot.sourceId, 'source-2');
    expect(controller.snapshot.windowToken, 'window-2');
    expect(controller.snapshot.brightness, AppearanceBrightness.light);
  });

  test('malformed native data cannot replace the last safe snapshot', () async {
    messenger.setMockMethodCallHandler(channel, (_) async => snapshot());
    final controller = DesktopAppearanceController(
        host: MethodChannelDesktopAppearanceHost(channel));
    addTearDown(controller.dispose);
    await controller.bind();
    final before = controller.snapshot;

    await messenger.handlePlatformMessage(
        channel.name,
        const StandardMethodCodec().encodeMethodCall(MethodCall(
            'appearanceChanged', snapshot(revision: 2, brightness: 'sepia'))),
        (_) {});

    expect(controller.snapshot, same(before));
  });

  test('rebinding ignores the prior binding generation', () async {
    var watches = 0;
    final first = Completer<Object?>();
    messenger.setMockMethodCallHandler(channel, (call) {
      if (call.method != 'watchAppearance') return Future<Object?>.value(null);
      return ++watches == 1
          ? first.future
          : Future<Object?>.value(
              snapshot(source: 'source-2', token: 'window-2', revision: 1));
    });
    final controller = DesktopAppearanceController(
        host: MethodChannelDesktopAppearanceHost(channel));
    addTearDown(controller.dispose);

    final old = controller.bind();
    final newer = controller.bind();
    first.complete(snapshot());
    await Future.wait([old, newer]);

    expect(controller.snapshot.sourceId, 'source-2');
  });

  testWidgets('shared root rebuilds Flutter theme on native appearance events',
      (tester) async {
    messenger.setMockMethodCallHandler(
        channel,
        (_) async => snapshot(
                brightness: 'light',
                contrast: 'increased',
                reduceTransparency: true,
                surface: 'opaque',
                accentColor: 0xffffc000,
                systemColors: const {
                  'keyboardFocusIndicator': 0xff0084ff,
                }));
    final controller = DesktopAppearanceController(
        host: MethodChannelDesktopAppearanceHost(channel));
    DesktopAppearance.configure(controller);
    addTearDown(DesktopAppearance.reset);
    await controller.bind();
    final stateKey = GlobalKey<_AppearanceSurfaceState>();

    await tester.pumpWidget(LocalizedApp(
        themeBuilder: () => AppStyle.theme,
        homeBuilder: () =>
            _AppearanceSurface(key: stateKey, marker: 'preserved')));
    await tester.pumpAndSettle();

    expect(stateKey.currentState!.marker, 'preserved');
    expect(
        Theme.of(tester.element(find.byKey(const Key('appearance-surface'))))
            .brightness,
        Brightness.light);
    expect(
        tester
            .widget<ColoredBox>(find.byKey(const Key('appearance-surface')))
            .color,
        DesignTokens.lightContrast.surface);
    expect(ActionIconAppearance.forKey('file-search').tint,
        DesignTokens.lightContrast.iconBlue);
    expect(FileTypeAppearance.pdf.tint, DesignTokens.lightContrast.iconCoral);
    expect(AppStyle.accent, const Color(0xffffc000));
    expect(AppStyle.onAccent, Colors.black);
    expect(AppStyle.selectedBorder, const Color(0xff0084ff));
    await messenger.handlePlatformMessage(
        channel.name,
        const StandardMethodCodec().encodeMethodCall(MethodCall(
            'appearanceChanged',
            snapshot(revision: 2, brightness: 'dark', contrast: 'standard'))),
        (_) {});
    await tester.pumpAndSettle();

    expect(
        Theme.of(tester.element(find.byKey(const Key('appearance-surface'))))
            .brightness,
        Brightness.dark);
    expect(
        tester
            .widget<ColoredBox>(find.byKey(const Key('appearance-surface')))
            .color,
        DesignTokens.dark.surface);
    expect(ActionIconAppearance.forKey('file-search').tint,
        DesignTokens.dark.iconBlue);
    expect(FileTypeAppearance.pdf.tint, DesignTokens.dark.iconCoral);
    expect(AppStyle.motionDuration(const Duration(milliseconds: 120)),
        const Duration(milliseconds: 120));
    expect(stateKey.currentState!.marker, 'preserved');

    await messenger.handlePlatformMessage(
        channel.name,
        const StandardMethodCodec().encodeMethodCall(MethodCall(
            'appearanceChanged',
            snapshot(
                revision: 3,
                brightness: 'dark',
                contrast: 'standard',
                reduceMotion: true))),
        (_) {});
    await tester.pumpAndSettle();

    expect(
        MediaQuery.of(
                tester.element(find.byKey(const Key('appearance-surface'))))
            .disableAnimations,
        isTrue);
    expect(AppStyle.motionDuration(const Duration(milliseconds: 120)),
        Duration.zero);
    expect(stateKey.currentState!.marker, 'preserved');
  });
}

class _AppearanceSurface extends StatefulWidget {
  const _AppearanceSurface({super.key, required this.marker});

  final String marker;

  @override
  State<_AppearanceSurface> createState() => _AppearanceSurfaceState();
}

class _AppearanceSurfaceState extends State<_AppearanceSurface> {
  String get marker => widget.marker;

  @override
  Widget build(BuildContext context) => ColoredBox(
        key: const Key('appearance-surface'),
        color: AppStyle.surface,
      );
}
