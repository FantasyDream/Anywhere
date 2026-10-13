import '../settings/host_settings.dart';
import 'file_search_controller.dart';
import 'dart:convert';
import '../src/rust/api.dart';
import 'context_source_status.dart';

enum LauncherPhase { hidden, activating, ready, resolving, executing, settings }

/// Executable offers and menu navigation remain explicit.
class LauncherEntry {
  const LauncherEntry.candidate(CandidateDto action)
      : candidate = action,
        navigationPath = null;
  const LauncherEntry.menu(List<String> path)
      : candidate = null,
        navigationPath = path;
  final CandidateDto? candidate;
  final List<String>? navigationPath;
  String get key => candidate == null
      ? 'menu:${jsonEncode(navigationPath)}'
      : 'candidate:${candidate!.id}';
  String get title =>
      candidate?.title ??
      (navigationPath!.isEmpty ? 'Browse Menus…' : navigationPath!.last);
}

class LauncherEntries {
  LauncherEntries(Iterable<LauncherEntry> entries)
      : rows = List.unmodifiable(entries) {
    for (var i = 0; i < rows.length; i++) {
      indices[rows[i].key] = i;
    }
  }
  final List<LauncherEntry> rows;
  final Map<String, int> indices = {};
}

/// Core supplies available scopes and ranked offers; Host owns menu navigation.
class LauncherViewState {
  LauncherViewState({
    required this.phase,
    required this.generation,
    this.snapshot,
    this.input = '',
    this.inputMode = InputModeDto.environment,
    this.inputModeReady = true,
    this.contentSource,
    this.scopeId,
    this.actionPath,
    this.selectedId,
    this.message,
    this.output,
    this.resultSource,
    this.resultStatus,
    String? universalShortcut,
    this.bindingChoices,
    this.fileActionTitle,
    this.fileFilter = FileFilter.all,
    this.navigationMode,
    this.retainedFileEntries,
  }) : _universalShortcut = universalShortcut;
  final LauncherPhase phase;
  final int generation;
  final LauncherStateDto? snapshot;
  final String input;
  final InputModeDto inputMode;
  final bool inputModeReady;
  final ContextSourceDto? contentSource;
  final FileFilter fileFilter;
  final InputModeDto? navigationMode;
  final LauncherEntries? retainedFileEntries;
  bool get navigationOpen => navigationMode != null;
  static const navigationHeight = 108.0;
  bool get hasFilePreview => isFileSearch;
  double get width =>
      phase == LauncherPhase.settings || !isFileSearch ? 648 : 920;
  bool get hasFileControls =>
      isFileSearch && bindingChoices == null && output == null;
  bool get hasControls => hasScopeBar || hasFileControls;
  bool get isFileSearch => inputMode == InputModeDto.fileSearch;
  bool get isContent => inputMode == InputModeDto.content;
  String get contentLabel => contentSource == ContextSourceDto.selectedText
      ? 'Selected text'
      : 'Recent clipboard';
  double get inputAreaHeight => inputHeight + (isContent ? 24 : 0);
  final String? scopeId;

  /// null = scope home, [] = menu root, [View, Appearance] = submenu.
  final List<String>? actionPath;
  final String? selectedId, message, output, resultSource;
  final ActionStatusDto? resultStatus;
  final String? _universalShortcut;
  String get universalShortcut =>
      _universalShortcut ?? HostSettings.defaults.universalLabel;
  final List<CandidateDto>? bindingChoices;
  final String? fileActionTitle;
  bool get visible =>
      phase == LauncherPhase.ready ||
      phase == LauncherPhase.settings ||
      resolving ||
      executing;
  bool get resolving => phase == LauncherPhase.resolving;
  bool get executing => phase == LauncherPhase.executing;
  static const rowHeight = 56.0,
      inputHeight = 72.0,
      scopeHeight = 36.0,
      headerHeight = 24.0,
      footerHeight = 44.0;
  static const maxRows = 7;
  List<ScopeDto> get scopes => snapshot?.scopes ?? const [];
  late final ScopeDto? selectedScope = _findSelectedScope();
  ScopeDto? _findSelectedScope() {
    for (final scope in scopes) {
      if (scope.id == scopeId) return scope;
    }
    return null;
  }

  bool get browsing => actionPath != null;
  String get searchText => input;
  late final Set<String>? _scopeActions = selectedScope?.actionIds.toSet();
  late final Set<String>? _scopeMenus = selectedScope?.menuIds.toSet();
  late final List<CandidateDto> allActions = List.unmodifiable([
    for (final candidate
        in snapshot?.contextCandidates ?? const <CandidateDto>[])
      if (candidate.availability != AvailabilityDto.hidden &&
          (scopeId == null || (_scopeActions?.contains(candidate.id) ?? false)))
        candidate,
  ]);
  bool isMenu(CandidateDto candidate) => candidate.isMenu;
  late final List<CandidateDto> menuActions = List.unmodifiable([
    for (final candidate in allActions)
      if (scopeId == null
          ? candidate.isMenu
          : (_scopeMenus?.contains(candidate.id) ?? false))
        candidate,
  ]);
  bool get canBrowseMenus =>
      bindingChoices == null && scopeId != null && menuActions.isNotEmpty;
  // These are already matched, scoped and ranked by Rust. UI only protects stale
  // input and organizes the authoritative offers into navigation rows.
  late final List<CandidateDto> scopedCandidates = List.unmodifiable([
    for (final candidate in snapshot?.candidates ?? const <CandidateDto>[])
      if (candidate.availability != AvailabilityDto.hidden) candidate,
  ]);

