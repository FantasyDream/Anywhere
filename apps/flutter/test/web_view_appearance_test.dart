import 'dart:async';

import 'package:anywhere/infrastructure/desktop_appearance.dart';
import 'package:anywhere/web_plugins/web_document_theme.dart';
import 'package:anywhere/web_plugins/web_locale.dart';
import 'package:anywhere/web_plugins/web_view_appearance.dart';
import 'package:anywhere/web_plugins/web_view_events.dart';
import 'package:flutter/services.dart';
import 'package:flutter_inappwebview/flutter_inappwebview.dart';
import 'package:flutter_test/flutter_test.dart';

class RecordingWebController extends PlatformInAppWebViewController {
  RecordingWebController()
      : super.implementation(
            const PlatformInAppWebViewControllerCreationParams(id: 1));
  final scripts = <String>[];
  bool disposed = false;
  Completer<void>? evaluation;
  Future<void>? Function(String source)? gate;
  @override
  Future<dynamic> evaluateJavascript(
      {required String source, ContentWorld? contentWorld}) async {
    if (disposed) throw PlatformException(code: 'disposed');
    scripts.add(source);
    final gated = gate?.call(source);
    if (gated != null) await gated;
    await evaluation?.future;
    return null;
  }
}

class FakeAppearanceHost implements DesktopAppearanceHost {
  FakeAppearanceHost(this.value);

  DesktopAppearanceSnapshot value;
  Future<void> Function(DesktopAppearanceSnapshot)? changed;

  @override
  void listen(Future<void> Function(DesktopAppearanceSnapshot) changed) {
    this.changed = changed;
  }

  @override
  Future<DesktopAppearanceSnapshot?> refresh(
      DesktopAppearanceSnapshot current) async {
    return value;
  }

  @override
  Future<DesktopAppearanceSnapshot?> watch() async => value;

  Future<void> emit(DesktopAppearanceSnapshot snapshot) async {
    value = snapshot;
    await changed?.call(snapshot);
  }

  @override
  void dispose() {}
}

