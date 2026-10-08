import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated.dart';

import 'src/rust/frb_generated.dart';
import 'src/rust/api.dart' as bridge;
import 'launcher_screen.dart';
import 'ui/app_style.dart';
import 'ui/app_components.dart';
import 'infrastructure/runtime_paths.dart';
import 'infrastructure/desktop_image_cache.dart';
import 'infrastructure/desktop_environment.dart';
import 'execution/execution_sessions_screen.dart';

const windowChannel = MethodChannel('anywhere/window');

Future<void> initializeRustLibrary(RuntimePaths paths) async {
  await RustLib.init(externalLibrary: ExternalLibrary.open(paths.library));
}

/// A second window owns only its view; the app-lifetime Rust Application is shared.
@pragma('vm:entry-point')
Future<void> executionSessionsMain() async {
  WidgetsFlutterBinding.ensureInitialized();
  configureDesktopImageCache();
  try {
    const channel = MethodChannel('anywhere/executions');
    final paths = await RuntimePaths.read(channel);
    DesktopEnvironment.configure(await DesktopEnvironment.read(channel));
    await initializeRustLibrary(paths);
    runApp(const ExecutionSessionsApp());
  } catch (error) {
    runApp(MaterialApp(
        debugShowCheckedModeBanner: false,
        theme: AppStyle.theme,
        builder: AppStyle.windowBuilder,
        home: AppFailureView(
            title: 'Running Tasks could not open', detail: '$error')));
  }
}

Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();
  configureDesktopImageCache();
  try {
    final runtime = await RuntimePaths.read(windowChannel);
    DesktopEnvironment.configure(await DesktopEnvironment.read(windowChannel));
    await initializeRustLibrary(runtime);
    final paths = runtime.values;
    await bridge.initialize(
      dataDirectory: runtime.requiredPath('data'),
      pluginDirectory: runtime.requiredPath('plugins'),
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
        activate: (session,
                {promptAccessibility = false, capturedEnvironment = false}) =>
            bridge.activate(
          sessionId: session,
          promptAccessibility: promptAccessibility,
          capturedEnvironment: capturedEnvironment,
        ),
        updateInput: (session, input, revision,
                {scopeId, inputMode = bridge.InputModeDto.environment}) =>
            bridge.updateInput(
          sessionId: session,
          input: input,
          scopeId: scopeId,
          inputMode: inputMode,
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
        shortcutActions: bridge.shortcutActions,
        supportedShortcutActions: (ids) =>
            bridge.supportedShortcutActions(actionIds: ids),
        shortcutApplications: bridge.shortcutApplications,
        resolveIntent: (session, revision, routes) =>
            bridge.resolveShortcutIntent(
                sessionId: session, revision: revision, routes: routes),
        waitIntent: (session, revision, routes) => bridge.waitShortcutIntent(
            sessionId: session, revision: revision, routes: routes),
        resolveBinding: (session, revision, action) =>
            bridge.resolveActionBinding(
                sessionId: session, revision: revision, actionId: action),
        waitBinding: (session, revision, action) => bridge.waitActionBinding(
            sessionId: session, revision: revision, actionId: action),
        deactivate: (session) => bridge.deactivate(sessionId: session),
        window: windowChannel,
        startupMessage: [
          paths['hotkeyError'] ?? '',
          paths['loginError'] ?? '',
        ].where((item) => item.isNotEmpty).join('\n'),
      ),
    );
  } catch (error) {
    runApp(MaterialApp(
        debugShowCheckedModeBanner: false,
        theme: AppStyle.theme,
        builder: AppStyle.windowBuilder,
        home: AppFailureView(
            title: 'Launcher could not start', detail: '$error')));
    await windowChannel.invokeMethod<void>('show', {
      'generation': 0,
      'height': 180.0,
    });
  }
}
