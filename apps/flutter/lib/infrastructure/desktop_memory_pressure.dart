import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';

import 'desktop_image_cache.dart';

enum DesktopMemoryPressureLevel { warning, critical }

/// One native pressure subscription per Flutter engine. It carries only the
/// severity and never requests or receives data snapshots.
class DesktopMemoryPressure extends ChangeNotifier {
  DesktopMemoryPressure(
      {this.channel = const MethodChannel('anywhere/memory-pressure')});

  static DesktopMemoryPressure current = DesktopMemoryPressure();

  final MethodChannel channel;
  DesktopMemoryPressureLevel? _level;
  bool _attached = false;

  DesktopMemoryPressureLevel? get level => _level;

  void attach() {
    if (_attached) return;
    _attached = true;
    channel.setMethodCallHandler((call) async {
      if (call.method != 'memoryPressure' || call.arguments is! String) {
        return null;
      }
      final level = call.arguments == 'critical'
          ? DesktopMemoryPressureLevel.critical
          : null;
      if (level == null && call.arguments != 'warning') return null;
      _level = level ?? DesktopMemoryPressureLevel.warning;
      try {
        releaseIdleDesktopImages();
      } catch (_) {
        // A pressure notification must never interrupt the interaction.
      }
      notifyListeners();
      return null;
    });
  }

  void resetForTest() {
    current.dispose();
    current = DesktopMemoryPressure();
  }

  @override
  void dispose() {
    if (_attached) {
      channel.setMethodCallHandler(null);
      _attached = false;
    }
    super.dispose();
  }
}
