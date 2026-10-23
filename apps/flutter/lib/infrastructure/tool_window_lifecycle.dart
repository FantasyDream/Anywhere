import 'package:flutter/widgets.dart';

/// Dedicated tool isolates unmount their view before the native engine stops.
/// The frame clears native text/semantics clients and disposes view-owned work.
/// Never use this for Launcher/Settings, which share a persistent isolate.
Future<void> prepareToolWindowToClose() async {
  final binding = WidgetsBinding.instance;
  // runApp starts a synthetic warm-up frame and can leave a real VSync pending
  // after its endOfFrame completes. Close on the ordinary frame instead.
  binding
      .attachRootWidget(binding.wrapWithDefaultView(const SizedBox.shrink()));
  await binding.endOfFrame;
}
