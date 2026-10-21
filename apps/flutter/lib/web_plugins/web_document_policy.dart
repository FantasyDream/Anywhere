import 'dart:convert';
import 'dart:typed_data';
import 'web_document_theme.dart';

/// Host document/resource policy and CSP. Rust independently checks identity,
/// package approval and asset digests; this adapter never grants capabilities.
class WebDocumentPolicy {
  WebDocumentPolicy(this.origin, this.entry) {
    final uri = Uri.parse(origin);
    if (uri.scheme != 'anywhere-plugin' ||
        !RegExp(r'^[a-f0-9]{64}\.plugin\.local$').hasMatch(uri.host) ||
        uri.path.isNotEmpty ||
        uri.hasPort ||
        uri.hasQuery ||
        uri.hasFragment ||
        uri.userInfo.isNotEmpty ||
        entry.startsWith('/') ||
        entry.split('/').any((p) => p.isEmpty || p == '.' || p == '..')) {
      throw ArgumentError('Invalid Web document');
    }
  }
  final String origin, entry;
  String get url => '$origin/$entry';
  String? resource(String value) {
    if (RegExp(r'%2e|%2f|%5c', caseSensitive: false).hasMatch(value)) {
      return null;
    }
    final uri = Uri.tryParse(value);
    final base = Uri.parse(origin);
    if (uri == null ||
        uri.scheme != base.scheme ||
        uri.host != base.host ||
        uri.hasPort ||
        uri.userInfo.isNotEmpty ||
        uri.hasQuery ||
        uri.hasFragment ||
        !uri.path.startsWith('/')) {
      return null;
    }
    final path = uri.path.substring(1);
    if (path.split('/').any((p) => p.isEmpty || p == '.' || p == '..') ||
        path.contains('\\') ||
        path.contains('\u0000')) {
      return null;
    }
    return path;
  }

  String? contentType(String path) {
    if (path.endsWith('.html')) return path == entry ? 'text/html' : null;
    return switch (path.split('.').last.toLowerCase()) {
      'js' || 'mjs' => 'text/javascript',
      'css' => 'text/css',
      'json' => 'application/json',
      'png' => 'image/png',
      'jpg' || 'jpeg' => 'image/jpeg',
      'webp' => 'image/webp',
      'gif' => 'image/gif',
      'svg' => 'image/svg+xml',
      'woff2' => 'font/woff2',
      'woff' => 'font/woff',
      'txt' => 'text/plain',
      _ => null,
    };
  }

  /// Host CSP is first in the parser, regardless of any policy declared by a package.
  Uint8List protectHtml(List<int> bytes) => utf8.encode(
      '<!doctype html><html><head><meta charset="utf-8">'
      '<meta http-equiv="Content-Security-Policy" content="'
      "default-src 'none'; script-src $origin; style-src $origin 'unsafe-inline'; "
      "img-src $origin data:; font-src $origin; media-src 'none'; "
      "connect-src 'none'; frame-src 'none'; child-src 'none'; worker-src 'none'; "
      "object-src 'none'; base-uri 'none'; form-action 'none'; "
      "script-src-attr 'none'; upgrade-insecure-requests\">"
      '<style data-anywhere-theme>$webDocumentThemeCss</style>'
      '</head>${utf8.decode(bytes)}</html>');
}