  /// Show the source of the execution target, rather than inferring it from a
  /// title or subtitle. Identical targets can have several merged context roots.
  String? sourceLabelFor(CandidateDto candidate) {
    if (candidate.kind == CandidateKindDto.file) {
      return isFileSearch || fileActionTitle != null ? null : 'Files';
    }
    if (candidate.kind != CandidateKindDto.action) return null;
    if (isMenu(candidate)) {
      return scopeId == null ? candidate.sectionLabel : null;
    }
    final contexts = snapshot?.contexts ?? const <ContextDto>[];
    ContextDto? target;
    for (final context in contexts) {
      if (context.id == candidate.executionContextId) {
        target = context;
        break;
      }
    }
    if (target == null) {
      for (final context in contexts) {
        if (candidate.contextKeys.contains(context.key)) {
          target = context;
          break;
        }
      }
    }
    return switch (target?.source) {
      ContextSourceDto.browserCurrentPage => 'Current page',
      ContextSourceDto.clipboard => 'Clipboard',
      ContextSourceDto.selectedText => 'Selected text',
      ContextSourceDto.finderSelection => 'Selected files',
      ContextSourceDto.userInput => 'Input',
      _ => null,
    };
  }

  bool _hasPrefix(List<String> path, List<String> prefix) {
    if (path.length < prefix.length) return false;
    for (var index = 0; index < prefix.length; index++) {
      if (path[index] != prefix[index]) return false;
    }
    return true;
  }

  late final LauncherEntries entryCollection =
      retainedFileEntries ?? LauncherEntries(_buildEntries());
  List<LauncherEntry> get visibleEntries => entryCollection.rows;
  List<LauncherEntry> _buildEntries() {
    if (!inputModeReady || (snapshot != null && snapshot!.input != input)) {
      return const [];
    }
    if (bindingChoices != null) {
      return [
        for (final candidate in bindingChoices!)
          LauncherEntry.candidate(candidate)
      ];
    }
    if (isFileSearch) {
      return [
        for (final candidate in scopedCandidates)
          LauncherEntry.candidate(candidate)
      ];
    }
    if (inputMode == InputModeDto.search &&
        scopeId == null &&
        input.trim().isEmpty) {
      return const [];
    }
    if (isContent) {
      return [
        for (final candidate in scopedCandidates)
          LauncherEntry.candidate(candidate)
      ];
    }
    // A query temporarily flattens the whole scope, retaining the browsing path.
    if (input.trim().isNotEmpty) {
      return [
        for (final candidate in scopedCandidates)
          LauncherEntry.candidate(candidate)
      ];
    }
    if (browsing && scopeId != null) {
      final path = actionPath!;
      final children = <String, List<String>>{};
      final actions = <CandidateDto>[];
      for (final candidate in menuActions) {
        if (!_hasPrefix(candidate.menuPath, path)) continue;
        if (candidate.menuPath.length > path.length) {
          final next = candidate.menuPath[path.length];
          children[next] = [...path, next];
        } else {
          actions.add(candidate);
        }
      }
      final folders = children.keys.toList()..sort();
      return [
        for (final name in folders) LauncherEntry.menu(children[name]!),
        for (final action in actions) LauncherEntry.candidate(action),
      ];
    }
    // A deliberate feature scope exposes its actions, including scripts and run targets.
    if (selectedScope != null && selectedScope!.isCurrentApp != true) {
      return [
        for (final candidate in allActions)
          if (!isMenu(candidate)) LauncherEntry.candidate(candidate),
        if (canBrowseMenus) const LauncherEntry.menu([]),
      ];
    }
    // No applications on Context Home; app menus only contribute a small shortlist.
    final source = scopeId == null ? scopedCandidates : allActions;
    final recommended = source
        .where((candidate) =>
            candidate.kind == CandidateKindDto.action && candidate.homeEligible)
        .take(7);
    return [
      for (final candidate in recommended) LauncherEntry.candidate(candidate),
      if (canBrowseMenus) const LauncherEntry.menu([]),
    ];
  }

  late final List<CandidateDto> visibleCandidates = List.unmodifiable([
    for (final entry in visibleEntries)
      if (entry.candidate != null) entry.candidate!,
  ]);
  int get selectedIndex {
    return entryCollection.indices[selectedId] ?? 0;
  }

