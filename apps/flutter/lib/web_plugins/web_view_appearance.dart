import 'dart:async';
import 'package:flutter/services.dart';
import 'package:flutter_inappwebview/flutter_inappwebview.dart';
import 'web_document_theme.dart';

/// SDK callbacks can wrap the same native view in different Dart facades.
bool sharesWebView(
        InAppWebViewController? current, InAppWebViewController callback) =>
    current != null && identical(current.platform, callback.platform);

/// Automatic Host presentation for every Web tool. No plugin RPC, preference,
/// JSON-specific behavior, polling, or DOM-wide observers are involved.
class WebViewAppearance {
  const WebViewAppearance(
      {this.native = const MethodChannel('anywhere/web-tools')});
  final MethodChannel native;

  Future<void> apply(InAppWebViewController controller,
      {required bool Function() isCurrent}) async {
    if (!isCurrent()) return;
    var transparent = false;
    try {
      transparent = await native
              .invokeMethod<bool>('configureWebAppearance')
              .timeout(const Duration(seconds: 2)) ==
          true;
    } on MissingPluginException {
      // Platforms without a native adapter keep the shared readable fallback.
    } on PlatformException {
      // A platform failure must not leave a default white canvas under white text.
    } on TimeoutException {
      // A missing native reply must not strand readiness or the loading indicator.
    }
    if (!isCurrent()) return;
    try {
      await controller
          .evaluateJavascript(source: webCanvasScript(transparent: transparent))
          .timeout(const Duration(seconds: 2));
    } on MissingPluginException {
      // A departing native view may already have been disposed.
    } on PlatformException {
      // The injected Host stylesheet still supplies the readable fallback.
    } on TimeoutException {
      // Keep the bounded Host presentation flow alive if WebKit stops replying.
    }
  }
}
