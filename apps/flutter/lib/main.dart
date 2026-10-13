import 'infrastructure/file_search_access.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated.dart';

import 'src/rust/frb_generated.dart';
import 'src/rust/api.dart' as bridge;
import 'launcher_screen.dart';
import 'ui/app_style.dart';
import 'ui/app_components.dart';
import 'infrastructure/runtime_paths.dart';
import 'infrastructure/window_host.dart';
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
    var fileSearchInitializationFailed = false;
    try {
      final preferences = await WindowHost(windowChannel).readSettings();
      await FileSearchAccess.apply(preferences);
    } catch (_) {
      await bridge.setFileSearch(
          enabled: false,
          roots: [],
          excludedDirectoryNames: [],
          engine: bridge.FileSearchEngineDto.systemIndex,
          includeHidden: false,
          restrictToRoots: true);
      fileSearchInitializationFailed = true;
    }
    final reply = await bridge.ping();
    if (reply != 'pong') throw StateError('Rust bridge ping failed');
    runApp(
      LauncherApp(
        waitForSnapshotDelta: (session, after, version, count) =>
            bridge.waitForSnapshotDelta(
                sessionId: session,
                after: BigInt.from(after),
                candidateListVersion: version,
                loadedCount: count),
        beginActivation: bridge.beginActivation,
        chooseActivationInput: (captures, nowMs,
                {required selectedEnabled,
                required clipboardEnabled,
                required allowPendingSelection}) =>
            bridge.chooseActivationInput(
                captures: captures,
                nowMs: nowMs,
                selectedEnabled: selectedEnabled,
                clipboardEnabled: clipboardEnabled,
                allowPendingSelection: allowPendingSelection),
        suspend: (session) => bridge.suspendLauncher(sessionId: session),
        resume: (session) async => bridge.resumeLauncher(sessionId: session),
        activate: (session,
                {promptAccessibility = false, capturedEnvironment = false}) =>
            bridge.activate(
          sessionId: session,
          promptAccessibility: promptAccessibility,
          capturedEnvironment: capturedEnvironment,
        ),
        updateFiles: (session, input, revision, filter) =>
            bridge.updateFileInput(
                sessionId: session,
                input: input,
                revision: revision,
                filter: filter),
        fileMoreDelta: (session, revision, version, count) =>
            bridge.loadMoreFilesDelta(
                sessionId: session,
                revision: revision,
                candidateListVersion: version,
                loadedCount: count),
        fileMore: (session, revision) =>
            bridge.loadMoreFiles(sessionId: session, revision: revision),
        fileActions: (session, revision, candidate) async => bridge.fileActions(
            sessionId: session, revision: revision, candidateId: candidate),
        freezeFiles: (session, revision) =>
            bridge.freezeFileResults(sessionId: session, revision: revision),
        loadFilePreview: (session, revision, candidate) => bridge.previewFile(
            sessionId: session, revision: revision, candidateId: candidate),
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
        fileSearchInitializationFailed: fileSearchInitializationFailed,
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