  double get selectedTop => selectedIndex * rowHeight;
  bool get hasScopeBar =>
      !isFileSearch &&
      !(inputMode == InputModeDto.search &&
          scopeId == null &&
          input.trim().isEmpty) &&
      scopes.isNotEmpty;
  bool get searchLoading =>
      snapshot?.searchSources
          .any((s) => s.state == SearchSourceStateDto.loading) ??
      false;
  bool get searchUnavailable =>
      snapshot?.searchSources
          .any((s) => s.state == SearchSourceStateDto.unavailable) ??
      false;
  bool get searchPartial =>
      snapshot?.searchSources
          .any((s) => s.state == SearchSourceStateDto.partial) ??
      false;
  String get fileRangeLabel {
    final roots = snapshot?.fileSearch?.roots ?? const <String>[];
    if (roots.isEmpty) return 'File Search';
    if (roots.length > 1) return 'File Search · ${roots.length} locations';
    final name = roots.single
            .split(RegExp(r'[/\\]'))
            .where((part) => part.isNotEmpty)
            .lastOrNull ??
        roots.single;
    return 'File Search · $name';
  }

  String get fileRangeDetail =>
      (snapshot?.fileSearch?.roots ?? const <String>[]).join('\n');
  String? get searchStatus {
    final progress = isFileSearch && bindingChoices == null && output == null
        ? snapshot?.fileSearch
        : null;
    if (progress != null) {
      final count = '${progress.confirmedMatches}';
      if (searchPartial) {
        return '$count found · Partial results · ${progress.issueCount} issues';
      }
      if (!progress.finished) return '$count found · Searching…';
      if (progress.inaccessibleCount > BigInt.zero) {
        return '$count found · ${progress.inaccessibleCount} restricted locations skipped';
      }
      return '$count found · Search complete';
    }
    return searchLoading
        ? 'Searching files…'
        : searchUnavailable
            ? isFileSearch
                ? 'File search unavailable'
                : 'File search unavailable · Other results are still usable'
            : null;
  }

  bool get hasPanel =>
      output != null ||
      (snapshot?.searchSources.isNotEmpty ?? false) ||
      visibleEntries.isNotEmpty ||
      scopeId != null ||
      isContent ||
      isFileSearch;
  String get emptyMessage =>
      input.isNotEmpty ? 'No matching actions' : 'No actions available';
  String? get notice {
    if (message != null) return message;
    // File Search owns a separate query: ambient app/menu diagnostics must not
    // appear as errors for a completed file query. File coverage errors remain.
    if (isFileSearch) {
      final issues = snapshot?.fileSearch?.issueExamples ?? const <String>[];
      return searchPartial && issues.isNotEmpty
          ? 'Some locations could not be searched. Available results are still usable.'
          : null;
    }
    return captureNotice?.detail ??
        ((snapshot?.diagnostics.isNotEmpty ?? false)
            ? snapshot!.diagnostics.join('\n')
            : null);
  }

  ContextSourceStatus? get captureNotice => isFileSearch
      ? null
      : ContextSourceStatus.notice(
          snapshot?.contextCaptures ?? const <ContextCaptureDto>[]);
  String? get noticeSettingsPane =>
      message == null ? captureNotice?.settingsPane : null;

  String get searchHint => isFileSearch
      ? 'Search files and folders…'
      : isContent
          ? 'Edit captured content…'
          : selectedScope == null
              ? 'Search apps and actions…'
              : 'Search ${selectedScope!.label} actions…';
  String get resultHeading => output != null
      ? (resultStatus == ActionStatusDto.failed ? 'Failed' : 'Succeeded')
      : bindingChoices != null
          ? fileActionTitle ?? 'Choose a target'
          : isContent
              ? 'Actions for this content'
              : input.trim().isNotEmpty
                  ? 'Results'
                  : browsing
                      ? ['Menus', ...actionPath!].join(' › ')
                      : selectedScope != null &&
                              selectedScope!.isCurrentApp != true
                          ? 'Actions'
                          : 'Suggested';
  // App and query are already visible in the input and Scope bar. Keep headings
  // for content, navigation, binding choices and execution results only.
  bool get hasResultHeading =>
      output != null || browsing || isContent || bindingChoices != null;
  double get previewHeight =>
      ((output?.split('\n').length ?? 0).clamp(3, 14) * 20.0) + 24;
  double get height {
    if (phase == LauncherPhase.settings) return 508;
    if (isFileSearch) return 600;
    if (navigationOpen) return inputAreaHeight + 1 + navigationHeight;
    final body = output != null
        ? previewHeight
        : visibleEntries.isEmpty
            ? 64.0
            : (visibleEntries.length * rowHeight + 8)
                .clamp(8, maxRows * rowHeight + 8);
    return (inputAreaHeight +
            (hasControls ? scopeHeight : 0) +
            (hasPanel || hasControls ? 1 : 0) +
            (notice == null ? 0 : 44) +
            (hasPanel
                ? (hasResultHeading ? headerHeight : 0) + body + footerHeight
                : 0))
        .clamp(inputHeight, 520.0);
  }
}
