import 'dart:async';
import '../infrastructure/runtime_policy.dart';
import '../src/rust/api.dart';
import 'stable_list.dart';

/// Request identity and retained-session lifetime. This service has no Scope,
/// menu, ranking, keyboard or Widget decisions.
class LauncherSessionCoordinator {
  LauncherSessionCoordinator(this.nowMillis);
  final int Function() nowMillis;
  int generation = 0, revision = 0, requestedRevision = 0;
  int? session;
  int? hiddenAt;
  Object? capturePolicy;
  Future<void>? suspension;
  Timer? _deadline;
  Future<bool>? _restoring;
  static const retention =
      Duration(milliseconds: RuntimePolicy.launcherRetentionMs);
  bool current(int value) => value == generation;
  bool get retained => hiddenAt != null;
  bool canResume(Object policy) =>
      hiddenAt != null &&
      capturePolicy == policy &&
      nowMillis() - hiddenAt! < retention.inMilliseconds;
  void retain(Object policy, Future<void> pending, void Function() expire) {
    hiddenAt = nowMillis();
    capturePolicy = policy;
    suspension = pending;
    _deadline?.cancel();
    _deadline = Timer(retention, expire);
  }

  void cancelDeadline() {
    _deadline?.cancel();
    _deadline = null;
  }

  Future<void>? clearRetention() {
    cancelDeadline();
    hiddenAt = null;
    capturePolicy = null;
    final pending = suspension;
    suspension = null;
    return pending;
  }

  Future<bool> restoreOnce(Future<bool> Function() run) =>
      _restoring ??= run().whenComplete(() => _restoring = null);
  void dispose() {
    cancelDeadline();
    ++generation;
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
