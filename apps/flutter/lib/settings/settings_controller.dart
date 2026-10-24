import 'dart:async';
import 'dart:ui' show Locale;

import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';

import '../infrastructure/application_history_backend.dart';
import '../infrastructure/clipboard_preferences_backend.dart';
import '../infrastructure/file_search_access.dart';
import '../infrastructure/runtime_policy.dart';
import '../infrastructure/window_host.dart';
import '../l10n/app_language.dart';
import '../launcher/context_source_status.dart';
import '../src/rust/api.dart' as extensions_api;
import '../src/rust/api.dart';
import 'application_history_settings_controller.dart';
import 'clipboard_settings_controller.dart';
import 'host_settings.dart';

typedef ApplyExcludedApps = Future<void> Function(List<String> bundleIds);

/// Preferences and feature status, independent of Launcher query/selection state.
class SettingsController extends ChangeNotifier {
  SettingsController({
    required DesktopHost window,
    ClipboardPreferencesBackend? clipboardBackend,
    AppHistoryPreferencesBackend? appHistoryBackend,
    String startupMessage = '',
    bool fileSearchInitializationFailed = false,
    ApplyExcludedApps? applyExcludedApps,
    Future<void> Function(HostSettings)? applyContextSources,
    Future<void> Function(HostSettings)? applyFileSearch,
    Future<void> Function(HostSettings)? authorizeFileSearch,
    FutureOr<List<ExtensionStatusDto>> Function()? readExtensions,
    Future<List<ExtensionStatusDto>> Function(String, bool)? enableExtension,
    Future<List<ExtensionStatusDto>> Function(String, String)? grantExtension,
    Future<List<ExtensionStatusDto>> Function(String)? removeExtensionGrant,
    Future<List<ExtensionStatusDto>> Function()? refreshFeatures,
    Future<List<ShortcutActionDto>> Function()? readShortcutActions,
    Future<List<String>> Function(List<String>)? readSupportedShortcutActions,
    Future<List<ShortcutApplicationDto>> Function()? readShortcutApplications,
  })  : appHistory = AppHistorySettingsController(
            appHistoryBackend ?? RustAppHistoryPreferencesBackend()),
        clipboard = ClipboardSettingsController(
            clipboardBackend ?? RustClipboardPreferencesBackend()),
        _window = window,
        _warning = startupMessage,
        _fileSearchInitializationFailed = fileSearchInitializationFailed,
        _readExtensions = readExtensions ?? extensionStatus,
        _refreshFeatures = refreshFeatures ?? extensions_api.refreshFeatures,
        _enableExtension = enableExtension ??
            ((key, enabled) =>
                extensions_api.setExtensionEnabled(key: key, enabled: enabled)),
        _grantExtension = grantExtension ??
            ((key, token) =>
                extensions_api.approveExtension(key: key, token: token)),
        _removeExtensionGrant = removeExtensionGrant ??
            ((key) => extensions_api.revokeExtension(key: key)),
        _readShortcutActions = readShortcutActions,
        _readSupportedShortcutActions = readSupportedShortcutActions,
        _readShortcutApplications = readShortcutApplications,
        _applyExcludedApps =
            applyExcludedApps ?? ((ids) => setExcludedApps(bundleIds: ids)),
        _applyFileSearch = applyFileSearch ?? FileSearchAccess.apply,
        _authorizeFileSearch = authorizeFileSearch ??
            (applyFileSearch ??
                ((s) => FileSearchAccess.apply(s, force: true))),
        _applyContextSources = applyContextSources ??
            ((s) => setContextSources(
                finderSelection: s.captureFinderSelection,
                selectedText: s.captureSelectedText,
                browserCurrentPage: s.captureBrowserCurrentPage,
                clipboard: s.captureClipboard)) {
    clipboard.addListener(_notify);
    appHistory.addListener(_notify);
  }

