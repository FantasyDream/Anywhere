import '../src/rust/api.dart' as api;

abstract interface class AppHistoryQueryBackend {
  Future<api.AppHistoryDto> search(String query, String source);
  Future<void> useItem(String id, {required bool copy});
  Future<void> cancel();
  Future<void> close();
}

abstract interface class AppHistoryPreferencesBackend {
  Future<List<api.AppHistorySourceDto>> read();
  Future<void> configure(String id, String path,
      {api.AppHistorySourceModeDto mode = api.AppHistorySourceModeDto.custom});
  Future<void> setActionEnabled(String id, bool enabled);
  Future<String> customDirectory();
  Future<void> openCustomDirectory();
}

class RustAppHistoryPreferencesBackend implements AppHistoryPreferencesBackend {
  @override
  Future<void> setActionEnabled(String id, bool enabled) =>
      api.setAppHistoryActionEnabled(id: id, enabled: enabled);
  @override
  Future<String> customDirectory() => api.appHistoryCustomDirectory();
  @override
  Future<void> openCustomDirectory() => api.openAppHistoryCustomDirectory();
  @override
  Future<List<api.AppHistorySourceDto>> read() => api.appHistorySources();
  @override
  Future<void> configure(String id, String path,
          {api.AppHistorySourceModeDto mode =
              api.AppHistorySourceModeDto.custom}) =>
      api.configureAppHistorySource(id: id, path: path, mode: mode);
}

/// Each page owns its lease. A late close cannot cancel a newly opened page.
class RustAppHistoryQueryBackend implements AppHistoryQueryBackend {
  Future<int>? _lease;
  bool _closed = false;
  Future<int> _session() => _lease ??= api.openAppHistorySession();
  @override
  Future<api.AppHistoryDto> search(String query, String source) async {
    if (_closed) throw StateError('History page closed');
    var result = await api.appHistorySearch(
        sessionId: await _session(), query: query, source: source);
    if (!_closed && result.notice == api.AppHistoryNoticeDto.stale) {
      _lease = null;
      result = await api.appHistorySearch(
          sessionId: await _session(), query: query, source: source);
    }
    return result;
  }

  @override
  Future<void> useItem(String id, {required bool copy}) =>
      api.useAppHistoryItem(id: id, copy: copy);
  @override
  Future<void> cancel() async {
    final lease = _lease;
    if (lease != null) await api.cancelAppHistory(sessionId: await lease);
  }

  @override
  Future<void> close() async {
    _closed = true;
    await cancel();
  }
}
