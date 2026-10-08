import 'package:flutter/foundation.dart';
import '../infrastructure/window_host.dart';
import '../launcher/context_source_status.dart';
import '../src/rust/api.dart';
import 'host_settings.dart';

typedef ApplyExcludedApps = Future<void> Function(List<String> bundleIds);

/// Preferences and plugin status, independent of Launcher query/selection state.
class SettingsController extends ChangeNotifier {
  SettingsController({
    required DesktopHost window,
    String startupMessage = '',
    ApplyExcludedApps? applyExcludedApps,
    Future<void> Function(HostSettings)? applyContextSources,
    List<PluginStatusDto> Function()? readPlugins,
    Future<List<PluginStatusDto>> Function()? reload,
    Future<List<ShortcutActionDto>> Function()? readShortcutActions,
    Future<List<String>> Function(List<String>)? readSupportedShortcutActions,
    Future<List<ShortcutApplicationDto>> Function()? readShortcutApplications,
  })  : _window = window,
        _warning = startupMessage,
        _readPlugins = readPlugins ?? pluginStatus,
        _reload = reload ?? reloadPlugins,
        _readShortcutActions = readShortcutActions,
        _readSupportedShortcutActions = readSupportedShortcutActions,
        _readShortcutApplications = readShortcutApplications,
        _applyExcludedApps =
            applyExcludedApps ?? ((ids) => setExcludedApps(bundleIds: ids)),
        _applyContextSources = applyContextSources ??
            ((s) => setContextSources(
                finderSelection: s.captureFinderSelection,
                selectedText: s.captureSelectedText,
                browserCurrentPage: s.captureBrowserCurrentPage,
                clipboard: s.captureClipboard));

  final DesktopHost _window;
  final ApplyExcludedApps _applyExcludedApps;
  final Future<void> Function(HostSettings) _applyContextSources;
  final List<PluginStatusDto> Function() _readPlugins;
  final Future<List<PluginStatusDto>> Function() _reload;
  final Future<List<ShortcutActionDto>> Function()? _readShortcutActions;
  final Future<List<String>> Function(List<String>)?
      _readSupportedShortcutActions;
  Future<void> _catalogQueue = Future.value();
  String? _catalogSyncError;
  int _loadEpoch = 0;
  bool _refreshingPlugins = false;
  bool get refreshingPlugins => _refreshingPlugins;
  Set<String> _supportedShortcutActions = const {};
  bool shortcutSupported(String id) =>
      _readSupportedShortcutActions == null ||
      _supportedShortcutActions.contains(id);
  final Future<List<ShortcutApplicationDto>> Function()?
      _readShortcutApplications;
  List<ShortcutApplicationDto> _shortcutApplications = const [];
  List<ShortcutApplicationDto> get shortcutApplications =>
      _shortcutApplications;
  List<ShortcutActionDto> _shortcutActions = const [];
  List<ShortcutActionDto> get shortcutActions => _shortcutActions;
  HostSettings _current = HostSettings.defaults;
  HostSettings get current => _current;
  String _warning;
  String get warning => _warning;
  String? _statusMessage;
  String? _shortcutRestoreError;
  String? get statusMessage => _statusMessage;
  List<PluginStatusDto> _plugins = const [];
  List<PluginStatusDto> get plugins => _plugins;
  List<ContextCaptureDto> _captures = const [];
  Future<bool> Function()? beforeClose;
  Future<bool>? _closing;
  bool _disposed = false;

  void _notify() {
    if (!_disposed) notifyListeners();
  }

  Future<void> load({bool plugins = false}) async {
    final epoch = ++_loadEpoch;
    final next = await _window.readSettings();
    if (_disposed || epoch != _loadEpoch) return;
    _current = next;
    final previousWarning = _warning;
    try {
      _warning = await _window.readSettingsStatus();
    } catch (_) {}
    if (_disposed || epoch != _loadEpoch) return;
    if (_warning.isNotEmpty || _statusMessage == previousWarning) {
      _statusMessage = _warning.isEmpty ? null : _warning;
    }
    if (next.actionShortcutVersion != 4) {
      _statusMessage =
          'Action shortcuts were saved by a newer version. Update Anywhere before changing settings.';
    }
    if (plugins) _plugins = _readPlugins();
    if (plugins && _readShortcutActions != null) {
      final actions = await _readShortcutActions();
      if (_disposed || epoch != _loadEpoch) return;
      _shortcutActions = actions;
    }
    if (plugins && _readShortcutApplications != null) {
      final apps = await _readShortcutApplications();
      if (_disposed || epoch != _loadEpoch) return;
      _shortcutApplications = apps;
    }
    if (_disposed || epoch != _loadEpoch) return;
    await _syncShortcutCatalog();
    _notify();
  }

  List<String> _bindingActionIds(HostSettings settings) => <String>{
        for (final binding in settings.actionShortcuts)
          if (binding.isIntent)
            ...binding.routes.map((r) => r.actionId)
          else
            binding.actionId,
      }.toList();

