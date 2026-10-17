import 'dart:typed_data';
import 'package:anywhere/src/rust/api.dart';
import 'package:anywhere/infrastructure/clipboard_history_backend.dart';

ClipboardItemDto item(String id) => ClipboardItemDto(
    id: id,
    summary: 'Entry $id',
    kind: 'text',
    copiedAtMs: 1000,
    byteCount: 20);

ClipboardDetailDto previewDetail(ClipboardItemDto item, String text) =>
    ClipboardDetailDto(
        item: item,
        text: text,
        imagePng: Uint8List(0),
        files: const [],
        formats: const ['text/plain;charset=utf-8']);

class History implements ClipboardHistoryBackend {
  List<ClipboardItemDto> rows = [item('1'), item('2')];
  bool enabled = false;
  final copied = <String>[];
  Future<ClipboardHistoryDto> Function(String)? searchOverride;
  Future<ClipboardDetailDto?> Function(String)? detailOverride;
  @override
  Future<ClipboardHistoryDto> search(String query) async => searchOverride !=
          null
      ? searchOverride!(query)
      : ClipboardHistoryDto(recording: enabled, available: true, items: rows);
  @override
  Future<ClipboardDetailDto?> detail(String id) async => detailOverride != null
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
