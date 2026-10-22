import '../l10n/app_language.dart';
import 'dart:ui' show Locale;
import '../tools/tool_page.dart';
import '../ui/design_tokens.g.dart';
import '../settings/host_settings.dart';
import 'file_search_controller.dart';
import 'dart:convert';
import 'dart:collection';
import 'stable_list.dart';
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
    indices = EntryIndex.entries(rows);
  }
  LauncherEntries.files(List<CandidateDto> candidates) {
    // Core has already filtered hidden offers. Protect injected/test snapshots
    // without copying an ordinary visible list on each append.
    final visible =
        candidates.any((c) => c.availability == AvailabilityDto.hidden)
            ? candidates.where((c) => c.availability != AvailabilityDto.hidden)
            : candidates;
    final stable = visible is StableList<CandidateDto>
        ? visible
        : StableList<CandidateDto>.from(visible);
    rows = CandidateRows(stable);
    indices = EntryIndex.files(stable);
  }
  late final List<LauncherEntry> rows;
  late final EntryIndex indices;
}

/// Only visible rows allocate a presentation wrapper. File chunks also share
/// their key lookup across append snapshots, preserving fast keyboard selection.
class CandidateRows extends ListBase<LauncherEntry> {
  CandidateRows(this.candidates);
  final List<CandidateDto> candidates;
  @override
  int get length => candidates.length;
  @override
  set length(int value) => throw UnsupportedError('Immutable entries');
  @override
  LauncherEntry operator [](int index) =>
      LauncherEntry.candidate(candidates[index]);
  @override
  void operator []=(int index, LauncherEntry value) =>
      throw UnsupportedError('Immutable entries');
}

