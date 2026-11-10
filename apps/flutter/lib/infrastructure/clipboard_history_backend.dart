import 'bridge_api.dart' as api;

abstract interface class ClipboardHistoryBackend {
  Future<api.ClipboardHistoryDto?> search(String query, {String? after});
  Future<api.NativePreviewDto> preparePreview(String id);
  Future<void> copy(String id);
  Future<void> delete(String id);
}

class RustClipboardHistoryBackend implements ClipboardHistoryBackend {
  @override
  Future<api.ClipboardHistoryDto?> search(String query, {String? after}) =>
      api.clipboardHistory(query: query, after: after);
  @override
  Future<api.NativePreviewDto> preparePreview(String id) =>
      api.previewClipboardEntry(id: id);
  @override
  Future<void> copy(String id) => api.copyClipboardEntry(id: id);
  @override
  Future<void> delete(String id) => api.deleteClipboardEntry(id: id);
}
