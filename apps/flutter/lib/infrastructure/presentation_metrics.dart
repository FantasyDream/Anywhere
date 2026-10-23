/// Opt-in local measurements. No query, context, path or clipboard body is kept.
enum PresentationOperation { activation, query, fileQuery }

class PresentationMetrics {
  static void Function(Map<String, Object>)? reporter;

  PresentationMetrics({void Function(Map<String, Object>)? report})
      : _report = report ?? reporter;
  final void Function(Map<String, Object>)? _report;
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
      };
      // Diagnostics cannot affect the normal interaction if a host is unavailable.
      try {
        _report?.call({
          'operation': operation,
          'elapsed_us': sample.watch.elapsedMicroseconds,
          'rows': rows
        });
      } catch (_) {}
    });
  }
}

class _Measurement {
  _Measurement(
      this.operation, this.generation, this.revision, this.presentation);
  final PresentationOperation operation;
  final int generation, presentation;
  final int? revision;
  final watch = Stopwatch()..start();
}