class EntryIndex {
  EntryIndex.entries(List<LauncherEntry> entries)
      : chunks = [
          (
            0,
            {
              for (var i = 0; i < entries.length; i++) entries[i].key: i,
            }
          )
        ];
  EntryIndex.files(StableList<CandidateDto> candidates) : chunks = [] {
    var offset = 0;
    for (final chunk in candidates.chunks) {
      final local = _fileKeys[chunk] ??= {
        for (var i = 0; i < chunk.length; i++) 'candidate:${chunk[i].id}': i,
      };
      chunks.add((offset, local));
      offset += chunk.length;
    }
  }
  static final _fileKeys = Expando<Map<String, int>>();
  final List<(int, Map<String, int>)> chunks;
  int? operator [](String? key) {
    if (key == null) return null;
    for (final (offset, local) in chunks.reversed) {
      final index = local[key];
      if (index != null) return offset + index;
    }
    return null;
  }
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
    this.toolPage,
    this.resultStatus,
    String? universalShortcut,
    this.bindingChoices,
    this.objectActionTitle,
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
      toolPage?.width ??
      (phase == LauncherPhase.settings || !isFileSearch ? 648 : 920);
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
  final String? selectedId, message, output;
  final ActionStatusDto? resultStatus;
  final ToolPageSession? toolPage;
  PluginDetailDto? get pluginDetail => toolPage?.legacyDetail;
  final String? _universalShortcut;
  String get universalShortcut =>
      _universalShortcut ?? HostSettings.defaults.universalLabel;
  final List<CandidateDto>? bindingChoices;
  final String? objectActionTitle;
  bool get visible =>
      phase == LauncherPhase.ready ||
      phase == LauncherPhase.settings ||
      resolving ||
      executing;
  bool get resolving => phase == LauncherPhase.resolving;
  bool get executing => phase == LauncherPhase.executing;
  static const rowHeight = 56.0,
      inputHeight = DesignTokens.pageHeaderHeight,
      scopeHeight = DesignTokens.pageToolbarHeight,
      headerHeight = 24.0;
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
  String? sourceLabelFor(CandidateDto candidate, [AppLocalizations? labels]) {
    final l10n = labels ?? lookupAppLocalizations(const Locale('en'));
    if (candidate.kind == CandidateKindDto.file) {
      return isFileSearch || objectActionTitle != null ? null : 'Files';
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
      ContextSourceDto.browserCurrentPage => l10n.currentPage,
      ContextSourceDto.clipboard => l10n.clipboard,
      ContextSourceDto.selectedText => l10n.selectedText,
      ContextSourceDto.finderSelection => l10n.selectedFiles,
      ContextSourceDto.userInput => l10n.input,
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

  late final LauncherEntries entryCollection = retainedFileEntries ??
      (isFileSearch &&
              bindingChoices == null &&
              inputModeReady &&
              snapshot != null &&
              snapshot!.input == input
          ? LauncherEntries.files(snapshot!.candidates)
          : LauncherEntries(_buildEntries()));
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
        // Reserve room for the full-menu entry, including the Scope bar.
        .take(canBrowseMenus ? 5 : maxRows);
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
      scopes.isNotEmpty &&
      (scopes.length > 1 || !scopes.single.isCurrentApp);
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
  String? get searchStatus =>
      searchStatusFor(lookupAppLocalizations(const Locale('en')));
  String? searchStatusFor(AppLocalizations l10n) {
    final progress = isFileSearch && bindingChoices == null && output == null
        ? snapshot?.fileSearch
        : null;
    if (progress != null) {
      final count = l10n.fileFoundCount(progress.confirmedMatches.toInt());
      if (searchPartial) {
        return '$count · ${l10n.partialResults} · ${l10n.searchIssues(progress.issueCount.toInt())}';
      }
      if (!progress.finished) return '$count · ${l10n.searching}';
      if (progress.inaccessibleCount > BigInt.zero) {
        return '$count · ${l10n.restrictedLocationsSkipped('${progress.inaccessibleCount}')}';
      }
      return '$count · ${l10n.searchComplete}';
    }
    return searchLoading
        ? l10n.searchingFiles
        : searchUnavailable
            ? isFileSearch
                ? l10n.fileSearchUnavailable
                : l10n.fileSearchOtherResults
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
  String? get notice => noticeFor(lookupAppLocalizations(const Locale('en')));
  String? noticeFor(AppLocalizations l10n) {
    if (message != null) return message;
    if (isFileSearch) {
      if (!searchPartial) return null;
      final count = snapshot?.fileSearch?.issueCount ?? BigInt.zero;
      return count > BigInt.zero
          ? l10n.fileSearchIssueNotice(count.toInt())
          : l10n.fileSearchIncomplete;
    }
    if (inputMode != InputModeDto.environment) return null;
    return captureNotice?.withLocalizations(l10n).detail ??
        ((snapshot?.diagnostics.isNotEmpty ?? false)
            ? l10n.actionsLoadIncomplete
            : null);
  }

  ContextSourceStatus? get captureNotice =>
      inputMode != InputModeDto.environment
          ? null
          : ContextSourceStatus.notice(
              (snapshot?.contextCaptures ?? const <ContextCaptureDto>[]).where(
                  (capture) =>
                      capture.source != ContextSourceDto.selectedText &&
                      capture.source != ContextSourceDto.clipboard));
  String? get noticeSettingsPane =>
      message == null ? captureNotice?.settingsPane : null;

  String get searchHint => isFileSearch
      ? 'Search files and folders…'
      : isContent
          ? 'Edit captured content…'
          : selectedScope == null
              ? 'Search apps and actions…'
              : 'Search ${selectedScope!.label} actions…';
  String get resultHeading =>
      resultHeadingFor(lookupAppLocalizations(const Locale('en')));
  String resultHeadingFor(AppLocalizations l10n) => output != null
      ? (resultStatus == ActionStatusDto.failed ? l10n.failed : l10n.succeeded)
      : bindingChoices != null
          ? objectActionTitle ?? l10n.chooseTarget
          : isContent
              ? l10n.contentActions
              : input.trim().isNotEmpty
                  ? l10n.results
                  : browsing
                      ? [l10n.menus, ...actionPath!].join(' › ')
                      : selectedScope != null &&
                              selectedScope!.isCurrentApp != true
                          ? l10n.actions
                          : l10n.suggested;
  // App and query are already visible in the input and Scope bar. Keep headings
  // for content, navigation, binding choices and execution results only.
  bool get hasResultHeading =>
      output != null || browsing || isContent || bindingChoices != null;
  double get previewHeight =>
      ((output?.split('\n').length ?? 0).clamp(3, 14) * 20.0) + 24;
  double get height {
    if (phase == LauncherPhase.settings) return 508;
    if (toolPage != null) return toolPage!.height;
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
            (hasPanel ? (hasResultHeading ? headerHeight : 0) + body : 0))
        .clamp(inputHeight, 520.0);
  }
}

/// Stable scope IDs, never translated labels, choose built-in category names.
String localizedScopeLabel(ScopeDto scope, AppLocalizations l10n) {
  if (scope.isCurrentApp) return scope.label;
  return switch (scope.id) {
    'folder' => l10n.folder,
    'file' => l10n.file,
    'web' => l10n.web,
    'tools' => l10n.tools,
    _ => scope.label,
  };
}
