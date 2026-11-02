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
    expect(samples.single.keys,
        unorderedEquals(['operation', 'elapsed_us', 'rows']));
    expect(samples.single['operation'], 'activation');
    expect(samples.single['rows'], 4);
    expect(metrics.pending, isFalse);
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
