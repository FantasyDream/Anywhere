import 'dart:async';

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
  @override
  Future<dynamic> evaluateJavascript(
      {required String source, ContentWorld? contentWorld}) async {
    if (disposed) throw PlatformException(code: 'disposed');
    scripts.add(source);
    await evaluation?.future;
    return null;
  }
}

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
    expect(platform.scripts, [webCanvasScript(transparent: true)]);
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
