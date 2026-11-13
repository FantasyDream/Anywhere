import 'package:anywhere/infrastructure/presentation_metrics.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  test('disabled measurements do not allocate pending work', () {
    final metrics = PresentationMetrics();
    metrics.begin(PresentationOperation.query,
        generation: 1, revision: 1, presentation: 0);
    expect(metrics.pending, isFalse);
  });

  test(
      'activation waits for the acknowledged window presentation and one completed frame',
      () {
    final samples = <Map<String, Object>>[];
    final frames = <void Function()>[];
    final metrics = PresentationMetrics(report: samples.add);
    metrics.begin(PresentationOperation.activation,
        generation: 3, presentation: 2);
    void ready(int presentation) => metrics.ready(
        generation: 3,
        revision: 1,
        presentation: presentation,
        rows: 4,
        afterFrame: frames.add,
        current: () => true);
    ready(1);
    expect(frames, isEmpty);
    ready(2);
    ready(2);
    expect(frames, hasLength(1));
    expect(samples, isEmpty);
    frames.single();
    expect(
        samples.single.keys,
        containsAll([
          'operation',
          'elapsed_us',
          'rows',
          'flutter_engine_count',
          'image_cache_entries',
          'image_cache_bytes',
          'image_cache_live',
          'image_cache_pending',
          'candidate_transfer_batches',
        ]));
    expect(samples.single['operation'], 'activation');
    expect(samples.single['rows'], 4);
    expect(samples.single['flutter_engine_count'], 1);
    expect(metrics.pending, isFalse);
  });

  test('scenario details and transfer boundaries remain numeric and bounded',
      () {
    final samples = <Map<String, Object>>[];
    PresentationMetrics.reporter = (_) {};
    addTearDown(() {
      PresentationMetrics.reporter = null;
      PresentationTransferStats.reset();
      PresentationDocumentStats.reset();
    });
    PresentationTransferStats.candidate(3, 64);
    final metrics = PresentationMetrics(
        report: samples.add,
        details: () => {'candidate_count': 10000, 'candidate_chunk_count': 41});
    metrics.begin(PresentationOperation.fileQuery,
        generation: 1, revision: 2, presentation: 1);
    metrics.ready(
        generation: 1,
        revision: 2,
        presentation: 1,
        rows: 7,
        afterFrame: (callback) => callback(),
        current: () => true);

    final sample = samples.single;
    expect(sample['candidate_count'], 10000);
    expect(sample['candidate_chunk_count'], 41);
    expect(sample['candidate_transfer_batches'], 1);
    expect(sample['candidate_transfer_rows_total'], 64);
    expect(sample['candidate_transfer_rows_last'], 64);
    expect(sample['candidate_transfer_kind_last'], 3);
    expect(sample.values.whereType<String>(), ['file_query']);
  });

  test('file page directions use distinct diagnostics operations', () {
    final samples = <Map<String, Object>>[];
    final metrics = PresentationMetrics(report: samples.add);

    for (final (operation, label) in [
      (PresentationOperation.filePageNext, 'file_page_next'),
      (PresentationOperation.filePagePrevious, 'file_page_previous'),
    ]) {
      metrics.begin(operation, generation: 1, revision: 2, presentation: 1);
      metrics.ready(
          generation: 1,
          revision: 2,
          presentation: 1,
          rows: 64,
          afterFrame: (callback) => callback(),
          current: () => true);
      expect(samples.last['operation'], label);
      expect(samples.last['rows'], 64);
    }
  });

  test('document counters are balanced and reported as numbers only', () {
    final samples = <Map<String, Object>>[];
    PresentationDocumentStats.reset();
    addTearDown(PresentationDocumentStats.reset);
    PresentationDocumentStats.registerWebDocument();
    final metrics = PresentationMetrics(report: samples.add);
    metrics.begin(PresentationOperation.query,
        generation: 1, revision: 1, presentation: 1);
    metrics.ready(
        generation: 1,
        revision: 1,
        presentation: 1,
        rows: 0,
        afterFrame: (callback) => callback(),
        current: () => true);
    expect(samples.single['web_document_count'], 1);
    PresentationDocumentStats.releaseWebDocument();
    PresentationDocumentStats.releaseWebDocument();
    expect(PresentationDocumentStats.activeWebDocuments, 0);
  });

  test('superseded revisions and hidden frames cannot publish a latency sample',
      () {
    final samples = <Map<String, Object>>[];
    final frames = <void Function()>[];
    final metrics = PresentationMetrics(report: samples.add);
    metrics.begin(PresentationOperation.query,
        generation: 1, revision: 2, presentation: 1);
    metrics.ready(
        generation: 1,
        revision: 1,
        presentation: 1,
        rows: 1,
        afterFrame: frames.add,
        current: () => true);
    expect(frames, isEmpty);
    metrics.ready(
        generation: 1,
        revision: 2,
        presentation: 1,
        rows: 1,
        afterFrame: frames.add,
        current: () => true);
    metrics.begin(PresentationOperation.fileQuery,
        generation: 1, revision: 3, presentation: 1);
    frames.removeLast()();
    expect(samples, isEmpty);
    metrics.ready(
        generation: 1,
        revision: 3,
        presentation: 1,
        rows: 1,
        afterFrame: frames.add,
        current: () => false);
    frames.removeLast()();
    expect(samples, isEmpty);
    metrics.ready(
        generation: 1,
        revision: 3,
        presentation: 1,
        rows: 1,
        afterFrame: frames.add,
        current: () => true);
    metrics.cancel();
    frames.removeLast()();
    expect(samples, isEmpty);
  });

  test('diagnostic transport errors do not interrupt an interaction', () {
    final metrics =
        PresentationMetrics(report: (_) => throw StateError('offline'));
    metrics.begin(PresentationOperation.query,
        generation: 1, revision: 1, presentation: 1);
    metrics.ready(
        generation: 1,
        revision: 1,
        presentation: 1,
        rows: 0,
        afterFrame: (callback) => callback(),
        current: () => true);
    expect(metrics.pending, isFalse);
  });
}