  final ClipboardSettingsController clipboard;
  final AppHistorySettingsController appHistory;
  final DesktopHost _window;
  final ApplyExcludedApps _applyExcludedApps;
  final Future<void> Function(HostSettings) _applyContextSources;
  final Future<void> Function(HostSettings) _applyFileSearch;
  final Future<void> Function(HostSettings) _authorizeFileSearch;
  int _searchConfiguration = 0;
  Future<void> _saveQueue = Future.value();
  final FutureOr<List<ExtensionStatusDto>> Function() _readExtensions;
  final Future<List<ExtensionStatusDto>> Function(String, bool)
      _enableExtension;
  final Future<List<ExtensionStatusDto>> Function(String, String)
      _grantExtension;
  final Future<List<ExtensionStatusDto>> Function(String) _removeExtensionGrant;
  List<ExtensionStatusDto> _extensions = const [];
  List<ExtensionStatusDto> get extensions => _extensions;
  final Future<List<ExtensionStatusDto>> Function() _refreshFeatures;
  final Future<List<ShortcutActionDto>> Function()? _readShortcutActions;
  final Future<List<String>> Function(List<String>)?
      _readSupportedShortcutActions;
  Future<void> _catalogQueue = Future.value();
  String? _catalogSyncError;
  bool _catalogRegistrationFailed = false;

  String _shortcutCatalogFailure(Object error) => error is PlatformException &&
          error.code == 'shortcuts'
      ? 'Some action shortcuts are paused. Reopen Settings to try again, or choose a different key combination.'
      : 'Action shortcuts could not be refreshed. Reopen Settings to try again.';
  int _loadEpoch = 0;
  bool _updatingFeatures = false;
  Future<void>? _extensionOperation;
  bool get updatingFeatures => _updatingFeatures;
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
  bool _fileSearchInitializationFailed;
  bool get fileSearchNeedsRecovery => _fileSearchInitializationFailed;
  static const fileSearchRecoveryMessage =
      'File search is disabled because its saved settings could not be applied. Review Search settings.';
  String get warning => [
        _warning,
        if (_fileSearchInitializationFailed) fileSearchRecoveryMessage,
      ].where((s) => s.isNotEmpty).join('\n');
  String? _statusText;
  String Function(AppLocalizations)? _localizedStatus;
  String? get _statusMessage => _statusText;
  set _statusMessage(String? value) {
    _statusText = value;
    _localizedStatus = null;
  }

  String? statusMessageFor(AppLocalizations labels) =>
      _localizedStatus?.call(labels) ?? _statusText;
  void _setLocalizedStatus(String Function(AppLocalizations) render) {
    _statusMessage = render(lookupAppLocalizations(const Locale('en')));
    _localizedStatus = render;
  }

  void _reportLocalizedError(String Function(AppLocalizations) render) {
    _setLocalizedStatus(render);
    _notify();
  }

  String? shortcutCatalogMessageFor(AppLocalizations labels) =>
      _catalogSyncError == null
          ? null
          : _catalogRegistrationFailed
              ? labels.shortcutsPausedHelp
              : labels.shortcutsRefreshFailed;
  String? _shortcutRestoreError;
  String? get statusMessage => _statusMessage;
  List<ContextCaptureDto> _captures = const [];
  Future<bool> Function()? beforeClose;
  Future<bool>? _closing;
  bool _disposed = false;

  void _notify() {
    if (!_disposed) notifyListeners();
  }

