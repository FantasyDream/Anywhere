import '../src/rust/api.dart' as api;

/// Settings reads only status. Clipboard bodies and the history list stay out of
/// preferences, and the usage pages receive no recording or clear operations.
abstract interface class ClipboardPreferencesBackend {
  Future<api.ClipboardHistoryStatusDto> read();
  Future<void> setRecording(bool enabled);
  Future<void> pause(int minutes);
  Future<void> setRetention(int days);
  Future<void> clear();
}

class RustClipboardPreferencesBackend implements ClipboardPreferencesBackend {
  @override
  Future<api.ClipboardHistoryStatusDto> read() => api.clipboardHistoryStatus();
  @override
  Future<void> setRecording(bool enabled) =>
      api.setClipboardRecording(enabled: enabled);
  @override
  Future<void> pause(int minutes) =>
      api.pauseClipboardRecording(minutes: minutes);
  @override
  Future<void> setRetention(int days) => api.setClipboardRetention(days: days);
  @override
  Future<void> clear() => api.clearClipboardHistory();
}
