import '../l10n/app_language.dart';

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
