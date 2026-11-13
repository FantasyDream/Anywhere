import 'dart:async';

import 'package:flutter/services.dart';

import 'presentation_metrics.dart';
import 'runtime_paths.dart';

/// Each lazy engine has its own Dart globals and host channel.
void configurePresentationMetrics(RuntimePaths paths, MethodChannel channel) {
  final enabled = paths.values['profileUi'] == '1';
  PresentationTransferStats.reset();
  PresentationMetrics.reporter = enabled
      ? (sample) => unawaited(channel
          .invokeMethod<void>('performanceSample', sample)
          .catchError((Object _) {}))
      : null;
}
