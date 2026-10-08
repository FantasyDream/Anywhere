import '../settings/host_settings.dart';
import 'dart:convert';
import '../src/rust/api.dart';
import 'context_source_status.dart';

enum LauncherPhase { hidden, activating, ready, resolving, executing, settings }

/// A row is either an executable offer or an explicit menu navigation step.
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
  }) : _universalShortcut = universalShortcut;
  final LauncherPhase phase;
  final int generation;
  final LauncherStateDto? snapshot;
  final String input;
  final InputModeDto inputMode;
  final bool inputModeReady;
  final ContextSourceDto? contentSource;
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
  bool _includesGroup(CandidateDto candidate) =>
      scopeId == null ||
      selectedScope?.isCurrentApp == true ||
      candidate.groupId == scopeId;
  late final List<CandidateDto> allActions = List.unmodifiable([
    for (final candidate
        in snapshot?.contextCandidates ?? const <CandidateDto>[])
      if (candidate.kind == CandidateKindDto.action &&
          candidate.availability != AvailabilityDto.hidden &&
          _includesGroup(candidate))
        candidate,
  ]);
  bool isMenu(CandidateDto candidate) =>
      candidate.menuPath.isNotEmpty ||
      (candidate.commandId?.startsWith('app.') ?? false);
  late final List<CandidateDto> menuActions = List.unmodifiable([
    for (final candidate in allActions)
      if (isMenu(candidate) &&
          (selectedScope?.isCurrentApp != true || candidate.groupId == scopeId))
        candidate,
  ]);
  bool get canBrowseMenus =>
      bindingChoices == null && scopeId != null && menuActions.isNotEmpty;
  late final List<CandidateDto> scopedCandidates = List.unmodifiable([
    for (final candidate in snapshot?.candidates ?? const <CandidateDto>[])
      if (candidate.availability != AvailabilityDto.hidden &&
          (scopeId == null ||
              (candidate.kind == CandidateKindDto.action &&
                  _includesGroup(candidate))))
        candidate,
  ]);

  /// Show the source of the execution target, rather than inferring it from a
  /// title or subtitle. Identical targets can have several merged context roots.
  String? sourceLabelFor(CandidateDto candidate) {
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

  late final List<LauncherEntry> visibleEntries =
      List.unmodifiable(_buildEntries());
  List<LauncherEntry> _buildEntries() {
    if (!inputModeReady ||
        (inputMode == InputModeDto.search && input.isEmpty)) {
      return const [];
    }
    if (bindingChoices != null) {
      return [
        for (final candidate in bindingChoices!)
          LauncherEntry.candidate(candidate)
      ];
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
    final index = visibleEntries.indexWhere((entry) => entry.key == selectedId);
    return index < 0 ? 0 : index;
  }

  double get selectedTop => selectedIndex * rowHeight;
  bool get hasScopeBar => scopes.isNotEmpty;
  bool get hasPanel =>
      output != null ||
      visibleEntries.isNotEmpty ||
      scopeId != null ||
      isContent;
  String get emptyMessage =>
      input.isNotEmpty ? 'No matching actions' : 'No actions available';
  String? get notice =>
      message ??
      captureNotice?.detail ??
      ((snapshot?.diagnostics.isNotEmpty ?? false)
          ? snapshot!.diagnostics.join('\n')
          : null);
  ContextSourceStatus? get captureNotice => ContextSourceStatus.notice(
      snapshot?.contextCaptures ?? const <ContextCaptureDto>[]);
  String? get noticeSettingsPane =>
      message == null ? captureNotice?.settingsPane : null;

  String get searchHint => isContent
      ? 'Edit captured content…'
      : selectedScope == null
          ? 'Search for an action or app…'
          : 'Search ${selectedScope!.label} actions…';
  String get resultHeading => output != null
      ? (resultStatus == ActionStatusDto.failed ? 'Failed' : 'Succeeded')
      : bindingChoices != null
          ? 'Choose a target'
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
    final body = output != null
        ? previewHeight
        : visibleEntries.isEmpty
            ? 64.0
            : (visibleEntries.length * rowHeight + 8)
                .clamp(8, maxRows * rowHeight + 8);
    return (inputAreaHeight +
            (hasScopeBar ? scopeHeight : 0) +
            (hasPanel || hasScopeBar ? 1 : 0) +
            (notice == null ? 0 : 44) +
            (hasPanel
                ? (hasResultHeading ? headerHeight : 0) + body + footerHeight
                : 0))
        .clamp(inputHeight, 520.0);
  }
}