DesktopAppearanceSnapshot modelSnapshot({
  String source = 'source-1',
  String token = 'window-1',
  int revision = 1,
  AppearanceBrightness brightness = AppearanceBrightness.dark,
  AppearanceContrast contrast = AppearanceContrast.standard,
  bool reduceTransparency = false,
  bool reduceMotion = false,
  WindowSurfaceMode surface = WindowSurfaceMode.systemManaged,
}) =>
    DesktopAppearanceSnapshot(
      sourceId: source,
      windowToken: token,
      revision: revision,
      brightness: brightness,
      contrast: contrast,
      reduceTransparency: reduceTransparency,
      reduceMotion: reduceMotion,
      surface: surface,
      capabilities: const DesktopAppearanceCapabilities(
          protocolVersion: 1,
          systemSurface: true,
          brightness: true,
          contrast: true,
          reduceTransparency: true,
          reduceMotion: true),
    );

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  const channel = MethodChannel('test/web-appearance');
  const appearance = WebViewAppearance(native: channel);
  final messenger =
      TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger;
  late RecordingWebController platform;
  late InAppWebViewController web;
  setUp(() {
    platform = RecordingWebController();
    web = InAppWebViewController.fromPlatform(platform: platform);
  });
  tearDown(() => messenger.setMockMethodCallHandler(channel, null));

  testWidgets(
      'unresponsive native appearance falls back without stranding readiness',
      (tester) async {
    final pending = Completer<bool>();
    messenger.setMockMethodCallHandler(channel, (_) => pending.future);
    var completed = false;
    final request = appearance
        .apply(web, isCurrent: () => true)
        .then((_) => completed = true);
    try {
      await tester.pump();
      await tester.pump(const Duration(seconds: 3));
      expect(completed, isTrue);
      expect(platform.scripts, [webCanvasScript(transparent: false)]);
    } finally {
      pending.complete(true);
      await request;
    }
    expect(platform.scripts, [webCanvasScript(transparent: false)]);
  });

  testWidgets(
      'unresponsive script evaluation cannot hold Host presentation forever',
      (tester) async {
    messenger.setMockMethodCallHandler(channel, (_) async => true);
    final pending = platform.evaluation = Completer<void>();
    var completed = false;
    final request = appearance
        .apply(web, isCurrent: () => true)
        .then((_) => completed = true);
    try {
      await tester.pump();
      await tester.pump(const Duration(seconds: 3));
      expect(completed, isTrue);
    } finally {
      pending.complete();
      await request;
    }
  });

  testWidgets('language notification cannot hold initial display',
      (tester) async {
    final pending = platform.evaluation = Completer<void>();
    var completed = false;
    final sent = notifyWebLanguage(web, 'zh').then((_) => completed = true);
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 300));
    expect(completed, isTrue);
    pending.complete();
    await sent;
  });

  test('locale notification is bounded public metadata and tolerates teardown',
      () async {
    await notifyWebLanguage(web, 'zh');
    expect(platform.scripts.single, contains("__anywhereLanguage = 'zh'"));
    expect(platform.scripts.single,
        contains("document.documentElement.lang = 'zh'"));
    expect(
        webLocaleScript("'</script>"), contains("__anywhereLanguage = 'en'"));
    expect(webLocaleScript("'</script>"), isNot(contains('</script>')));
    platform.disposed = true;
    await notifyWebLanguage(web, 'en');
  });

  test('canvas script carries complete light contrast and motion state', () {
    const appearance = DesktopAppearanceSnapshot(
      sourceId: 'source',
      windowToken: 'window',
      revision: 7,
      brightness: AppearanceBrightness.light,
      contrast: AppearanceContrast.increased,
      reduceTransparency: false,
      reduceMotion: true,
      surface: WindowSurfaceMode.systemManaged,
      capabilities: DesktopAppearanceCapabilities(
          protocolVersion: 1, systemSurface: true),
    );
    final script = webCanvasScript(transparent: true, appearance: appearance);
    expect(script, contains("root.dataset.awAppearance = 'light'"));
    expect(script, contains("root.dataset.awContrast = 'increased'"));
    expect(script, contains("root.dataset.awReduceMotion = 'true'"));
    expect(script, contains("root.style.setProperty('color-scheme', 'light'"));
    expect(script, contains("'transparent'"));
  });

  test('presentation events tolerate teardown and carry no plugin capability',
      () async {
    await notifyWebPresentation(web, WebPresentationEvent.visibility, false);
    expect(platform.scripts.single, contains('anywhere:visibility'));
    expect(platform.scripts.single, contains('detail:false'));
    platform.disposed = true;
    await notifyWebPresentation(web, WebPresentationEvent.panel, false);
    await notifyWebPresentation(null, WebPresentationEvent.visibility, true);
  });

  test('every plugin receives Host canvas setup without any plugin API call',
      () async {
    messenger.setMockMethodCallHandler(channel, (call) async {
      expect(call.method, 'configureWebAppearance');
      expect(call.arguments, isNull);
      return true;
    });
    final loaded = InAppWebViewController.fromPlatform(platform: platform);
    await appearance.apply(loaded, isCurrent: () => sharesWebView(web, loaded));
    expect(platform.scripts, [webCanvasScript(transparent: false)]);
  });

  for (final result in [false, null, 'unsupported', 'failed']) {
    test('unavailable native canvas uses the shared fallback: $result',
        () async {
      messenger.setMockMethodCallHandler(channel, (_) async {
        if (result == 'unsupported') throw MissingPluginException();
        if (result == 'failed') throw PlatformException(code: 'unavailable');
        return result;
      });
      await appearance.apply(web, isCurrent: () => true);
      expect(platform.scripts, [webCanvasScript(transparent: false)]);
    });
  }

  test('hide invalidates an in-flight native result before document mutation',
      () async {
    final pending = Completer<bool>();
    messenger.setMockMethodCallHandler(channel, (_) => pending.future);
    var visible = true;
    final request = appearance.apply(web, isCurrent: () => visible);
    visible = false;
    pending.complete(true);
    await request;
    expect(platform.scripts, isEmpty);
  });

  test('old response cannot overwrite a resumed document appearance', () async {
    final pending = Completer<bool>();
    var calls = 0;
    messenger.setMockMethodCallHandler(
        channel, (_) => ++calls == 1 ? pending.future : Future.value(false));
    var epoch = 1;
    final old = appearance.apply(web, isCurrent: () => epoch == 1);
    await Future<void>.delayed(Duration.zero);
    epoch = 2;
    await appearance.apply(web, isCurrent: () => epoch == 2);
    pending.complete(true);
    await old;
    expect(platform.scripts, [webCanvasScript(transparent: false)]);
  });

  test('late JS completion restores the latest native appearance version',
      () async {
    messenger.setMockMethodCallHandler(channel, (_) async => true);
    final host = FakeAppearanceHost(
        modelSnapshot(revision: 1, brightness: AppearanceBrightness.light));
    final controller = DesktopAppearanceController(host: host);
    addTearDown(controller.dispose);
    await controller.bind();
    final versioned =
        WebViewAppearance(native: channel, controller: controller);
    final first = Completer<void>();
    final second = Completer<void>();
    var evaluations = 0;
    platform.gate = (_) => switch (++evaluations) {
          1 => first.future,
          2 => second.future,
          _ => null,
        };

    final old = versioned.apply(web, isCurrent: () => true);
    await Future<void>.delayed(Duration.zero);
    await host.emit(
        modelSnapshot(revision: 2, brightness: AppearanceBrightness.dark));
    final current = versioned.apply(web, isCurrent: () => true);
    await Future<void>.delayed(Duration.zero);
    second.complete();
    await current;
    first.complete();
    await old;

    expect(
        platform.scripts.last, contains("root.dataset.awAppearance = 'dark'"));
  });

  test('stale view does not invoke native configuration', () async {
    var calls = 0;
    messenger.setMockMethodCallHandler(channel, (_) async {
      calls++;
      return true;
    });
    await appearance.apply(web, isCurrent: () => false);
    expect(calls, 0);
    expect(platform.scripts, isEmpty);
  });

  test('departing platform view is safe to dispose during application',
      () async {
    messenger.setMockMethodCallHandler(channel, (_) async => true);
    platform.disposed = true;
    await appearance.apply(web, isCurrent: () => true);
    expect(platform.scripts, isEmpty);
  });
}
