import 'dart:async';
import 'package:flutter/services.dart';
import 'package:flutter_inappwebview/flutter_inappwebview.dart';

/// Public presentation metadata only; never a grant or an invocation parameter.
/// The value is restricted to supported language tags before entering JavaScript.
String webLocaleScript(String language) {
  final value = language == 'zh' ? 'zh' : 'en';
  return '''
(() => {
  const changed = window.__anywhereLanguage !== '$value';
  window.__anywhereLanguage = '$value';
  if (document.documentElement) document.documentElement.lang = '$value';
  if (changed) window.dispatchEvent(new CustomEvent('anywhere:language', {detail:'$value'}));
})();
''';
}

Future<void> notifyWebLanguage(
    InAppWebViewController? controller, String language) async {
  if (controller == null) return;
  try {
    await controller
        .evaluateJavascript(source: webLocaleScript(language))
        .timeout(const Duration(milliseconds: 300));
  } on MissingPluginException {
    // Native teardown may race a language notification.
  } on PlatformException {
    // An inactive page is updated again on resume/load.
  } on TimeoutException {
    // A nonresponsive document must not block initial display or focus restoration.
  }
}
