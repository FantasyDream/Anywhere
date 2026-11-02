import 'dart:async';

import 'package:anywhere/infrastructure/application_history_backend.dart';
import 'package:anywhere/src/rust/api.dart';
import 'package:anywhere/src/rust/frb_generated.dart';
import 'package:flutter_test/flutter_test.dart';

class HistoryApi implements RustLibApi {
  int opens = 0, searches = 0;
  bool failOpen = false;
  Completer<int>? opening;
  final cancelled = <int>[];
  var pending = Completer<AppHistoryDto>();
  void reset() {
    opens = searches = 0;
    failOpen = false;
    opening = null;
    cancelled.clear();
    pending = Completer<AppHistoryDto>();
  }

  @override
  Future<int> crateApiOpenAppHistorySession() async {
    ++opens;
    if (failOpen) throw StateError('synthetic storage failure');
    return opening == null ? opens : await opening!.future;
  }

  @override
  Future<AppHistoryDto> crateApiAppHistorySearch(
      {required int sessionId, required String query, required String source}) {
    ++searches;
    return pending.future;
  }

  @override
  Future<void> crateApiCancelAppHistory({required int sessionId}) async {
    cancelled.add(sessionId);
  }

  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}

void main() {
  final api = HistoryApi();
  setUpAll(() => RustLib.initMock(api: api));
  tearDownAll(RustLib.dispose);
  setUp(api.reset);
  test('an explicit retry can recover from a failed lease open', () async {
    api.failOpen = true;
    final backend = RustAppHistoryQueryBackend();
    await expectLater(backend.search('', 'idea'), throwsStateError);
    api.failOpen = false;
    api.pending
        .complete(AppHistoryDto(items: const [], failedSources: const []));
    await backend.search('', 'idea');
    expect(api.opens, 2);
    expect(api.searches, 1);
    await backend.close();
  });

  test('closing during lease open never starts a source read', () async {
    api.opening = Completer<int>();
    final backend = RustAppHistoryQueryBackend();
    final reading = expectLater(backend.search('', 'idea'), throwsStateError);
    await Future<void>.delayed(Duration.zero);
    final closing = backend.close();
    api.pending
        .complete(AppHistoryDto(items: const [], failedSources: const []));
    api.opening!.complete(1);
    await Future.wait([reading, closing]);
    expect(api.searches, 0);
    expect(api.cancelled, [1]);
  });

  test('a stale read never reacquires ownership from a newer page', () async {
    final old = RustAppHistoryQueryBackend();
    final reading = old.search('', 'idea');
    await Future<void>.delayed(Duration.zero);
    final next = RustAppHistoryQueryBackend();
    final nextReading = next.search('', 'vscode');
    await Future<void>.delayed(Duration.zero);
    expect(api.opens, 2);
    api.pending.complete(AppHistoryDto(
        items: const [],
        failedSources: const [],
        notice: AppHistoryNoticeDto.stale));
    await Future.wait([reading, nextReading]);
    expect(api.opens, 2,
        reason: 'only explicitly opening a page may obtain a new lease');
    await old.close();
    expect(api.cancelled, [1]);
    await next.close();
    expect(api.cancelled, [1, 2]);
  });
}
