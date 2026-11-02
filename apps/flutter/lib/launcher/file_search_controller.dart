import 'package:flutter/foundation.dart';

import '../src/rust/api.dart';
import 'file_filter.dart';
export 'file_filter.dart';

typedef LoadFilePreview = Future<NativePreviewDto> Function(
    int session, int revision, String candidate);

/// File filtering only; content preparation is explicit through the preview port.
class FileSearchController extends ChangeNotifier {
  FileSearchController(this.loadPreview);
  final LoadFilePreview? loadPreview;
  FileFilter filter = FileFilter.all;
  void setFilter(FileFilter value) {
    if (filter == value) return;
    filter = value;
    notifyListeners();
  }

  void cycleFilter(int direction) => setFilter(FileFilter.values[
      (filter.index + direction + FileFilter.values.length) %
          FileFilter.values.length]);
  void reset() {
    filter = FileFilter.all;
  }
}
