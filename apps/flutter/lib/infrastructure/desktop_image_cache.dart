import 'package:flutter/painting.dart';

/// Each Flutter engine owns its decoded image cache. Application icons are
/// small; the framework's general-purpose 100 MiB limit is unnecessary here.
void configureDesktopImageCache() {
  PaintingBinding.instance.imageCache
    ..maximumSize = 128
    ..maximumSizeBytes = 8 * 1024 * 1024;
}
