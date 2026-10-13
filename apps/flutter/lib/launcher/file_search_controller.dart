import '../infrastructure/runtime_policy.dart';
import 'dart:async';
import 'package:flutter/foundation.dart';
import '../src/rust/api.dart';

import 'file_filter.dart';
export 'file_filter.dart';

typedef LoadFilePreview = Future<FilePreviewDto> Function(
    int session, int revision, String candidate);

/// Presentation filters and a debounced, latest-selection-only preview.
class FileSearchController extends ChangeNotifier {
  FileSearchController(this._load);
  final LoadFilePreview? _load;
  FileFilter filter = FileFilter.all;
  bool loading = false;
  FilePreviewDto? preview;
  String? error;
  Timer? _timer;
  Record? _selection;
  int _request = 0;
  bool _disposed = false;

  void setFilter(FileFilter value) {
    if (filter == value) return;
    filter = value;
    notifyListeners();
  }

  void cycleFilter(int direction) {
    setFilter(FileFilter.values[
        (filter.index + direction + FileFilter.values.length) %
            FileFilter.values.length]);
  }

  // Called before Launcher notification. Synchronous changes are included in that
  // notification; only asynchronous completion needs another one.
  void sync(int? session, int revision, CandidateDto? file) {
    final selection =
        session != null && file != null ? (session, revision, file.id) : null;
    if (selection == _selection) return;
    _selection = selection;
    final request = ++_request;
    _timer?.cancel();
    preview = null;
    error = null;
    loading = selection != null;
    if (selection == null) return;
    if (_load == null) {
      loading = false;
      error = 'Preview is unavailable';
      return;
    }
    _timer =
        Timer(const Duration(milliseconds: RuntimePolicy.filePreviewDebounceMs),
            () async {
      try {
        final value = await _load(session!, revision, file!.id);
        if (_disposed || request != _request) return;
        preview = value;
      } catch (_) {
        if (_disposed || request != _request) return;
        error = 'Preview unavailable. Search again if the file has changed.';
      }
      loading = false;
      notifyListeners();
    });
  }

  void reset() {
    _timer?.cancel();
    ++_request;
    _selection = null;
    preview = null;
    error = null;
    loading = false;
    filter = FileFilter.all;
  }

  @override
  void dispose() {
    _disposed = true;
    reset();
    super.dispose();
  }
}
