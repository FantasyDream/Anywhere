import '../l10n/app_language.dart';
import '../src/rust/api.dart';
import 'launcher_view_state.dart';

/// Presentation data for the selected file-search roots.
///
/// The search state owns the roots; labels and tooltips are derived here so
/// widgets do not duplicate path parsing or localization decisions.
class FileRangePresentation {
  FileRangePresentation(Iterable<String> roots)
      : roots = List.unmodifiable(roots);

  final List<String> roots;

  String get detail => roots.join('\n');

  String label(AppLocalizations l10n) {
    if (roots.isEmpty) return l10n.fileSearch;
    if (roots.length > 1) return l10n.fileLocations(roots.length);
    final name = roots.single
            .split(RegExp(r'[/\\]'))
            .where((part) => part.isNotEmpty)
            .lastOrNull ??
        roots.single;
    return l10n.fileRange(name);
  }
}

/// Localized labels are resolved by the caller, never by the session state.
extension LauncherPresentation on LauncherViewState {
  FileRangePresentation get fileRangePresentation =>
      FileRangePresentation(snapshot?.fileSearch?.roots ?? const <String>[]);
  String get fileRangeDetail => fileRangePresentation.detail;

  /// Show the source of the execution target, rather than inferring it from a
  /// title or subtitle. Identical targets can have several merged context roots.
  String? sourceLabelFor(CandidateDto candidate, AppLocalizations l10n) {
    if (candidate.kind == CandidateKindDto.file) {
      return isFileSearch || objectActionTitle != null ? null : l10n.files;
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
