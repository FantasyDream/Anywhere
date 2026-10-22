import 'package:flutter/services.dart';
import 'package:flutter_inappwebview/flutter_inappwebview.dart';

enum WebPresentationEvent { panel, visibility }

/// Best-effort presentation notifications. A departing platform view can reject
/// evaluation; that must not become an unhandled error during hide or teardown.
Future<void> notifyWebPresentation(InAppWebViewController? controller,
    WebPresentationEvent event, bool value) async {
  if (controller == null) return;
  try {
    await controller.evaluateJavascript(
        source:
            "window.dispatchEvent(new CustomEvent('anywhere:${event.name}',{detail:$value}));");
  } on MissingPluginException {
    // The native view was removed or this platform has no adapter.
  } on PlatformException {
    // Platform disposal can race with a presentation notification.
  }
}
