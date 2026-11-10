import 'dart:async';
import 'dart:ui' show Locale;

import 'package:flutter/foundation.dart';

import '../infrastructure/application_history_backend.dart';
import '../infrastructure/bridge_api.dart' as extensions_api;
import '../infrastructure/bridge_api.dart';
import '../infrastructure/clipboard_preferences_backend.dart';
import '../infrastructure/file_search_access.dart';
import '../infrastructure/runtime_policy.dart';
import '../infrastructure/window_host.dart';
import '../l10n/app_language.dart';
import '../launcher/context_source_status.dart';
import 'application_history_settings_controller.dart';
import 'clipboard_settings_controller.dart';
import 'feature_catalog_coordinator.dart';
import 'host_settings.dart';
import 'settings_edits.dart';
import 'settings_save_coordinator.dart';
import 'shortcut_catalog_coordinator.dart';

enum SettingsCloseChoice { save, discard, cancel }

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
        _window = window {
    _saves = SettingsSaveCoordinator(
        window: window,
        startupMessage: startupMessage,
        fileSearchInitializationFailed: fileSearchInitializationFailed,
        applyExcludedApps:
            applyExcludedApps ?? ((ids) => setExcludedApps(bundleIds: ids)),
        applyFileSearch: applyFileSearch ?? FileSearchAccess.apply,
        authorizeFileSearch: authorizeFileSearch ??
            (applyFileSearch ??
                ((s) => FileSearchAccess.apply(s, force: true))),
        applyContextSources: applyContextSources ??
            ((s) => setContextSources(
                finderSelection: s.captureFinderSelection,
                selectedText: s.captureSelectedText,
                browserCurrentPage: s.captureBrowserCurrentPage,
                clipboard: s.captureClipboard)));
    _shortcuts = ShortcutCatalogCoordinator(
        window: window,
        readSettings: () => current,
        onRegistered: _shortcutCatalogRegistered,
        readActions: readShortcutActions,
        readSupportedActions: readSupportedShortcutActions,
        readApplications: readShortcutApplications);
    _features = FeatureCatalogCoordinator(
        read: readExtensions ?? extensionStatus,
        refresh: refreshFeatures ?? extensions_api.refreshFeatures,
        enable: enableExtension ??
            ((key, enabled) =>
                extensions_api.setExtensionEnabled(key: key, enabled: enabled)),
        approve: grantExtension ??
            ((key, token) =>
                extensions_api.approveExtension(key: key, token: token)),
        revoke: removeExtensionGrant ??
            ((key) => extensions_api.revokeExtension(key: key)),
        invalidateReads: _saves.invalidateReads,
        onStarting: _featureUpdateStarted,
        onFailure: (error) => _setLocalizedStatus(
            (labels) => labels.extensionsUpdateFailed('$error')),
        reconcileShortcuts: _reconcileFeatureShortcuts,
        onChanged: _notify);
    clipboard.addListener(_notify);
    appHistory.addListener(_notify);
    appHistory.onCatalogChanged = _features.reconcileSource;
  }

  final ClipboardSettingsController clipboard;
  final AppHistorySettingsController appHistory;
  final DesktopHost _window;
  late final SettingsSaveCoordinator _saves;
  late final ShortcutCatalogCoordinator _shortcuts;
  late final FeatureCatalogCoordinator _features;
  List<ExtensionStatusDto> get extensions => _features.statuses;
  String _shortcutFailureMessage(ShortcutCatalogFailure failure) => failure ==
          ShortcutCatalogFailure.registration
      ? 'Some action shortcuts are paused. Reopen Settings to try again, or choose a different key combination.'
      : 'Action shortcuts could not be refreshed. Reopen Settings to try again.';
  String? get _catalogSyncError => _shortcuts.failure == null
      ? null
      : _shortcutFailureMessage(_shortcuts.failure!);
  bool get updatingFeatures => _features.busy;
  bool shortcutSupported(String id) => _shortcuts.supports(id);
  List<ShortcutApplicationDto> get shortcutApplications =>
      _shortcuts.applications;
  List<ShortcutActionDto> get shortcutActions => _shortcuts.actions;
  HostSettings get current => _saves.current;
  bool get fileSearchNeedsRecovery => _saves.fileSearchNeedsRecovery;
  static const fileSearchRecoveryMessage =
      'File search is disabled because its saved settings could not be applied. Review Search settings.';
  String get warning => [
        _saves.warning,
        if (fileSearchNeedsRecovery) fileSearchRecoveryMessage,
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
          : _shortcuts.failure == ShortcutCatalogFailure.registration
              ? labels.shortcutsPausedHelp
              : labels.shortcutsRefreshFailed;
  String? _shortcutRestoreError;
  String? get statusMessage => _statusMessage;
  List<ContextCaptureDto> _captures = const [];
  Future<SettingsCloseChoice> Function(bool hasDrafts)? confirmClose;
  VoidCallback? revealHistoryFailure;
  bool _shortcutEditing = false;
  bool get closing => _closing != null;
  void setShortcutEditing(bool editing) {
    if (_shortcutEditing == editing) return;
    _shortcutEditing = editing;
    _notify();
  }

  SettingsEdits? _edits;
  SettingsEdits get edits => _edits ??= _createEdits();
  SettingsEdits _createEdits() => SettingsEdits(
      read: () => current,
      save: save,
      reportFailure: () =>
          _reportLocalizedError((labels) => labels.settingsSaveFailed))
    ..addListener(_notify);
  void openEditing() {
    if (!edits.hasDrafts && !edits.busy) edits.reset();
  }

  Future<bool> saveEdit(HostSettings Function(HostSettings) update,
      {bool excluded = false, bool fileSearch = false}) {
    if (closing || _disposed) return Future.value(false);
    return edits.apply(update, excludedApps: excluded, fileSearch: fileSearch);
  }

  Future<bool>? _closing;
  bool _disposed = false;

  void _notify() {
    if (!_disposed) notifyListeners();
  }

  Future<void> load({bool catalog = false}) async {
    final read = await _saves.load();
    if (_disposed || read == null) return;
    final epoch = read.revision;
    final previousWarning = [
      read.warning,
      if (read.recovery) fileSearchRecoveryMessage,
    ].where((s) => s.isNotEmpty).join('\n');
    if (warning.isNotEmpty || _statusMessage == previousWarning) {
      _statusMessage = warning.isEmpty ? null : warning;
    }
    if (current.actionShortcutVersion != 4) {
      _setLocalizedStatus((labels) => labels.shortcutsSavedByNewerVersion);
    }
    if (catalog) {
      try {
        await _features.load(() => epoch == _saves.loadRevision);
        if (_disposed || epoch != _saves.loadRevision) return;
      } catch (error) {
        if (_disposed || epoch != _saves.loadRevision) return;
        _setLocalizedStatus((labels) => labels.extensionsLoadFailed('$error'));
      }
    }
    if (catalog) {
      await _shortcuts.loadPickers(() => epoch == _saves.loadRevision);
    }
    if (_disposed || epoch != _saves.loadRevision) return;
    await _syncShortcutCatalog();
    _notify();
  }

  Future<void> _syncShortcutCatalog() async {
    await _shortcuts.synchronize();
    if (_shortcuts.failure != null) _notify();
  }

  Future<void> _shortcutCatalogRegistered(
      HostSettings snapshot, ShortcutCatalogFailure? previousFailure) async {
    final previousWarning = warning;
    bool isCurrent() => identical(snapshot, current);
    await _saves.refreshStatus(isCurrent: isCurrent);
    if (_disposed || !isCurrent()) return;
    if (warning.isNotEmpty ||
        _statusMessage == previousWarning ||
        (previousFailure != null &&
            _statusMessage == _shortcutFailureMessage(previousFailure))) {
      _statusMessage = warning.isEmpty ? null : warning;
    }
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
  Future<bool> prepareClose() => _closing ??= _prepareClose().whenComplete(() {
        _closing = null;
        _notify();
      });
  Future<bool> _prepareClose() async {
    _notify();
    if (_shortcutEditing) {
      _reportLocalizedError(
          (labels) => labels.finishOrCancelEditingTheActionShortcut);
      return false;
    }
    // Capture all queues before awaiting one: a failure may finish and clear its
    // queue while another write is still running. Each module retains its outcome.
    final results = await Future.wait([
      appHistory.waitForPending(),
      clipboard.waitForPending(),
      _edits?.waitForPending() ?? Future.value(true),
    ]);
    // A source write can start catalog reconciliation while these queues drain.
    await _features.waitForPending();
    if (_disposed) return false;
    if (!results[0]) {
      _reportLocalizedError((labels) => labels.appHistorySettingsFailed);
    } else if (!results[1]) {
      _reportLocalizedError((labels) =>
          clipboard.noticeFor(labels) ?? labels.clipboardSettingsFailed);
    }
    final dirty = (_edits?.hasDrafts ?? false) || appHistory.hasDrafts;
    if (!dirty && results.every((saved) => saved)) return true;
    final choice = await (confirmClose?.call(dirty) ??
        Future.value(SettingsCloseChoice.cancel));
    if (_disposed) return false;
    if (choice == SettingsCloseChoice.discard) {
      _edits?.reset();
      appHistory.discardDrafts();
      return true;
    }
    if (choice != SettingsCloseChoice.save) return false;
    final applied = !(_edits?.hasDrafts ?? false) || await edits.saveDrafts();
    final historyApplied = applied && await appHistory.saveDrafts();
    if (applied && !historyApplied) {
      appHistory.setFilter(appHistory.failedSource ?? '', onlyEnabled: false);
      revealHistoryFailure?.call();
    }
    if (historyApplied &&
        ((_edits?.hasDrafts ?? false) || appHistory.hasDrafts)) {
      _reportLocalizedError((labels) => labels.newEditsWereMadeWhileSavingSave);
      return false;
    }
    return historyApplied;
  }

  Future<bool> save(HostSettings next) => _saves.save(() => _save(next));

  Future<bool> _save(HostSettings next) async {
    if (_disposed || !_validateSettings(next)) return false;
    List<String>? supported;
    try {
      supported = await _shortcuts.supportedActionsFor(next);
    } catch (error) {
      if (_disposed) return false;
      await _syncShortcutCatalog();
      if (_disposed) return false;
      reportError(_shortcutFailureMessage(ShortcutCatalogFailure.from(error)));
      return false;
    }
    if (_disposed) return false;
    final failures = await _saves.apply(next, supportedActionIds: supported);
    if (_disposed || failures == null) return false;
    await _syncShortcutCatalog();
    if (_disposed) return false;
    if (_catalogSyncError != null) failures.add(_catalogSyncError!);
    _statusMessage = failures.isEmpty
        ? (warning.isEmpty ? null : warning)
        : failures.join('\n');
    _notify();
    return failures.isEmpty;
  }

  bool _validateSettings(HostSettings next) {
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
    return true;
  }

  Future<void> authorizeFileSearch() async {
    if (authorizingFiles || _disposed) return;
    final pending = _saves.authorize();
    _notify();
    try {
      final outcome = await pending;
      if (_disposed) return;
      switch (outcome) {
        case SettingsAuthorizationOutcome.applied:
          _statusMessage = warning.isEmpty ? null : warning;
        case SettingsAuthorizationOutcome.failed:
          _setLocalizedStatus((labels) => labels.directoryAccessFailed);
        case SettingsAuthorizationOutcome.superseded:
          break;
      }
    } finally {
      _notify();
    }
  }

  bool get authorizingFiles => _saves.authorizingFiles;

  Future<void> refreshFeatureData() => _changeFeatures(_features.refresh);

  Future<void> setExtensionEnabled(
          ExtensionStatusDto extension, bool enabled) =>
      _changeFeatures(() => _features.enable(extension, enabled));

  Future<void> approveExtension(ExtensionStatusDto extension) =>
      _changeFeatures(() => _features.approve(extension));

  Future<void> revokeExtension(ExtensionStatusDto extension) =>
      _changeFeatures(() => _features.revoke(extension));

  Future<void> _changeFeatures(Future<void> Function() change) {
    if (_disposed || closing) {
      return Future.value();
    }
    return change();
  }

  void _featureUpdateStarted() {
    _statusMessage = null;
    _notify();
  }

  Future<void> _reconcileFeatureShortcuts() async {
    try {
      await _shortcuts.refreshActions();
    } catch (error) {
      if (!_disposed && _statusMessage == null) {
        _setLocalizedStatus((labels) => labels.shortcutLoadFailed('$error'));
      }
    }
    await _syncShortcutCatalog();
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
    _saves.dispose();
    _shortcuts.dispose();
    _features.dispose();
    _edits?.removeListener(_notify);
    _edits?.dispose();
    clipboard.removeListener(_notify);
    clipboard.dispose();
    appHistory.removeListener(_notify);
    appHistory.dispose();
    super.dispose();
  }
}
