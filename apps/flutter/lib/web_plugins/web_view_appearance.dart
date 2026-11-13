import 'dart:async';
import 'package:flutter/services.dart';
import 'package:flutter_inappwebview/flutter_inappwebview.dart';

import '../infrastructure/desktop_appearance.dart';
import 'web_document_theme.dart';

/// SDK callbacks can wrap the same native view in different Dart facades.
bool sharesWebView(
        InAppWebViewController? current, InAppWebViewController callback) =>
    current != null && identical(current.platform, callback.platform);

/// Automatic Host presentation for every Web tool. No plugin RPC, preference,
/// JSON-specific behavior, polling, or DOM-wide observers are involved.
class WebViewAppearance {
  const WebViewAppearance(
      {this.native = const MethodChannel('anywhere/web-tools'),
      this.controller});
  final MethodChannel native;
  final DesktopAppearanceController? controller;

  DesktopAppearanceSnapshot get _snapshot =>
      (controller ?? DesktopAppearance.current).snapshot;

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
    // A native or operating-system event can arrive while evaluateJavascript
    // is pending. Reapply the latest complete snapshot before returning so an
    // older JS completion cannot leave the document on stale colors.
    try {
      for (var attempt = 0; attempt < 3; attempt++) {
        final appearance = _snapshot;
        await controller
            .evaluateJavascript(
                source: webCanvasScript(
                    transparent: transparent &&
                        appearance.usesSystemSurface &&
                        !appearance.reduceTransparency,
                    appearance: appearance))
            .timeout(const Duration(seconds: 2));
        if (!isCurrent() || _snapshot.sameVersion(appearance)) return;
      }
    } on MissingPluginException {
      // A departing native view may already have been disposed.
    } on PlatformException {
      // The injected Host stylesheet still supplies the readable fallback.
    } on TimeoutException {
      // Keep the bounded Host presentation flow alive if WebKit stops replying.
    }
  }
}
