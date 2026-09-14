import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated.dart';

import 'src/rust/frb_generated.dart';
import 'src/rust/api.dart' as bridge;
import 'launcher_screen.dart';

const windowChannel = MethodChannel('context_launcher/window');

Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();
  try {
    final executable = File(Platform.resolvedExecutable).parent.path;
    await RustLib.init(
      externalLibrary: ExternalLibrary.open(
        Platform.isWindows
            ? '$executable/launcher_bridge_flutter.dll'
            : '$executable/../Frameworks/liblauncher_bridge_flutter.dylib',
      ),
    );
    final paths = await windowChannel.invokeMapMethod<String, String>('paths');
    await bridge.initialize(
      dataDirectory: paths!['data']!,
      pluginDirectory: paths['plugins']!,
    );
    final reply = await bridge.ping();
    if (reply != 'pong') throw StateError('Rust bridge ping failed');
    runApp(
      LauncherApp(
        activate: bridge.activate,
        updateInput: (session, input, revision) => bridge.updateInput(
          sessionId: session,
          input: input,
          revision: revision,
        ),
        execute: (session, action) =>
            bridge.execute(sessionId: session, actionId: action),
        deactivate: (session) => bridge.deactivate(sessionId: session),
        window: windowChannel,
        startupMessage: paths['hotkeyError'] ?? '',
      ),
    );
  } catch (error) {
    runApp(
      MaterialApp(
        home: Scaffold(
          body: Padding(
            padding: const EdgeInsets.all(28),
            child: SelectableText('Launcher could not start.\n$error'),
          ),
        ),
      ),
    );
    await windowChannel
        .invokeMethod<void>('show', {'generation': 0, 'height': 180.0});
  }
}
