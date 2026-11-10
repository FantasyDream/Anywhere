import 'dart:async';

import 'package:anywhere/features/clipboard/clipboard_history_session.dart';
import 'package:anywhere/infrastructure/bridge_api.dart';
import 'package:anywhere/launcher/launcher_view_state.dart';
import 'package:anywhere/launcher/stable_list.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter_test/flutter_test.dart';

import 'support/clipboard_fixtures.dart';

class CountedFile extends CandidateDto {
  CountedFile(int index, {bool hidden = false})
      : super(
            id: 'file:$index',
            kind: CandidateKindDto.file,
            title: '$index',
            menuPath: const [],
            contextKeys: const [],
            primaryAction: true,
            iconKey: 'file',
            score: 0,
            groupId: 'files',
            availability:
                hidden ? AvailabilityDto.hidden : AvailabilityDto.enabled);
  static int reads = 0;
  @override
  AvailabilityDto get availability {
    reads++;
    return super.availability;
  }
}

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  test('file appends inspect only new chunks for hidden rows', () {
    var rows = StableList<CandidateDto>.from(
        List.generate(50000, (index) => CountedFile(index)));
    LauncherEntries.files(rows);
    CountedFile.reads = 0;
    for (var index = 0; index < 10; index++) {
      rows = rows.append([CountedFile(50000 + index)]);
      expect(LauncherEntries.files(rows).rows.length, rows.length);
    }
    debugPrint('file_visibility_reads_after_10_appends=${CountedFile.reads}');
    expect(CountedFile.reads, lessThanOrEqualTo(10 * StableList.chunkSize));
  });

  test('chunk visibility cache still omits injected hidden files', () {
    final rows = StableList<CandidateDto>.from([
      CountedFile(0, hidden: true),
      CountedFile(1),
    ]);
    final before = LauncherEntries.files(rows);
    final after = LauncherEntries.files(rows.append([CountedFile(2)]));
    expect(before.rows.single.candidate!.id, 'file:1');
    expect(after.rows.map((row) => row.candidate!.id), ['file:1', 'file:2']);
    expect(after.indices['candidate:file:0'], isNull);
  });

  test('file selection index shares identifiers with its candidate chunks', () {
    final rows =
        StableList<CandidateDto>.from(List.generate(50000, CountedFile.new));
    final index = EntryIndex.files(rows);
    var shared = 0, copiedUnits = 0;
    for (var chunk = 0; chunk < rows.chunks.length; chunk++) {
      final keys = index.chunks[chunk].$2.keys.toList();
      for (var item = 0; item < keys.length; item++) {
        if (identical(keys[item], rows.chunks[chunk][item].id)) {
          ++shared;
        } else {
          copiedUnits += keys[item].length;
        }
      }
    }
    debugPrint(
        'file_index_shared_ids=$shared extra_key_code_units=$copiedUnits');
    expect(shared, 50000);
    expect(copiedUnits, 0);
    expect(index['candidate:file:49999'], 49999);
    expect(index['menu:file:49999'], isNull);
    expect(index['candidate:missing'], isNull);
  });

  testWidgets('queued refresh waits for the newest query debounce',
      (tester) async {
    final backend = History();
    final session = ClipboardHistorySession(backend);
    await tester.pump();
    final blocked = Completer<ClipboardHistoryDto>();
    final queries = <String>[];
    backend.searchOverride = (query) {
      queries.add(query);
      return queries.length == 1
          ? blocked.future
          : Future.value(ClipboardHistoryDto(
              recording: false, available: true, items: [item(query)]));
    };
    final first = session.refresh();
    session.query.text = 'intermediate';
    await tester.pump(const Duration(milliseconds: 120));
    session.query.text = 'latest';
    blocked.complete(ClipboardHistoryDto(
        recording: false, available: true, items: [item('old')]));
    await tester.pump();
    expect(queries, ['']);
    expect(session.items, isEmpty);
    await tester.pump(const Duration(milliseconds: 120));
    await tester.pump();
    await first;
    expect(queries, ['', 'latest']);
    expect(session.items.single.id, 'latest');
    session.dispose();
  });

  test('clipboard refresh admits one read and coalesces pending requests',
      () async {
    final backend = History();
    final session = ClipboardHistorySession(backend);
    await Future<void>.delayed(Duration.zero);
    final blocked = Completer<ClipboardHistoryDto>();
    var calls = 0;
    backend.searchOverride = (_) {
      calls++;
      return calls == 1
          ? blocked.future
          : Future.value(ClipboardHistoryDto(
              recording: false, available: true, items: [item('latest')]));
    };
    try {
      final reads = [
        session.refresh(),
        for (var i = 0; i < 5; i++) session.refresh()
      ];
      expect(calls, 1);
      blocked.complete(ClipboardHistoryDto(
          recording: false, available: true, items: [item('old')]));
      await Future.wait(reads);
      expect(calls, 2);
      expect(session.items.single.id, 'latest');
    } finally {
      session.dispose();
      if (!blocked.isCompleted) {
        blocked.complete(const ClipboardHistoryDto(
            recording: false, available: true, items: []));
      }
    }
  });
}