  Future<void> load({bool catalog = false}) async {
    final epoch = ++_loadEpoch;
    final next = await _window.readSettings();
    if (_disposed || epoch != _loadEpoch) return;
    final previousWarning = warning;
    var nextWarning = _warning;
    try {
      nextWarning = await _window.readSettingsStatus();
    } catch (_) {}
    if (_disposed || epoch != _loadEpoch) return;
    _current = next;
    _warning = nextWarning;
    if (next.fileSearchError != null) _fileSearchInitializationFailed = true;
    if (warning.isNotEmpty || _statusMessage == previousWarning) {
      _statusMessage = warning.isEmpty ? null : warning;
    }
    if (next.actionShortcutVersion != 4) {
      _setLocalizedStatus((labels) => labels.shortcutsSavedByNewerVersion);
    }
    if (catalog) {
      try {
        final statuses = await _readExtensions();
        if (_disposed || epoch != _loadEpoch) return;
        _extensions = statuses;
      } catch (error) {
        if (_disposed || epoch != _loadEpoch) return;
        _setLocalizedStatus((labels) => labels.extensionsLoadFailed('$error'));
      }
    }
    if (catalog && _readShortcutActions != null) {
      final actions = await _readShortcutActions();
      if (_disposed || epoch != _loadEpoch) return;
      _shortcutActions = actions;
    }
    if (catalog && _readShortcutApplications != null) {
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
      final previousError = _catalogSyncError;
      _catalogSyncError = null;
      _catalogRegistrationFailed = false;
      final snapshot = current;
      final ids = _bindingActionIds(snapshot);
      try {
        final supported = await reader(ids);
        if (_disposed) return;
        // A concurrent save must not publish eligibility for its predecessor.
        if (!identical(snapshot, current)) return;
        _supportedShortcutActions = Set.unmodifiable(supported);
        await _window.syncActionShortcutCatalog(snapshot, supported);
        final previousWarning = warning;
        _warning = await _window.readSettingsStatus();
        if (warning.isNotEmpty ||
            _statusMessage == previousWarning ||
            (previousError != null && _statusMessage == previousError)) {
          _statusMessage = warning.isEmpty ? null : warning;
        }
      } catch (error) {
        if (_disposed || !identical(snapshot, current)) return;
        _catalogSyncError = _shortcutCatalogFailure(error);
        _catalogRegistrationFailed =
            error is PlatformException && error.code == 'shortcuts';
        _supportedShortcutActions = const {};
        // An uncertain catalog must not keep a revoked action's system chord alive.
        try {
          await _window.syncActionShortcutCatalog(snapshot, const []);
        } catch (_) {}
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
    _reportLocalizedError(
        (labels) => '${labels.shortcutsRestoreFailed}\n$detail');
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
  Future<bool> _prepareClose() async {
    // Capture every pending result before awaiting another controller. A failed
    // save can finish and clear its queue while the other queue is draining.
    final historyPending = appHistory.waitForPending();
    final clipboardPending = clipboard.waitForPending();
    // Finish catalog invalidation before the host restores global action hotkeys.
    await _extensionOperation;
    final results = await Future.wait([historyPending, clipboardPending]);
    if (!results[0]) {
      _reportLocalizedError((labels) => labels.appHistorySettingsFailed);
      return false;
    }
    if (!results[1]) {
      _reportLocalizedError((labels) =>
          clipboard.noticeFor(labels) ?? labels.clipboardSettingsFailed);
      return false;
    }
    if (_disposed) return false;
    return await (beforeClose?.call() ?? Future.value(true));
  }

  Future<bool> save(HostSettings next) {
    // Invalidate authorization/readback immediately, even while an earlier save
    // is still applying. Serialization belongs to this service, not its Widget.
    ++_searchConfiguration;
    ++_loadEpoch;
    final result = _saveQueue.then((_) => _save(next));
    _saveQueue = result.then<void>((_) {}, onError: (Object _) {});
    return result;
  }

  Future<bool> _save(HostSettings next) async {
    if (_disposed) return false;
    if (next.fileSearchError != null) {
      _reportLocalizedError((labels) => next.fileSearchErrorFor(labels)!);
      return false;
    }
    if (next.actionShortcutVersion != 4) {
      _reportLocalizedError((labels) => labels.unsupportedShortcutSettings);
      return false;
    }
    if (next.actionShortcuts.length > RuntimePolicy.maxActionShortcuts) {
      _reportLocalizedError((labels) =>
          labels.maxActionShortcuts(RuntimePolicy.maxActionShortcuts));
      return false;
    }
    final ids = <String>{};
    for (final binding in next.actionShortcuts) {
      final error = binding.validate(next);
      if (error != null || !ids.add(binding.actionId)) {
        _reportLocalizedError((labels) =>
            binding.validate(next, labels) ?? labels.oneShortcutPerAction);
        return false;
      }
    }
    List<String>? supported;
    try {
      supported =
          await _readSupportedShortcutActions?.call(_bindingActionIds(next));
    } catch (error) {
      if (_disposed) return false;
      await _syncShortcutCatalog();
      if (_disposed) return false;
      reportError(_shortcutCatalogFailure(error));
      return false;
    }
    if (_disposed) return false;
    final previous = _current;
    final error =
        await _window.writeSettings(next, supportedActionIds: supported);
    if (_disposed) return false;
    try {
      final readback = await _window.readSettings();
      if (_disposed) return false;
      _current = readback;
    } catch (_) {
      // Never publish a rejected binding when the authoritative readback fails.
      _current = error == null ? next : previous;
    }
    if (_disposed) return false;
    try {
      final status = await _window.readSettingsStatus();
      if (_disposed) return false;
      _warning = status;
    } catch (_) {}
    final failures = <String>[if (error != null && error.isNotEmpty) error];
    if (_disposed) return false;
    try {
      await _applyExcludedApps(current.excludedApps);
    } catch (error) {
      failures.add('$error');
    }
    if (_disposed) return false;
    try {
      await _applyContextSources(current);
    } catch (error) {
      failures.add('$error');
    }
    if (_disposed) return false;
    try {
      await _applyFileSearch(current);
      if (_disposed) return false;
      final readback = await _window.readSettings();
      if (_disposed) return false;
      _current = readback;
      _fileSearchInitializationFailed = false;
    } catch (_) {
      _fileSearchInitializationFailed = true;
      failures
          .add('File search settings could not be applied. Try saving again.');
    }
    if (_disposed) return false;
    try {
      final status = await _window.readSettingsStatus();
      if (_disposed) return false;
      _warning = status;
    } catch (_) {}
    if (_disposed) return false;
    await _syncShortcutCatalog();
    if (_disposed) return false;
    if (_catalogSyncError != null) failures.add(_catalogSyncError!);
    _statusMessage = failures.isEmpty
        ? (warning.isEmpty ? null : warning)
        : failures.join('\n');
    _notify();
    return failures.isEmpty;
  }

  Future<void> authorizeFileSearch() async {
    if (_authorizingFiles || _disposed) return;
    _authorizingFiles = true;
    _notify();
    final configuration = _searchConfiguration;
    try {
      bool currentRequest() =>
          !_disposed && configuration == _searchConfiguration;
      await _saveQueue;
      if (!currentRequest()) return;
      await _authorizeFileSearch(current);
      if (!currentRequest()) return;
      final next = await _window.readSettings();
      if (!currentRequest()) return;
      final warning = await _window.readSettingsStatus();
      if (!currentRequest()) return;
      _current = next;
      _fileSearchInitializationFailed = false;
      _warning = warning;
      _statusMessage = warning.isEmpty ? null : warning;
    } on SearchAccessSuperseded {
      // A save owns the newer configuration and its status.
    } catch (_) {
      if (_disposed || configuration != _searchConfiguration) return;
      _fileSearchInitializationFailed = true;
      _setLocalizedStatus((labels) => labels.directoryAccessFailed);
    } finally {
      _authorizingFiles = false;
      _notify();
    }
  }

  bool _authorizingFiles = false;
  bool get authorizingFiles => _authorizingFiles;

  Future<void> refreshFeatureData() => _changeExtensions(_refreshFeatures);

  Future<void> setExtensionEnabled(
          ExtensionStatusDto extension, bool enabled) =>
      _changeExtensions(() => _enableExtension(extension.key, enabled));

  Future<void> approveExtension(ExtensionStatusDto extension) =>
      _changeExtensions(
          () => _grantExtension(extension.key, extension.approvalToken));

  Future<void> revokeExtension(ExtensionStatusDto extension) =>
      _changeExtensions(() => _removeExtensionGrant(extension.key));

  Future<void> _changeExtensions(
          Future<List<ExtensionStatusDto>> Function() change) =>
      _runExtensionOperation(() async {
        final statuses = await change();
        if (!_disposed) _extensions = statuses;
      });

  Future<void> _runExtensionOperation(Future<void> Function() operation) {
    if (_updatingFeatures || _disposed || _closing != null) {
      return Future.value();
    }
    final pending = _updateExtensions(operation);
    _extensionOperation = pending;
    return pending;
  }

  Future<void> _updateExtensions(Future<void> Function() operation) async {
    ++_loadEpoch;
    _updatingFeatures = true;
    _statusMessage = null;
    _notify();
    try {
      await operation();
      if (_disposed) return;
      ++_loadEpoch;
    } catch (error) {
      if (!_disposed) {
        _setLocalizedStatus(
            (labels) => labels.extensionsUpdateFailed('$error'));
      }
    } finally {
      if (!_disposed && _readShortcutActions != null) {
        try {
          final actions = await _readShortcutActions();
          if (!_disposed) _shortcutActions = actions;
        } catch (error) {
          if (!_disposed) {
            _shortcutActions = const [];
            if (_statusMessage == null) {
              _setLocalizedStatus(
                  (labels) => labels.shortcutLoadFailed('$error'));
            }
          }
        }
      }
      // The backend can commit before its response/status read fails. Always
      // reconcile eligibility; the sync itself unregisters uncertain chords.
      await _syncShortcutCatalog();
      _updatingFeatures = false;
      _notify();
    }
  }

  Future<bool> openPrivacySettings(String pane) async {
    try {
      await _window.openPrivacySettings(pane);
      return true;
    } catch (_) {
      _reportLocalizedError((labels) => labels.openSystemSettingsFailed);
      return false;
    }
  }

  @override
  void dispose() {
    _disposed = true;
    clipboard.removeListener(_notify);
    clipboard.dispose();
    appHistory.removeListener(_notify);
    appHistory.dispose();
    super.dispose();
  }
}
