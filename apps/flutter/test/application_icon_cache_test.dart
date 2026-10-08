import 'dart:async';
import 'dart:typed_data';
import 'package:flutter_test/flutter_test.dart';
import 'package:anywhere/infrastructure/application_icon_cache.dart';
import 'package:anywhere/src/rust/api.dart';

CandidateDto app(String id) => CandidateDto(
    id: id,
    groupId: 'app',
    title: id,
    kind: CandidateKindDto.application,
    score: 0,
    availability: AvailabilityDto.enabled,
    contextKeys: const [],
    homeEligible: true,
    primaryAction: false,
    menuPath: const [],
    iconKey: 'application');

void main() {
  test('byte budget holds even when all requested icons are pinned', () async {
    final cache = ApplicationIconCache((_) async => Uint8List(4),
        byteBudget: 6, maxEntryBytes: 4);
    cache.request([app('a'), app('b')]);
    await Future<void>.delayed(Duration.zero);
    expect(cache.entryCount, 1);
    expect(cache.retainedBytes, 4);
    expect(cache.bytesFor('a'), isNull);
    expect(cache.bytesFor('b'), hasLength(4));
    cache.clear();
    expect(cache.entryCount, 0);
    expect(cache.retainedBytes, 0);
    cache.dispose();
  });

  test('oversized icon is rejected without repeated loads until retry',
      () async {
    var calls = 0;
    final cache = ApplicationIconCache((_) async {
      calls++;
      return Uint8List(5);
    }, byteBudget: 8, maxEntryBytes: 4);
    cache.request([app('large')]);
    await Future<void>.delayed(Duration.zero);
    cache.request([app('large')]);
    expect(calls, 1);
    expect(cache.retainedBytes, 0);
    expect(cache.bytesFor('large'), isNull);
    cache.retryFailures();
    cache.request([app('large')]);
    await Future<void>.delayed(Duration.zero);
    expect(calls, 2);
    cache.dispose();
  });

  test(
      'queued icons outside the viewport never start and concurrency stays bounded',
      () async {
    final reads = <String, Completer<Uint8List?>>{};
    final cache = ApplicationIconCache((id) {
      final pending = Completer<Uint8List?>();
      reads[id] = pending;
      return pending.future;
    });
    cache.request(List.generate(12, (i) => app('$i')));
    expect(reads.length, ApplicationIconCache.maxInFlight);
    cache.request([app('new')]);
    for (final pending in reads.values.toList()) {
      pending.complete(Uint8List(1));
    }
    await Future<void>.delayed(Duration.zero);
    expect(reads.keys, ['0', '1', '2', '3', 'new']);
    reads['new']!.complete(Uint8List(1));
    await Future<void>.delayed(Duration.zero);
    cache.dispose();
  });
}
