import 'package:flutter_test/flutter_test.dart';
import 'package:anywhere/launcher/context_source_status.dart';
import 'package:anywhere/launcher/launcher_view_state.dart';
import 'package:anywhere/src/rust/api.dart';
import 'launcher_fixtures.dart';

ContextCaptureDto capture(
        ContextSourceDto source, ContextCaptureStatusDto status) =>
    ContextCaptureDto(source: source, status: status, capturedAtMs: 1);

void main() {
  test('ambient capture diagnostics cannot spill into global or content search',
      () {
    final snapshot = launcherSnapshot(
        sessionId: 1,
        revision: 0,
        snapshotSeq: 1,
        input: '',
        contextComplete: true,
        candidates: const [],
        contextCaptures: [
          capture(ContextSourceDto.finderSelection,
              ContextCaptureStatusDto.permissionDenied)
        ],
        diagnostics: [
          'platform.current_directory: captured application is no longer running'
        ]);
    for (final mode in [InputModeDto.search, InputModeDto.content]) {
      final view = LauncherViewState(
          phase: LauncherPhase.ready,
          generation: 1,
          inputMode: mode,
          snapshot: snapshot);
      expect(view.notice, isNull);
      expect(view.noticeSettingsPane, isNull);
      final failedExecution = LauncherViewState(
          phase: LauncherPhase.ready,
          generation: 1,
          inputMode: mode,
          snapshot: snapshot,
          message: 'Copy failed. Try again.');
      expect(failedExecution.notice, 'Copy failed. Try again.');
    }
  });

  test('environment diagnostics use friendly guidance rather than provider IDs',
      () {
    final view = LauncherViewState(
        phase: LauncherPhase.ready,
        generation: 1,
        snapshot: launcherSnapshot(
            sessionId: 1,
            revision: 0,
            snapshotSeq: 1,
            input: '',
            contextComplete: true,
            candidates: const [],
            diagnostics: ['platform.current_directory: transport failed']));
    expect(view.notice, contains('other actions are still available'));
    expect(view.notice, isNot(contains('platform.current_directory')));
  });

  test(
      'expected empty and unsupported sources stay quiet but have status details',
      () {
    for (final status in [
      ContextCaptureStatusDto.empty,
      ContextCaptureStatusDto.unsupported,
      ContextCaptureStatusDto.applicationChanged,
    ]) {
      final item = capture(ContextSourceDto.selectedText, status);
      final presentation = ContextSourceStatus(item.source, item);
      expect(presentation.statusText, isNotEmpty);
      expect(presentation.detail, isNotEmpty);
      expect(ContextSourceStatus.notice([item]), isNull);
    }
    final denied = capture(
        ContextSourceDto.clipboard, ContextCaptureStatusDto.permissionDenied);
    final disabled = ContextSourceStatus(denied.source, denied, enabled: false);
    expect(disabled.attention, 0);
    expect(disabled.settingsPane, isNull);
    expect(disabled.statusText, startsWith('Off'));
  });

  test(
      'permission guidance wins over generic diagnostics regardless of arrival order',
      () {
    final timeout = capture(
        ContextSourceDto.finderSelection, ContextCaptureStatusDto.timedOut);
    final denied = capture(
        ContextSourceDto.clipboard, ContextCaptureStatusDto.permissionDenied);
    for (final captures in [
      [timeout, denied],
      [denied, timeout]
    ]) {
      final view = LauncherViewState(
          phase: LauncherPhase.ready,
          generation: 1,
          snapshot: launcherSnapshot(
              sessionId: 1,
              revision: 0,
              snapshotSeq: 1,
              input: '',
              contextComplete: true,
              searchSources: const [],
              resultActions: const [],
              candidates: [],
              contextCaptures: captures,
              diagnostics: ['clipboard: read failed']));
      expect(view.notice, contains('Allow clipboard access'));
      expect(view.noticeSettingsPane, 'clipboard');
    }
  });

  test(
      'recoverable capture errors describe the next step without pretending permission was denied',
      () {
    for (final status in [
      ContextCaptureStatusDto.timedOut,
      ContextCaptureStatusDto.tooLarge,
      ContextCaptureStatusDto.tooManyItems,
      ContextCaptureStatusDto.unavailable,
      ContextCaptureStatusDto.failed
    ]) {
      final item = capture(ContextSourceDto.finderSelection, status);
      final notice = ContextSourceStatus.notice([item])!;
      expect(notice.detail.toLowerCase(), contains('reopen anywhere'));
      expect(notice.settingsPane, isNull);
    }
  });
}
