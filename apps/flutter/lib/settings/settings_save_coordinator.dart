import '../infrastructure/file_search_access.dart';
import '../infrastructure/window_host.dart';
import 'host_settings.dart';

enum SettingsAuthorizationOutcome { applied, failed, superseded }

/// Owns authoritative Host readbacks, serialized saves and file authorization.
/// Drafts, shortcut eligibility, close decisions and user wording stay outside.
class SettingsSaveCoordinator {
  SettingsSaveCoordinator({
    required DesktopHost window,
    required Future<void> Function(List<String>) applyExcludedApps,
    required Future<void> Function(HostSettings) applyContextSources,
    required Future<void> Function(HostSettings) applyFileSearch,
    required Future<void> Function(HostSettings) authorizeFileSearch,
    String startupMessage = '',
    bool fileSearchInitializationFailed = false,
  })  : _window = window,
        _applyExcludedApps = applyExcludedApps,
        _applyContextSources = applyContextSources,
        _applyFileSearch = applyFileSearch,
        _authorizeFileSearch = authorizeFileSearch,
        _warning = startupMessage,
        _fileSearchNeedsRecovery = fileSearchInitializationFailed;

  final DesktopHost _window;
  final Future<void> Function(List<String>) _applyExcludedApps;
  final Future<void> Function(HostSettings) _applyContextSources;
  final Future<void> Function(HostSettings) _applyFileSearch;
  final Future<void> Function(HostSettings) _authorizeFileSearch;
  HostSettings _current = HostSettings.defaults;
  HostSettings get current => _current;
  String _warning;
  String get warning => _warning;
  bool _fileSearchNeedsRecovery;
  bool get fileSearchNeedsRecovery => _fileSearchNeedsRecovery;
  int _loadRevision = 0, _configuration = 0;
  int get loadRevision => _loadRevision;
  void invalidateReads() => ++_loadRevision;
  Future<void> _pending = Future.value();
  bool _disposed = false, _authorizingFiles = false;
  bool get authorizingFiles => _authorizingFiles;

  /// Returns the read's revision only after publishing both authoritative values.
  Future<({int revision, String warning, bool recovery})?> load() async {
    final revision = ++_loadRevision;
    final next = await _window.readSettings();
    if (_disposed || revision != _loadRevision) return null;
    final previous = (
      revision: revision,
      warning: _warning,
      recovery: _fileSearchNeedsRecovery
    );
    var warning = _warning;
    try {
      warning = await _window.readSettingsStatus();
    } catch (_) {
      /* Keep the last known Host status if only status reading fails. */
    }
    if (_disposed || revision != _loadRevision) return null;
    _current = next;
    _warning = warning;
    if (next.fileSearchError != null) _fileSearchNeedsRecovery = true;
    return previous;
  }

  /// Validation and catalog reconciliation are part of the same queued save.
  Future<bool> save(Future<bool> Function() operation) {
    ++_configuration;
    ++_loadRevision;
    final result = _pending.then<bool>((_) => _disposed ? false : operation());
    _pending = result.then<void>((_) {}, onError: (Object _) {});
    return result;
  }

  /// Called inside save's serialized operation after shortcut validation.
  /// A null result means disposal revoked publication, not a successful save.
  Future<List<String>?> apply(HostSettings next,
      {List<String>? supportedActionIds}) async {
    if (_disposed) return null;
    final previous = _current;
    final error = await _window.writeSettings(next,
        supportedActionIds: supportedActionIds);
    if (_disposed) return null;
    try {
      final readback = await _window.readSettings();
      if (_disposed) return null;
      _current = readback;
    } catch (_) {
      // Rejected native bindings must never become the published snapshot.
      _current = error == null ? next : previous;
    }
    if (_disposed) return null;
    final failures = <String>[if (error != null && error.isNotEmpty) error];
    try {
      await _applyExcludedApps(current.excludedApps);
    } catch (error) {
      failures.add('$error');
    }
    if (_disposed) return null;
    try {
      await _applyContextSources(current);
    } catch (error) {
      failures.add('$error');
    }
    if (_disposed) return null;
    try {
      await _applyFileSearch(current);
      if (_disposed) return null;
      final readback = await _window.readSettings();
      if (_disposed) return null;
      _current = readback;
      _fileSearchNeedsRecovery = false;
    } catch (_) {
      _fileSearchNeedsRecovery = true;
      failures
          .add('File search settings could not be applied. Try saving again.');
    }
    if (_disposed) return null;
    try {
      await refreshStatus();
    } catch (_) {/* A failed status read does not undo acknowledged writes. */}
    return _disposed ? null : failures;
  }

  Future<void> refreshStatus({bool Function()? isCurrent}) async {
    final warning = await _window.readSettingsStatus();
    if (!_disposed && (isCurrent?.call() ?? true)) _warning = warning;
  }

  Future<SettingsAuthorizationOutcome> authorize() async {
    if (_authorizingFiles || _disposed) {
      return SettingsAuthorizationOutcome.superseded;
    }
    _authorizingFiles = true;
    final configuration = _configuration;
    bool currentRequest() => !_disposed && configuration == _configuration;
    try {
      await _pending;
      if (!currentRequest()) return SettingsAuthorizationOutcome.superseded;
      await _authorizeFileSearch(current);
      if (!currentRequest()) return SettingsAuthorizationOutcome.superseded;
      final next = await _window.readSettings();
      if (!currentRequest()) return SettingsAuthorizationOutcome.superseded;
      final warning = await _window.readSettingsStatus();
      if (!currentRequest()) return SettingsAuthorizationOutcome.superseded;
      _current = next;
      _fileSearchNeedsRecovery = false;
      _warning = warning;
      return SettingsAuthorizationOutcome.applied;
    } on SearchAccessSuperseded {
      return SettingsAuthorizationOutcome.superseded;
    } catch (_) {
      if (!currentRequest()) return SettingsAuthorizationOutcome.superseded;
      _fileSearchNeedsRecovery = true;
      return SettingsAuthorizationOutcome.failed;
    } finally {
      _authorizingFiles = false;
    }
  }

  void dispose() {
    _disposed = true;
  }
}
