import 'package:flutter/painting.dart';

/// Opt-in local measurements. No query, context, path or clipboard body is kept.
enum PresentationOperation {
  activation,
  query,
  fileQuery,
  filePageNext,
  filePagePrevious,
  clipboardQuery,
  shelfQuery
}

class PresentationMetrics {
  static void Function(Map<String, Object>)? reporter;

  PresentationMetrics(
      {void Function(Map<String, Object>)? report,
      Map<String, Object> Function()? details})
      : _report = report ?? reporter,
        _details = details;
  final void Function(Map<String, Object>)? _report;
  final Map<String, Object> Function()? _details;
  _Measurement? _pending;
  _Measurement? _scheduled;
  bool get pending => _pending != null;

  void begin(PresentationOperation operation,
      {required int generation, int? revision, required int presentation}) {
    if (_report == null) return;
    _pending = _Measurement(operation, generation, revision, presentation);
    _scheduled = null;
  }

  void cancel() {
    _pending = _scheduled = null;
  }

  void ready({
    required int generation,
    required int revision,
    required int presentation,
    required int rows,
    required void Function(void Function()) afterFrame,
    required bool Function() current,
  }) {
    final sample = _pending;
    if (sample == null ||
        identical(sample, _scheduled) ||
        generation != sample.generation ||
        presentation < sample.presentation ||
        (sample.revision != null && revision != sample.revision)) {
      return;
    }
    _scheduled = sample;
    afterFrame(() {
      if (!identical(_pending, sample)) return;
      _scheduled = null;
      if (!current()) return;
      _pending = null;
      final operation = switch (sample.operation) {
        PresentationOperation.activation => 'activation',
        PresentationOperation.query => 'query',
        PresentationOperation.fileQuery => 'file_query',
        PresentationOperation.filePageNext => 'file_page_next',
        PresentationOperation.filePagePrevious => 'file_page_previous',
        PresentationOperation.clipboardQuery => 'clipboard_query',
        PresentationOperation.shelfQuery => 'shelf_query',
      };
      // Diagnostics cannot affect the normal interaction if a host is unavailable.
      try {
        final counts = <String, Object>{
          ..._runtimeDiagnostics(),
          ...?_details?.call(),
          ...PresentationTransferStats.snapshot(),
        };
        _report?.call({
          'operation': operation,
          'elapsed_us': sample.watch.elapsedMicroseconds,
          'rows': rows,
          ...counts,
        });
      } catch (_) {}
    });
  }
}

/// Bounded counters updated only at transport boundaries. They contain sizes,
/// never candidates, paths, clipboard text or document snapshots.
abstract final class PresentationTransferStats {
  static int _batchCount = 0;
  static int _rowsTotal = 0;
  static int _lastRows = 0;
  static int _lastKind = 0;

  // 0 none, 1 replacement, 2 retained list, 3 append tail.
  static void candidate(int kind, int rows) {
    if (PresentationMetrics.reporter == null) return;
    _batchCount++;
    _rowsTotal += rows;
    _lastRows = rows;
    _lastKind = kind;
  }

  static Map<String, Object> snapshot() => {
        'candidate_transfer_batches': _batchCount,
        'candidate_transfer_rows_total': _rowsTotal,
        'candidate_transfer_rows_last': _lastRows,
        'candidate_transfer_kind_last': _lastKind,
      };

  static void reset() {
    _batchCount = 0;
    _rowsTotal = 0;
    _lastRows = 0;
    _lastKind = 0;
  }
}

/// One Flutter engine has one isolate, so these counters need no lock. Every
/// registered document is paired with an end call on replacement or disposal.
abstract final class PresentationDocumentStats {
  static int _activeWebDocuments = 0;

  static void registerWebDocument() {
    _activeWebDocuments++;
  }

  static void releaseWebDocument() {
    if (_activeWebDocuments > 0) _activeWebDocuments--;
  }

  static int get activeWebDocuments => _activeWebDocuments;

  static void reset() {
    _activeWebDocuments = 0;
  }
}

Map<String, Object> _runtimeDiagnostics() {
  ImageCache? cache;
  try {
    cache = PaintingBinding.instance.imageCache;
  } catch (_) {
    // Pure model tests may not install a widget binding. Normal app engines do.
  }
  return {
    'flutter_engine_count': 1,
    'image_cache_entries': cache?.currentSize ?? 0,
    'image_cache_bytes': cache?.currentSizeBytes ?? 0,
    'image_cache_live': cache?.liveImageCount ?? 0,
    'image_cache_pending': cache?.pendingImageCount ?? 0,
    'web_document_count': PresentationDocumentStats.activeWebDocuments,
  };
}

class _Measurement {
  _Measurement(
      this.operation, this.generation, this.revision, this.presentation);
  final PresentationOperation operation;
  final int generation, presentation;
  final int? revision;
  final watch = Stopwatch()..start();
}
