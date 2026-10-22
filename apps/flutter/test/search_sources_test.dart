import 'package:flutter_test/flutter_test.dart';
import 'package:anywhere/launcher/launcher_view_state.dart';
import 'package:anywhere/settings/host_settings.dart';
import 'package:anywhere/src/rust/api.dart';
import 'launcher_fixtures.dart';

void main() {
  test('file search settings round trip and migrate older settings', () {
    final settings = HostSettings.defaults.copyWith(
        fileSearchEnabled: false,
        fileSearchEngine: FileSearchEngineDto.combined,
        fileSearchIncludeHidden: true,
        fileSearchRoots: ['/Users/test/Documents'],
        fileSearchExcludedNames: ['logs']);
    final loaded = HostSettings.fromMap(settings.toMap());
    expect(loaded.fileSearchEnabled, false);
    expect(loaded.fileSearchEngine, FileSearchEngineDto.combined);
    expect(loaded.fileSearchIncludeHidden, true);
    expect(HostSettings.fromMap({}).fileSearchEngine,
        FileSearchEngineDto.systemIndex);
    expect(
        HostSettings.fromMap({'fileSearchEngine': 'unknown'}).fileSearchEngine,
        FileSearchEngineDto.systemIndex);
    expect(HostSettings.fromMap({}).fileSearchIncludeHidden, false);
    expect(loaded.fileSearchRoots, ['/Users/test/Documents']);
    expect(loaded.fileSearchExcludedNames, ['logs']);
    expect(HostSettings.fromMap({}).fileSearchEnabled, true);
    expect(settings.fileSearchError, isNull);
    expect(settings.copyWith(fileSearchRoots: ['relative']).fileSearchError,
        isNotNull);
    expect(
        settings
            .copyWith(fileSearchExcludedNames: ['nested/name']).fileSearchError,
        isNotNull);
  });
  test('loading, empty and unavailable search are distinct and show a panel',
      () {
    LauncherViewState view(SearchSourceStateDto status) => LauncherViewState(
        phase: LauncherPhase.ready,
        generation: 1,
        input: 'report',
        inputMode: InputModeDto.search,
        snapshot: launcherSnapshot(
            sessionId: 1,
            revision: 1,
            snapshotSeq: 1,
            input: 'report',
            contextComplete: true,
            searchSources: [SearchSourceDto(id: 'files', state: status)],
            candidates: [],
            diagnostics: []));
    expect(view(SearchSourceStateDto.loading).hasPanel, true);
    expect(view(SearchSourceStateDto.loading).searchStatus, 'Searching files…');
    expect(view(SearchSourceStateDto.ready).searchStatus, isNull);
    expect(view(SearchSourceStateDto.ready).hasPanel, true);
    expect(view(SearchSourceStateDto.unavailable).searchStatus,
        contains('Other results are still usable'));
  });
}
