import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated.dart';

import 'src/rust/frb_generated.dart';
import 'src/rust/api.dart' as bridge;
import 'launcher_screen.dart';

const windowChannel = MethodChannel('anywhere/window');

Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();
  try {
    final executable = File(Platform.resolvedExecutable).parent.path;
    await RustLib.init(
      externalLibrary: ExternalLibrary.open(
        '$executable/../Frameworks/libanywhere_bridge_flutter.dylib',
      ),
    );
    final paths = await windowChannel.invokeMapMethod<String, String>('paths');
    await bridge.initialize(
      dataDirectory: paths!['data']!,
      pluginDirectory: paths['plugins']!,
    );
    final excluded = (paths['excludedApps'] ?? '')
        .split('\n')
        .map((id) => id.trim())
        .where((id) => id.isNotEmpty)
        .toList();
    if (excluded.isNotEmpty) {
      await bridge.setExcludedApps(bundleIds: excluded);
    }
    await bridge.setContextSources(
      finderSelection: paths['captureFinderSelection'] != '0',
      selectedText: paths['captureSelectedText'] != '0',
      browserCurrentPage: paths['captureBrowserCurrentPage'] != '0',
      clipboard: paths['captureClipboard'] != '0',
    );
    final reply = await bridge.ping();
    if (reply != 'pong') throw StateError('Rust bridge ping failed');
    runApp(
      LauncherApp(
        beginActivation: bridge.beginActivation,
        activate: (session, {promptAccessibility = false}) => bridge.activate(
          sessionId: session,
          promptAccessibility: promptAccessibility,
        ),
        updateInput: (session, input, revision, {scopeId}) =>
            bridge.updateInput(
          sessionId: session,
          input: input,
          scopeId: scopeId,
          revision: revision,
        ),
        execute: (session, revision, candidate) => bridge.execute(
          sessionId: session,
          revision: revision,
          candidateId: candidate,
        ),
        waitForSnapshot: (session, after) => bridge.waitForSnapshot(
          sessionId: session,
          after: BigInt.from(after),
        ),
        loadIcon: (candidate) => bridge.applicationIcon(candidateId: candidate),
        promptAccessibility: () => bridge.promptAccessibility(),
        deactivate: (session) => bridge.deactivate(sessionId: session),
        window: windowChannel,
        startupMessage: [
          paths['hotkeyError'] ?? '',
          paths['loginError'] ?? '',
        ].where((item) => item.isNotEmpty).join('\n'),
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
    await windowChannel.invokeMethod<void>('show', {
      'generation': 0,
      'height': 180.0,
    });
  }
}
