import 'dart:convert';
import 'package:flutter_test/flutter_test.dart';
import 'package:anywhere_web_plugin_probe/probe_bridge.dart';

void main() {
  test('one document per binding; navigation and reload cannot reuse it', () {
    final bridge = ProbeBridge(1);
    final target = Uri.parse('anywhere-plugin://probe.local/index.html');
    expect(bridge.permitNavigation(target, isMainFrame: false), isFalse);
    expect(
        bridge.permitNavigation(Uri.parse('https://example.com'),
            isMainFrame: true),
        isFalse);
    expect(bridge.permitNavigation(target, isMainFrame: true), isTrue);
    expect(bridge.permitNavigation(target, isMainFrame: true), isFalse);
    expect(ProbeBridge(2).permitNavigation(target, isMainFrame: true), isTrue);
    expect(
        (ProbeBridge(3)..dispose()).permitNavigation(target, isMainFrame: true),
        isFalse);
  });
  String request(String id,
          {String method = 'probe.echo', String text = '你好 😀'}) =>
      jsonEncode({
        'protocol': 1,
        'requestId': id,
        'method': method,
        'params': {'text': text}
      });
  Map<String, Object?> call(ProbeBridge bridge, String value,
          {Uri? origin, bool mainFrame = true}) =>
      bridge.handle(value,
          sourceOrigin: origin ?? ProbeBridge.origin, isMainFrame: mainFrame);
  String? code(Map<String, Object?> value) =>
      (value['error'] as Map<String, Object?>?)?['code'] as String?;

  test('round trips Unicode and syntax without evaluating script', () {
    final bridge = ProbeBridge(7);
    const text = '''你好 😀 "</script>\\\n\u0000''';
    expect(call(bridge, request('a', text: text))['result'],
        {'text': text, 'epoch': 7});
  });
  test('rejects subframes and foreign or misleading origins', () {
    final bridge = ProbeBridge(1);
    expect(code(call(bridge, request('a'), mainFrame: false)),
        'UNTRUSTED_DOCUMENT');
    for (final url in [
      'https://probe.local',
      'anywhere-plugin://other.local',
      'anywhere-plugin://probe.local.evil',
      'anywhere-plugin://user@probe.local',
      'anywhere-plugin://probe.local:1',
      'anywhere-plugin://probe.local/path',
      'anywhere-plugin://probe.local?x=1',
      'anywhere-plugin://probe.local#x'
    ]) {
      expect(code(call(bridge, request('a'), origin: Uri.parse(url))),
          'UNTRUSTED_DOCUMENT',
          reason: url);
    }
    expect(
        code(
            bridge.handle(request('a'), sourceOrigin: null, isMainFrame: true)),
        'UNTRUSTED_DOCUMENT');
  });
  test('rejects malformed schema and self-reported authority', () {
    final bridge = ProbeBridge(1);
    for (final raw in [
      'null',
      '[]',
      '{',
      '{}',
      request('a').replaceFirst('"protocol":1', '"protocol":2'),
      request('a').replaceFirst('"params":', '"pluginId":"trusted","params":'),
      request('../escape')
    ]) {
      expect(code(call(bridge, raw)), 'INVALID_REQUEST');
    }
  });
  test('cannot call clipboard, context, process, or FRB methods', () {
    final bridge = ProbeBridge(1);
    for (final method in [
      'clipboard.readText',
      'context.getCurrentApp',
      'shell.exec',
      'initialize',
      'invoke'
    ]) {
      expect(
          code(call(
              bridge, request(method.replaceAll('.', '_'), method: method))),
          'METHOD_NOT_ALLOWED');
    }
  });
  test('counts UTF8 bytes and bounds replay tracking', () {
    final bridge = ProbeBridge(1);
    expect(code(call(bridge, request('large', text: '汉' * 23000))),
        'FRAME_TOO_LARGE');
    expect(code(call(bridge, request('large', text: '汉' * 1400))),
        'INVALID_PARAMS');
    expect(code(call(bridge, request('large'))), 'DUPLICATE_REQUEST');
    for (var i = 0; i < 127; i++) {
      expect(code(call(bridge, request('id$i'))), isNull);
    }
    expect(code(call(bridge, request('last'))), 'QUOTA_EXCEEDED');
  });
  test('hidden, disposed and replaced documents cannot use old binding', () {
    final old = ProbeBridge(1)..interactive = false;
    expect(code(call(old, request('a'))), 'INACTIVE');
    old.interactive = true;
    expect(code(call(old, request('a'))), isNull);
    old.dispose();
    expect(code(call(old, request('b'))), 'INACTIVE');
    expect(call(ProbeBridge(2), request('a'))['result'],
        {'text': '你好 😀', 'epoch': 2});
  });
}
