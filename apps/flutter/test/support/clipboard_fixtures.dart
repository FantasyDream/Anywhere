import 'package:anywhere/infrastructure/clipboard_history_backend.dart';
import 'package:anywhere/src/rust/api.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

ClipboardItemDto item(String id) => ClipboardItemDto(
    id: id,
    summary: 'Entry $id',
    kind: 'text',
    copiedAtMs: 1000,
    byteCount: 20);

NativePreviewDto previewDetail(ClipboardItemDto item, String text) =>
    NativePreviewDto(
        id: 'preview-${item.id}',
        paths: ['/synthetic/${item.id}.txt'],
        title: text);

final previewCalls = <MethodCall>[];
void mockNativePreview() {
  previewCalls.clear();
  TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
      .setMockMethodCallHandler(const MethodChannel('anywhere/preview'),
          (call) async {
    previewCalls.add(call);
    return call.method == 'capabilities' ? {'supported': true} : null;
  });
}

class History implements ClipboardHistoryBackend {
  List<ClipboardItemDto> rows = [item('1'), item('2')];
  bool enabled = false;
  final copied = <String>[];
  Future<ClipboardHistoryDto> Function(String)? searchOverride;
  Future<NativePreviewDto> Function(String)? detailOverride;
  @override
  Future<ClipboardHistoryDto> search(String query) async => searchOverride !=
          null
      ? searchOverride!(query)
      : ClipboardHistoryDto(recording: enabled, available: true, items: rows);
  @override
  Future<NativePreviewDto> preparePreview(String id) async =>
      detailOverride != null
          ? detailOverride!(id)
          : previewDetail(item(id), 'Full text $id');
  @override
  Future<void> copy(String id) async {
    copied.add(id);
  }

  @override
  Future<void> delete(String id) async {
    rows = rows.where((row) => row.id != id).toList();
  }
}
