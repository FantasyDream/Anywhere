import 'dart:convert';
import 'dart:typed_data';

import 'package:anywhere/ui/design_tokens.g.dart';
import 'package:anywhere/web_plugins/web_bootstrap.dart';
import 'package:anywhere/web_plugins/web_document_policy.dart';
import 'package:anywhere/web_plugins/web_document_theme.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  final origin = 'anywhere-plugin://${'a' * 64}.plugin.local';
  final policy = WebDocumentPolicy(origin, 'dist/index.html');
  test('protected HTML is ready for native bytes and preserves UTF-8 content',
      () {
    const markup = '<body><p>中文 😀 café</p></body>';
    final Uint8List bytes = policy.protectHtml(utf8.encode(markup));
    expect(utf8.decode(bytes), endsWith('$markup</html>'));
    expect(() => policy.protectHtml([0xff]), throwsFormatException);
  });
  test(
      'resource requests require exact native origin and verified relative path',
      () {
    expect(policy.resource('$origin/dist/main.js'), 'dist/main.js');
    for (final url in [
      'https://example.com/main.js',
      '$origin.evil/dist/main.js',
      '$origin/dist/main.js?x=1',
      '$origin/dist/main.js#x',
      '$origin/dist/%2e%2e/secret',
      '$origin/dist/%5csecret',
      '$origin:8/dist/main.js'
    ]) {
      expect(policy.resource(url), isNull, reason: url);
    }
    expect(policy.contentType('other.html'), isNull);
    expect(policy.contentType('manifest.json'), 'application/json');
    expect(policy.contentType('dist/main.js'), 'text/javascript');
  });
  test(
      'Host policy precedes hostile markup and denies network/frame/worker/form',
      () {
    final html = utf8.decode(policy.protectHtml(
        utf8.encode('<script src="https://attacker.test/a.js"></script>')));
    expect(html.indexOf('Content-Security-Policy'),
        lessThan(html.indexOf('<script')));
    for (final rule in [
      "connect-src 'none'",
      "frame-src 'none'",
      "worker-src 'none'",
      "form-action 'none'",
      "base-uri 'none'"
    ]) {
      expect(html, contains(rule));
    }
    expect(html, isNot(contains("script-src 'unsafe-inline'")));
  });
  test('Host origin/entry cannot inject document policy', () {
    for (final origin in [
      'https://example.com',
      'anywhere-plugin://bad.plugin.local',
      '${policy.origin}/x'
    ]) {
      expect(() => WebDocumentPolicy(origin, 'dist/index.html'),
          throwsArgumentError);
    }
    expect(() => WebDocumentPolicy(policy.origin, '../index.html'),
        throwsArgumentError);
  });
  test('shared readable theme is available before plugin styles and scripts',
      () {
    const markup =
        '<link rel="stylesheet" href="style.css"><script src="main.js"></script>';
    final html = utf8.decode(policy.protectHtml(utf8.encode(markup)));
    expect(html, contains(webDesignTokensCss));
    expect(html, contains(webDocumentThemeCss));
    expect(html.indexOf('Content-Security-Policy'),
        lessThan(html.indexOf('data-anywhere-theme')));
    expect(html.indexOf('data-anywhere-theme'), lessThan(html.indexOf(markup)));
    expect(
        webDesignTokensCss,
        contains(
            'background: var(--aw-web-canvas, var(--aw-fallback-surface))'));
    expect(webDesignTokensCss, isNot(contains('color-scheme:')));
    expect(webBootstrap, isNot(contains('createElement(\'style\')')));
  });
  test('canvas defaults are injected for plain HTML and common framework roots',
      () {
    for (final markup in [
      '<body><p>No SDK or CSS</p></body>',
      '<html style="background:white!important"><body style="background:white!important"><div id="app"></div></body></html>',
      '<style>:root{color-scheme:dark;background:#fff}#root{background:red}</style><div id="root"></div>',
      '<div id="__next"><article class="panel">Custom panel</article></div>',
    ]) {
      final html = utf8.decode(policy.protectHtml(utf8.encode(markup)));
      expect(html, contains(webDocumentThemeCss));
      expect(
          html.indexOf('data-anywhere-theme'), lessThan(html.indexOf(markup)));
    }
    expect(webDocumentThemeCss, contains('!important'));
    final script = webCanvasScript(transparent: true);
    expect(script, contains('document.body'));
    expect(script, contains('#app, #root, #__next'));
    expect(script, isNot(contains('MutationObserver')));
    expect(script, isNot(contains('querySelectorAll(\'*\')')));
  });
  test('plain Web controls receive shared metrics without importing SDK CSS',
      () {
    final html = utf8.decode(policy.protectHtml(utf8.encode(
        '<body><button>Run</button><input><select><option>Two</option></select></body>')));
    expect(html, contains(webControlsCss));
    expect(html.indexOf(webControlsCss), lessThan(html.indexOf('<button>')));
    expect(webControlsCss, contains('var(--aw-control-height)'));
    expect(webControlsCss, contains('var(--aw-control-weight)'));
    expect(webControlsCss, contains('var(--aw-field-font-size)'));
    expect(webControlsCss, contains('var(--aw-keycap-radius)'));
    expect(webControlsCss, contains('appearance: none'));
    expect(webControlsCss, isNot(contains('color-scheme: dark !important')));
  });
}
