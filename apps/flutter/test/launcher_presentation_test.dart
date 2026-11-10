import 'dart:ui' show Locale;

import 'package:anywhere/infrastructure/bridge_api.dart';
import 'package:anywhere/l10n/app_language.dart';
import 'package:anywhere/launcher/launcher_presentation.dart';
import 'package:anywhere/launcher/launcher_view_state.dart';
import 'package:flutter_test/flutter_test.dart';

import 'launcher_fixtures.dart';

void main() {
  final labels = lookupAppLocalizations(const Locale('en'));

  test('file range presentation localizes empty and multiple roots', () {
    expect(FileRangePresentation(const []).label(labels), 'File Search');
    expect(FileRangePresentation(['/Users/me', '/Volumes/work']).label(labels),
        'File Search · 2 locations');
  });

  test('file range presentation uses the final non-empty path component', () {
    final presentation = FileRangePresentation(['/Users/me/Workspace/']);

    expect(presentation.label(labels), 'File Search · Workspace');
    expect(presentation.detail, '/Users/me/Workspace/');
  });

  test('notice presence agrees with both languages for every input mode', () {
    final zh = lookupAppLocalizations(const Locale('zh'));
    for (final mode in InputModeDto.values) {
      for (final partial in [true, false]) {
        for (final message in [null, '', 'Explicit operation error']) {
          final view = LauncherViewState(
              phase: LauncherPhase.ready,
              generation: 1,
              inputMode: mode,
              message: message,
              snapshot: launcherSnapshot(
                  sessionId: 1,
                  revision: 1,
                  snapshotSeq: 1,
                  input: '',
                  contextComplete: true,
                  candidates: [],
                  diagnostics: [
                    'capture failed'
                  ],
                  searchSources: [
                    SearchSourceDto(
                        id: 'files',
                        state: partial
                            ? SearchSourceStateDto.partial
                            : SearchSourceStateDto.ready)
                  ]));
          expect(view.hasNotice, view.noticeFor(labels) != null);
          expect(view.hasNotice, view.noticeFor(zh) != null);
          if (message != null) {
            expect(view.noticeFor(zh), message);
          } else if (view.hasNotice) {
            expect(view.noticeFor(zh), isNot(view.noticeFor(labels)));
          }
        }
      }
    }
  });

  test('one range presentation can render the current language on demand', () {
    final range = FileRangePresentation(['/Users/me', '/Volumes/work']);
    final zh = lookupAppLocalizations(const Locale('zh'));
    expect(range.label(labels), 'File Search · 2 locations');
    expect(range.label(zh), '文件搜索 · 2 个位置');
    expect(range.detail, '/Users/me\n/Volumes/work');
  });
}
