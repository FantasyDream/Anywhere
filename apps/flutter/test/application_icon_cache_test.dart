import 'dart:async';
import 'dart:typed_data';

import 'package:anywhere/infrastructure/application_icon_cache.dart';
import 'package:anywhere/src/rust/api.dart';
import 'package:flutter_test/flutter_test.dart';

CandidateDto app(String id) => CandidateDto(
    id: id,
    groupId: 'app',
    title: id,
    kind: CandidateKindDto.application,
    score: 0,
    availability: AvailabilityDto.enabled,
    contextKeys: const [],
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
  test('physical concurrency stays bounded across repeated clear and dispose',
      () async {
    var active = 0, peak = 0;
    final reads = <Completer<Uint8List?>>[];
    final cache = ApplicationIconCache((_) async {
      ++active;
      if (active > peak) peak = active;
      final result = Completer<Uint8List?>();
      reads.add(result);
      try {
        return await result.future;
      } finally {
        --active;
      }
    });
    cache.request(List.generate(4, (i) => app('old$i')));
    for (var round = 0; round < 5; round++) {
      cache.clear();
      cache.request(List.generate(4, (i) => app('new$round-$i')));
    }
    expect(peak, ApplicationIconCache.maxInFlight);
    for (final read in reads.toList()) {
      read.complete(Uint8List(1));
    }
    await Future<void>.delayed(Duration.zero);
    expect(reads.length, 8);
    cache.dispose();
    for (final read in reads.skip(4)) {
      read.complete(null);
    }
    await Future<void>.delayed(Duration.zero);
    expect(active, 0);
    expect(peak, ApplicationIconCache.maxInFlight);
  });
}
