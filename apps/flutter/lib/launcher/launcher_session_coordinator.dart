import 'dart:async';
import '../infrastructure/runtime_policy.dart';
import '../src/rust/api.dart';
import 'launcher_backend.dart';
import 'stable_list.dart';

/// Request identity and retained-session lifetime. This service has no Scope,
/// menu, ranking, keyboard or Widget decisions.
class LauncherSessionCoordinator {
  LauncherSessionCoordinator(this.nowMillis);
  final int Function() nowMillis;
  int _generation = 0, _revision = 0, _requestedRevision = 0;
  int? _session;
  LauncherStateDto? _snapshot;
  int? _hiddenAt;
  Object? _capturePolicy;
  Future<void>? _suspension;
  Timer? _deadline;
  Future<bool>? _restoring;
  static const retention =
      Duration(milliseconds: RuntimePolicy.launcherRetentionMs);
  int get generation => _generation;
  int get revision => _revision;
  int get requestedRevision => _requestedRevision;
  int? get session => _session;
  LauncherStateDto? get snapshot => _snapshot;
  int? get hiddenAt => _hiddenAt;
  Future<void>? get suspension => _suspension;

  bool current(int value) => value == _generation;
  bool get retained => _hiddenAt != null;
  bool canResume(Object policy) =>
      _hiddenAt != null &&
      _capturePolicy == policy &&
      nowMillis() - _hiddenAt! < retention.inMilliseconds;

  int beginGeneration() => ++_generation;

  void invalidateGeneration() {
    ++_generation;
  }

  void attachSession(int value) {
    _session = value;
  }

  void clearSession() {
    _session = null;
    clearSnapshot();
  }

  void clearSnapshot() {
    _snapshot = null;
  }

  void acceptRevision(int value) {
    _revision = value;
    _requestedRevision = value;
  }

  int beginRevision() {
    final next = ++_revision;
    _requestedRevision = next;
    return next;
  }

  /// Commit only a newer snapshot from the active session and requested input.
  /// A tool may advance input in Rust without a local update request.
  bool acceptSnapshot(LauncherStateDto state, {bool followInput = false}) {
    if (state.sessionId != _session || state.revision < _requestedRevision) {
      return false;
    }
    final held = _snapshot;
    if (held != null &&
        (state.revision < held.revision ||
            (state.revision == held.revision &&
                state.snapshotSeq <= held.snapshotSeq))) {
      return false;
    }
    if (followInput && state.revision > _revision) {
      _requestedRevision = state.revision;
    }
    _snapshot = state;
    _revision = state.revision;
    return true;
  }

  /// Follow one generation, merging deltas against the latest committed list.
  /// The owner supplies visibility and publishes UI changes after acceptance.
  Future<void> follow(
      {required LauncherBackend backend,
      required int generation,
      required int session,
      required bool Function() visible,
      required Future<void> Function(LauncherStateDto) publish}) async {
    final wait = backend.waitForSnapshot;
    final waitDelta = backend.waitForSnapshotDelta;
    if (wait == null && waitDelta == null) return;
    bool active() => current(generation) && _session == session && visible();
    var cursor = _snapshot == null ? 0 : snapshotCursor(_snapshot!);
    while (active()) {
      final baseCount = _snapshot?.candidates.length ?? 0;
      final notice = waitDelta == null
          ? await wait!(session, cursor)
          : await waitDelta(
              session, cursor, _snapshot?.candidateListVersion ?? 0, baseCount);
      if (!active()) return;
      if (notice.kind == SnapshotWaitKindDto.timedOut) continue;
      if (notice.kind != SnapshotWaitKindDto.updated) return;
      final packet = notice.state;
      if (packet == null || packet.sessionId != session) return;
      final heldCursor =
          _snapshot == null ? cursor : snapshotCursor(_snapshot!);
      final nextCursor = snapshotCursor(packet);
      if (nextCursor <= heldCursor) {
        cursor = heldCursor;
        continue;
      }
      final next = materialize(_snapshot, notice, baseCount);
      if (next == null) {
        cursor = heldCursor;
        continue;
      }
      cursor = nextCursor;
      await publish(next);
    }
  }

  void retain(Object policy, Future<void> pending, void Function() expire) {
    _hiddenAt = nowMillis();
    _capturePolicy = policy;
    _suspension = pending;
    _deadline?.cancel();
    _deadline = Timer(retention, expire);
  }

  void cancelDeadline() {
    _deadline?.cancel();
    _deadline = null;
  }

  Future<void>? clearRetention() {
    cancelDeadline();
    _hiddenAt = null;
    _capturePolicy = null;
    final pending = _suspension;
    _suspension = null;
    return pending;
  }

  Future<bool> restoreOnce(Future<bool> Function() run) =>
      _restoring ??= run().whenComplete(() => _restoring = null);
  void dispose() {
    cancelDeadline();
    invalidateGeneration();
  }

  /// The transport supplies replacements, stable-list tails, or metadata only.
  /// Reject a tail based on a list that changed while the request was pending.
  static LauncherStateDto? materialize(
      LauncherStateDto? held, SnapshotWaitDto packet, int baseCount) {
    final next = packet.state;
    if (next == null || packet.candidateUpdate == CandidateUpdateDto.replace) {
      return next;
    }
    if (held == null ||
        held.sessionId != next.sessionId ||
        held.revision != next.revision ||
        held.candidateListVersion != next.candidateListVersion ||
        held.candidates.length != baseCount) {
      return null;
    }
    final rows = packet.candidateUpdate == CandidateUpdateDto.retain
        ? held.candidates
        : (held.candidates is StableList<CandidateDto>
                ? held.candidates as StableList<CandidateDto>
                : StableList<CandidateDto>.from(held.candidates))
            .append(next.candidates);
    return LauncherStateDto(
        sessionId: next.sessionId,
        revision: next.revision,
        snapshotSeq: next.snapshotSeq,
        candidateListVersion: next.candidateListVersion,
        input: next.input,
        contextComplete: next.contextComplete,
        searchSources: next.searchSources,
        fileSearch: next.fileSearch,
        resultActions: next.resultActions,
        contexts: next.contexts,
        contextCandidates: next.contextCandidates,
        preferredContextKey: next.preferredContextKey,
        scopes: next.scopes,
        selectedScope: next.selectedScope,
        contextCaptures: next.contextCaptures,
        candidates: rows,
        diagnostics: next.diagnostics);
  }
}
