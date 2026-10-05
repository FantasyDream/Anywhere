import 'package:flutter/foundation.dart';
import '../infrastructure/window_host.dart';
import '../launcher/context_source_status.dart';
import '../src/rust/api.dart';
import 'host_settings.dart';

typedef ApplyExcludedApps = Future<void> Function(List<String> bundleIds);

/// Preferences and plugin status, independent of Launcher query/selection state.
class SettingsController extends ChangeNotifier {
  SettingsController({
    required WindowHost window,
    String startupMessage = '',
    ApplyExcludedApps? applyExcludedApps,
    Future<void> Function(HostSettings)? applyContextSources,
    List<PluginStatusDto> Function()? readPlugins,
    Future<List<PluginStatusDto>> Function()? reload,
    Future<List<ShortcutActionDto>> Function()? readShortcutActions,
  })  : _window = window,
        _warning = startupMessage,
        _readPlugins = readPlugins ?? pluginStatus,
        _reload = reload ?? reloadPlugins,
        _readShortcutActions = readShortcutActions,
        _applyExcludedApps =
            applyExcludedApps ?? ((ids) => setExcludedApps(bundleIds: ids)),
        _applyContextSources = applyContextSources ??
            ((s) => setContextSources(
                finderSelection: s.captureFinderSelection,
                selectedText: s.captureSelectedText,
                browserCurrentPage: s.captureBrowserCurrentPage,
                clipboard: s.captureClipboard));

  final WindowHost _window;
  final ApplyExcludedApps _applyExcludedApps;
  final Future<void> Function(HostSettings) _applyContextSources;
  final List<PluginStatusDto> Function() _readPlugins;
  final Future<List<PluginStatusDto>> Function() _reload;
  final Future<List<ShortcutActionDto>> Function()? _readShortcutActions;
  List<ShortcutActionDto> _shortcutActions = const [];
  List<ShortcutActionDto> get shortcutActions => _shortcutActions;
  HostSettings _current = HostSettings.defaults;
  HostSettings get current => _current;
  String _warning;
  String get warning => _warning;
  String? _statusMessage;
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
    final next = await _window.readSettings();
    if (_disposed) return;
    _current = next;
    if (next.actionShortcutVersion != 1) {
      _statusMessage =
          'Action shortcuts were saved by a newer version. Update Anywhere before changing settings.';
    }
    if (plugins) _plugins = _readPlugins();
    if (plugins && _readShortcutActions != null) {
      _shortcutActions = await _readShortcutActions();
    }
    _notify();
  }

  void recordCaptures(List<ContextCaptureDto> captures) {
    _captures = List.unmodifiable(captures);
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

  /// Native close, Done, Escape, Launcher switching and quit share one draft decision.
  Future<bool> prepareClose() =>
      _closing ??= _prepareClose().whenComplete(() => _closing = null);
  Future<bool> _prepareClose() async =>
      await (beforeClose?.call() ?? Future.value(true));

  Future<bool> save(HostSettings next) async {
    if (next.actionShortcutVersion != 1) {
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
    final error = await _window.writeSettings(next);
    try {
      _current = await _window.readSettings();
    } catch (_) {
      _current = next;
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
    _statusMessage = failures.isEmpty
        ? (warning.isEmpty ? null : warning)
        : failures.join('\n');
    _notify();
    return failures.isEmpty;
  }

  Future<void> refreshPlugins() async {
    try {
      _plugins = await _reload();
    } catch (error) {
      _statusMessage = '$error';
    }
    _notify();
  }

  Future<bool> openPrivacySettings(String pane) async {
    try {
      await _window.openPrivacySettings(pane);
      return true;
    } catch (_) {
      reportError(
          'System Settings could not be opened. Open it from the Apple menu.');
      return false;
    }
  }

  @override
  void dispose() {
    _disposed = true;
    super.dispose();
  }
}