  /// Serialize catalog reads and Host writes; each operation reads current settings
  /// when it starts. Native checks the configuration snapshot again before applying.
  Future<void> _syncShortcutCatalog() {
    final reader = _readSupportedShortcutActions;
    if (reader == null || _disposed) return Future.value();
    final next = _catalogQueue.then((_) async {
      if (_disposed || current.actionShortcutVersion != 4) return;
      _catalogSyncError = null;
      final snapshot = current;
      final ids = _bindingActionIds(snapshot);
      try {
        final supported = await reader(ids);
        if (_disposed) return;
        // A concurrent save must not publish eligibility for its predecessor.
        if (!identical(snapshot, current)) return;
        _supportedShortcutActions = Set.unmodifiable(supported);
        await _window.syncActionShortcutCatalog(snapshot, supported);
        final previousWarning = _warning;
        _warning = await _window.readSettingsStatus();
        if (_warning.isNotEmpty ||
            _statusMessage == previousWarning ||
            _statusMessage
                    ?.startsWith('Action shortcuts could not be refreshed:') ==
                true) {
          _statusMessage = _warning.isEmpty ? null : _warning;
        }
      } catch (error) {
        if (_disposed || !identical(snapshot, current)) return;
        _catalogSyncError = 'Action shortcuts could not be refreshed: $error';
        _supportedShortcutActions = const {};
        // An uncertain catalog must not keep a revoked action's system chord alive.
        try {
          await _window.syncActionShortcutCatalog(snapshot, const []);
        } catch (_) {}
        _statusMessage = _catalogSyncError;
        _notify();
      }
    });
    _catalogQueue = next;
    return next;
  }

  void recordCaptures(List<ContextCaptureDto> captures) {
    _captures = List.unmodifiable(captures.map((capture) => ContextCaptureDto(
        source: capture.source,
        status: capture.status,
        capturedAtMs: capture.capturedAtMs)));
  }

  List<ContextSourceStatus> get contextSourceStatuses => [
        for (final source in ContextSourceStatus.sources)
          ContextSourceStatus(
              source, _captures.where((c) => c.source == source).firstOrNull,
              enabled: switch (source) {
                ContextSourceDto.finderSelection =>
                  current.captureFinderSelection,
                ContextSourceDto.selectedText => current.captureSelectedText,
                ContextSourceDto.browserCurrentPage =>
                  current.captureBrowserCurrentPage,
                ContextSourceDto.clipboard => current.captureClipboard,
                _ => false,
              }),
      ];
  void reportError(String message) {
    _statusMessage = message;
    _notify();
  }

  void reportShortcutRestoreError(String detail) {
    _shortcutRestoreError =
        'Action shortcuts could not be restored. Change the conflicting shortcut or try closing again.\n$detail';
    reportError(_shortcutRestoreError!);
  }

  void shortcutsRestored() {
    if (_shortcutRestoreError != null &&
        _statusMessage == _shortcutRestoreError) {
      _statusMessage = null;
      _notify();
    }
    _shortcutRestoreError = null;
  }

  /// Native close, Done, Escape, Launcher switching and quit share one draft decision.
  Future<bool> prepareClose() =>
      _closing ??= _prepareClose().whenComplete(() => _closing = null);
  Future<bool> _prepareClose() async =>
      await (beforeClose?.call() ?? Future.value(true));

  Future<bool> save(HostSettings next) async {
    if (next.actionShortcutVersion != 4) {
      reportError(
          'Unsupported action shortcut settings version. Update Anywhere before changing settings.');
      return false;
    }
    if (next.actionShortcuts.length > 128) {
      reportError('At most 128 action shortcuts are supported.');
      return false;
    }
    final ids = <String>{};
    for (final binding in next.actionShortcuts) {
      final error = binding.validate(next);
      if (error != null || !ids.add(binding.actionId)) {
        reportError(error ?? 'An action can only have one shortcut.');
        return false;
      }
    }
    List<String>? supported;
    try {
      supported =
          await _readSupportedShortcutActions?.call(_bindingActionIds(next));
    } catch (error) {
      await _syncShortcutCatalog();
      reportError('Action shortcuts could not be refreshed: $error');
      return false;
    }
    if (_disposed) return false;
    ++_loadEpoch; // Invalidate settings loads started before this save.
    final previous = _current;
    final error =
        await _window.writeSettings(next, supportedActionIds: supported);
    try {
      _current = await _window.readSettings();
    } catch (_) {
      // Never publish a rejected binding when the authoritative readback fails.
      _current = error == null ? next : previous;
    }
    try {
      _warning = await _window.readSettingsStatus();
    } catch (_) {}
    final failures = <String>[if (error != null && error.isNotEmpty) error];
    try {
      await _applyExcludedApps(current.excludedApps);
    } catch (error) {
      failures.add('$error');
    }
    try {
      await _applyContextSources(current);
    } catch (error) {
      failures.add('$error');
    }
    await _syncShortcutCatalog();
    if (_catalogSyncError != null) failures.add(_catalogSyncError!);
    _statusMessage = failures.isEmpty
        ? (warning.isEmpty ? null : warning)
        : failures.join('\n');
    _notify();
    return failures.isEmpty;
  }

  Future<void> refreshPlugins() async {
    if (_refreshingPlugins || _disposed) return;
    _refreshingPlugins = true;
    _notify();
    try {
      _plugins = await _reload();
      if (_disposed) return;
      try {
        if (_readShortcutActions != null) {
          _shortcutActions = await _readShortcutActions();
        }
      } finally {
        await _syncShortcutCatalog();
      }
    } catch (error) {
      _statusMessage = '$error';
    } finally {
      _refreshingPlugins = false;
      _notify();
    }
  }

  Future<bool> openPrivacySettings(String pane) async {
    try {
      await _window.openPrivacySettings(pane);
      return true;
    } catch (_) {
      reportError(
          'System Settings could not be opened. Open the system settings manually.');
      return false;
    }
  }

  @override
  void dispose() {
    _disposed = true;
    super.dispose();
  }
}
