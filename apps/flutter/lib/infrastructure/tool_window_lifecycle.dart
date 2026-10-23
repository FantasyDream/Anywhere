import 'package:flutter/widgets.dart';

/// Dedicated tool isolates unmount their view before the native engine stops.
/// The frame clears native text/semantics clients and disposes view-owned work.
/// Never use this for Launcher/Settings, which share a persistent isolate.
Future<void> prepareToolWindowToClose() async {
  runApp(const SizedBox.shrink());
  await WidgetsBinding.instance.endOfFrame;
}
