import '../src/rust/api.dart';

enum LauncherPhase { hidden, activating, ready, resolving, executing, settings }

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
    this.resultSource,
    this.resultStatus,
    this.universalShortcut = '⌘⌥⇧Space',
  });
  final LauncherPhase phase;
  final int generation;
  final LauncherStateDto? snapshot;
  final String input;
  final String? scopeId;
  final String? selectedId;
  final String? message;
  final String? output;
  final String? resultSource;
  final ActionStatusDto? resultStatus;
  final String universalShortcut;
  bool get visible =>
      phase == LauncherPhase.ready ||
      phase == LauncherPhase.settings ||
      resolving ||
      executing;
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
        if (candidate.availability != AvailabilityDto.hidden) {
          list.add(candidate);
        }
      }
    } else {
      final ids = scope?.candidateIds;
      if (ids == null) return const [];
      for (final id in ids) {
        final candidate = byId[id];
        if (candidate != null &&
            candidate.availability != AvailabilityDto.hidden) {
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
          : _capturePermissionNotice);

  String? get _capturePermissionNotice {
    for (final capture
        in snapshot?.contextCaptures ?? const <ContextCaptureDto>[]) {
      if (capture.status != ContextCaptureStatusDto.permissionDenied) continue;
      return switch (capture.source) {
        ContextSourceDto.finderSelection =>
          'Allow Finder Automation in System Settings to use selected files.',
        ContextSourceDto.selectedText =>
          'Allow Accessibility in System Settings to use selected text.',
        ContextSourceDto.browserCurrentPage =>
          'Allow Safari Automation in System Settings to use the current page.',
        ContextSourceDto.clipboard =>
          'Allow clipboard access to use copied text.',
        _ => null,
      };
    }
    return null;
  }

  String get searchHint {
    final active = scope;
    if (active == null) {
      return 'Search for an action or app…';
    }
    return 'Search ${active.label} · $universalShortcut for all apps';
  }

  String get resultHeading {
    if (output == null) return 'Suggested';
    if (resultStatus == ActionStatusDto.failed) return 'Failed';
    return 'Succeeded';
  }

  double get previewHeight =>
      ((output?.split('\n').length ?? 0).clamp(3, 14) * 20.0) + 24;

  double get height {
    if (phase == LauncherPhase.settings) {
      return 480;
    }
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
