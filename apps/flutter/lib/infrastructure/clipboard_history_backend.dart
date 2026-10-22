import '../src/rust/api.dart' as api;

abstract interface class ClipboardHistoryBackend {
  Future<api.ClipboardHistoryDto> search(String query);
  Future<api.ClipboardDetailDto?> detail(String id);
  Future<void> copy(String id);
  Future<void> delete(String id);
}

class RustClipboardHistoryBackend implements ClipboardHistoryBackend {
  @override
  Future<api.ClipboardHistoryDto> search(String query) =>
      api.clipboardHistory(query: query);
  @override
  Future<api.ClipboardDetailDto?> detail(String id) =>
      api.clipboardHistoryDetail(id: id);
  @override
  Future<void> copy(String id) => api.copyClipboardEntry(id: id);
  @override
  Future<void> delete(String id) => api.deleteClipboardEntry(id: id);
}
