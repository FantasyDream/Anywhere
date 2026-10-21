import 'dart:io';
import 'package:flutter_inappwebview_macos/flutter_inappwebview_macos.dart';

/// The manually assembled native host registers only the accepted platform adapter.
void registerWebPlatform() {
  if (Platform.isMacOS) MacOSInAppWebViewPlatform.registerWith();
}
