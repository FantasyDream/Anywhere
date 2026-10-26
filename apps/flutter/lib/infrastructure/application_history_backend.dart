import '../src/rust/api.dart' as api;

abstract interface class AppHistoryQueryBackend {
  Future<api.AppHistoryDto> search(String query, String source);
  Future<void> useItem(String id, {required bool copy});
  Future<List<api.CandidateDto>> actions(String id,
      {required int sessionId, required int revision});
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
  Future<int> _session() async {
    final lease = _lease ??= api.openAppHistorySession();
    try {
      return await lease;
    } catch (_) {
      // An explicit refresh may retry a transient failure. Do not cache the
      // failed Future for the rest of this page's lifetime.
      if (identical(_lease, lease)) _lease = null;
      rethrow;
    }
  }

  @override
  Future<api.AppHistoryDto> search(String query, String source) async {
    if (_closed) throw StateError('History page closed');
    final session = await _session();
    if (_closed) throw StateError('History page closed');
    // A stale result may belong to a replaced page. Reopening here would steal
    // the newer page's lease before its presentation can reject this response.
    return api.appHistorySearch(
        sessionId: session, query: query, source: source);
  }

  @override
  Future<void> useItem(String id, {required bool copy}) =>
      api.useAppHistoryItem(id: id, copy: copy);
  @override
  Future<List<api.CandidateDto>> actions(String id,
      {required int sessionId, required int revision}) async {
    if (_closed) throw StateError('History page closed');
    return api.appHistoryObjectActions(
        sessionId: sessionId, revision: revision, id: id);
  }

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
