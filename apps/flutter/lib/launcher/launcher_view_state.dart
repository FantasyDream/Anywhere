import 'dart:collection';
import 'dart:convert';

import '../infrastructure/bridge_api.dart';
import '../settings/host_settings.dart';
import '../tools/tool_page.dart';
import 'context_source_status.dart';
import 'file_search_controller.dart';
import 'stable_list.dart';

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
    // Complete chunks are immutable and shared across appends. Cache only their
    // visibility fact; weak keys allow discarded query chunks to be collected.
    final source = candidates is StableList<CandidateDto>
        ? candidates
        : StableList<CandidateDto>.from(candidates);
    final hidden = source.chunks.any((chunk) => _hiddenFiles[chunk] ??=
        chunk.any((c) => c.availability == AvailabilityDto.hidden));
    final stable = hidden
        ? StableList<CandidateDto>.from(
            source.where((c) => c.availability != AvailabilityDto.hidden))
        : source;
    rows = CandidateRows(stable);
    indices = EntryIndex.files(stable);
  }
  static final _hiddenFiles = Expando<bool>();
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
      : _fileIds = false,
        chunks = [
          (
            0,
            {
              for (var i = 0; i < entries.length; i++) entries[i].key: i,
            }
          )
        ];
  EntryIndex.files(StableList<CandidateDto> candidates)
      : _fileIds = true,
        chunks = [] {
    var offset = 0;
    for (final chunk in candidates.chunks) {
      final local = _fileKeys[chunk] ??= {
        // Reuse the existing ID strings; prefixing every key duplicates all
        // loaded file identifiers for the lifetime of their chunks.
        for (var i = 0; i < chunk.length; i++) chunk[i].id: i,
      };
      chunks.add((offset, local));
      offset += chunk.length;
    }
  }
  static final _fileKeys = Expando<Map<String, int>>();
  final bool _fileIds;
  final List<(int, Map<String, int>)> chunks;
  int? operator [](String? key) {
    if (key == null) return null;
    if (_fileIds) {
      const prefix = 'candidate:';
      if (!key.startsWith(prefix)) return null;
      key = key.substring(prefix.length);
    }
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
  bool get hasFileControls =>
      isFileSearch && bindingChoices == null && output == null;
  bool get hasControls => hasScopeBar || hasFileControls;
  bool get isFileSearch => inputMode == InputModeDto.fileSearch;
  // Input acknowledgement can precede the first asynchronous file-search
  // batch. Measure a nonempty batch immediately, or a confirmed finished empty
  // result; a loading placeholder is not a usable list frame.
  bool get canMeasureQueryFrame =>
      inputModeReady &&
      snapshot?.input == input &&
      (!isFileSearch ||
          visibleEntries.isNotEmpty ||
          snapshot?.fileSearch?.finished == true);
  bool get isContent => inputMode == InputModeDto.content;
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
    // Rust ranks every applicable action. Home only limits the visible rows;
    // domain scopes retain the same order without a recommendation whitelist.
    final source = scopeId == null ? scopedCandidates : allActions;
    final rankedPrefix = source
        .where((candidate) => candidate.kind == CandidateKindDto.action)
        // Reserve room for the full-menu entry, including the Scope bar.
        .take(canBrowseMenus ? 5 : maxRows);
    return [
      for (final candidate in rankedPrefix) LauncherEntry.candidate(candidate),
      if (canBrowseMenus) const LauncherEntry.menu([]),
    ];
  }

  late final List<CandidateDto> visibleCandidates = List.unmodifiable([
    for (final entry in visibleEntries)
      if (entry.candidate != null) entry.candidate!,
  ]);
  late final int selectedIndex =
      entryCollection.indices[selectedId] ?? (hasFileControls ? -1 : 0);

  LauncherEntry? get selectedEntry =>
      selectedIndex < 0 || visibleEntries.isEmpty
          ? null
          : visibleEntries[selectedIndex];

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
  bool get hasPanel =>
      output != null ||
      (snapshot?.searchSources.isNotEmpty ?? false) ||
      visibleEntries.isNotEmpty ||
      scopeId != null ||
      isContent ||
      isFileSearch;
  bool get hasNotice =>
      message != null ||
      (isFileSearch
          ? searchPartial
          : inputMode == InputModeDto.environment &&
              (captureNotice != null ||
                  (snapshot?.diagnostics.isNotEmpty ?? false)));

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

  // App and query are already visible in the input and Scope bar. Keep headings
  // for content, navigation, binding choices and execution results only.
  bool get hasResultHeading =>
      output != null || browsing || isContent || bindingChoices != null;
}
