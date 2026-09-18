import '../src/rust/api.dart';

enum LauncherPhase { hidden, activating, ready, resolving, executing }

/// Immutable presentation state; Rust still owns matching, ranking and scopes.
class LauncherViewState {
  const LauncherViewState({
    required this.phase,
    required this.generation,
    this.snapshot,
    this.input = '',
    this.scopeId,
    this.selectedId,
    this.message,
    this.output,
  });
  final LauncherPhase phase;
  final int generation;
  final LauncherStateDto? snapshot;
  final String input;
  final String? scopeId;
  final String? selectedId;
  final String? message;
  final String? output;
  bool get visible => phase == LauncherPhase.ready || resolving || executing;
  bool get resolving => phase == LauncherPhase.resolving;
  bool get executing => phase == LauncherPhase.executing;
  static const rowHeight = 52.0,
      inputHeight = 64.0,
      scopeHeight = 32.0,
      headerHeight = 28.0,
      footerHeight = 32.0;
  static const maxRows = 7;
  List<ScopeDto> get scopes => snapshot?.scopes ?? const [];
  ScopeDto? get scope {
    final id = scopeId;
    if (id == null) return null;
    for (final scope in scopes) {
      if (scope.id == id) return scope;
    }
    return null;
  }

  /// Candidates of the active scope, ranked, as this view will show them.
  ///
  /// `scopeId == null` is Universal Search over the full ranked list. A selected
  /// scope that has no visible actions stays empty rather than leaking other rows.
  List<CandidateDto> get visibleCandidates {
    final state = snapshot;
    if (state == null) return const [];
    final byId = <String, CandidateDto>{
      for (final candidate in state.candidates) candidate.id: candidate,
    };
    final list = <CandidateDto>[];
    final selected = scopeId;
    if (selected == null) {
      for (final candidate in state.candidates) {
        if (candidate.availability != 'hidden') list.add(candidate);
      }
    } else {
      final ids = scope?.candidateIds;
      if (ids == null) return const [];
      for (final id in ids) {
        final candidate = byId[id];
        if (candidate != null && candidate.availability != 'hidden') {
          list.add(candidate);
        }
      }
    }
    return list;
  }

  int get selectedIndex {
    final candidates = visibleCandidates;
    final id = selectedId;
    if (id != null) {
      final index = candidates.indexWhere((candidate) => candidate.id == id);
      if (index >= 0) return index;
    }
    return 0;
  }

  String? get notice =>
      message ??
      ((snapshot?.diagnostics.isNotEmpty ?? false)
          ? snapshot!.diagnostics.join('\n')
          : null);
  double get previewHeight =>
      ((output?.split('\n').length ?? 0).clamp(3, 14) * 20.0) + 24;

  double get height {
    final candidates = visibleCandidates;
    final body = output != null
        ? previewHeight
        : candidates.length.clamp(0, maxRows) * rowHeight + 8;
    final scope = scopes.isNotEmpty ? scopeHeight : 0.0;
    final panel = output != null || candidates.isNotEmpty
        ? headerHeight + body + footerHeight + 2
        : 0.0;
    return (inputHeight + scope + (notice == null ? 0 : 44) + panel).clamp(
      inputHeight,
      520.0,
    );
  }
}
