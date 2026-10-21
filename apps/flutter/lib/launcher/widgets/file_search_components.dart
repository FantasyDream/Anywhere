import '../../l10n/app_language.dart';
import 'package:flutter/material.dart';
import '../../ui/app_style.dart';
import '../../ui/file_type_appearance.dart';
import '../file_search_controller.dart';
import '../launcher_view_state.dart';
import 'launcher_components.dart';
import '../../infrastructure/desktop_environment.dart';

/// The file page uses the same filter chips and cycle keys as Scope.
class FileSearchControls extends StatelessWidget {
  const FileSearchControls(
      {super.key,
      required this.files,
      required this.onSelect,
      this.enabled = true});
  final FileSearchController files;
  final VoidCallback onSelect;
  final bool enabled;

  String _typeLabel(BuildContext context, FileFilter type) => switch (type) {
        FileFilter.all => context.l10n.allTypes,
        FileFilter.files => context.l10n.files,
        FileFilter.folders => context.l10n.folders,
        FileFilter.documents => context.l10n.documents,
        FileFilter.pdf => context.l10n.pdf,
        FileFilter.images => context.l10n.images,
        FileFilter.audio => context.l10n.audio,
        FileFilter.video => context.l10n.video,
        FileFilter.archives => context.l10n.archives,
        FileFilter.code => context.l10n.code,
      };

  @override
  Widget build(BuildContext context) => SizedBox(
      key: const Key('file-search-controls'),
      height: LauncherViewState.scopeHeight,
      child: LauncherFilterBar(
          key: const Key('file-type-filters'),
          keyPrefix: 'file-filter',
          selectedId: files.filter.name,
          enabled: enabled,
          options: [
            for (final type in FileFilter.values)
              LauncherFilterOption(type.name, _typeLabel(context, type),
                  'Search this file type · Ctrl+Tab / Ctrl+Shift+Tab'),
          ],
          onSelect: (id) {
            files.setFilter(
                FileFilter.values.firstWhere((type) => type.name == id));
            onSelect();
          }));
}

class FileSearchPreview extends StatelessWidget {
  const FileSearchPreview({super.key, required this.files});
  final FileSearchController files;

  @override
  Widget build(BuildContext context) {
    final preview = files.preview;
    final path = preview?.path;
    final name = path
        ?.split(RegExp(r'[/\\]'))
        .where((part) => part.isNotEmpty)
        .lastOrNull;
    final modified = preview?.modifiedMs == null
        ? null
        : DateTime.fromMillisecondsSinceEpoch(preview!.modifiedMs!.toInt())
            .toLocal();
    final localizations = MaterialLocalizations.of(context);
    final appearance = preview == null
        ? null
        : FileTypeAppearance.forName(name ?? '',
            isDirectory: preview.isDirectory);
    return Column(
        key: const Key('file-preview-pane'),
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Padding(
              padding: const EdgeInsets.fromLTRB(16, 12, 16, 8),
              child: Text(context.l10n.preview, style: AppStyle.paneTitle)),
          Expanded(
              child: files.loading
                  ? const Center(
                      child: SizedBox.square(
                          dimension: 18,
                          child: CircularProgressIndicator(strokeWidth: 1.5)))
                  : preview == null
                      ? Center(
                          child: Padding(
                              padding: const EdgeInsets.all(16),
                              child: Text(
                                  files.error ?? 'Select a file to preview',
                                  textAlign: TextAlign.center,
                                  style: AppStyle.caption)))
                      : KeyedSubtree(
                          // A new target starts at the top rather than inheriting
                          // the previous file's content scroll position.
                          key: ValueKey(path),
                          child: SingleChildScrollView(
                              key: const Key('file-preview'),
                              padding: const EdgeInsets.fromLTRB(16, 8, 16, 16),
                              child: Column(
                                  crossAxisAlignment: CrossAxisAlignment.start,
                                  children: [
                                    FileTypeIcon(
                                        appearance: appearance!, size: 40),
                                    const SizedBox(height: 12),
                                    Text(name ?? path!,
                                        maxLines: 2,
                                        overflow: TextOverflow.ellipsis,
                                        style: AppStyle.body.copyWith(
                                            fontWeight: FontWeight.w600)),
                                    const SizedBox(height: 14),
                                    _property(
                                        context.l10n.kind, appearance.label),
                                    if (!preview.isDirectory)
                                      _property(context.l10n.size,
                                          _size(preview.sizeBytes)),
                                    if (modified != null)
                                      _property(context.l10n.modified,
                                          '${localizations.formatMediumDate(modified)} · ${localizations.formatTimeOfDay(TimeOfDay.fromDateTime(modified))}'),
                                    const SizedBox(height: 12),
                                    Text(context.l10n.location,
                                        style: AppStyle.caption),
                                    const SizedBox(height: 4),
                                    SelectableText(path!,
                                        style: AppStyle.caption.copyWith(
                                            color: AppStyle.secondary)),
                                    if (preview.notice != null) ...[
                                      const SizedBox(height: 12),
                                      Text(preview.notice!,
                                          style: AppStyle.caption),
                                    ],
                                    if (preview.text != null) ...[
                                      const Divider(height: 24),
                                      Container(
                                          width: double.infinity,
                                          padding: const EdgeInsets.all(10),
                                          decoration: BoxDecoration(
                                              color: AppStyle.actionSurface,
                                              borderRadius:
                                                  BorderRadius.circular(
                                                      AppStyle.controlRadius)),
                                          child: SelectableText(preview.text!,
                                              style: TextStyle(
                                                  fontFamily: DesktopEnvironment
                                                      .current.monoFontFamily,
                                                  fontSize: DesignTokens
                                                      .previewCodeSize,
                                                  height: DesignTokens
                                                      .previewCodeLineHeight,
                                                  color: AppStyle.primary))),
                                    ],
                                  ])))),
        ]);
  }

  Widget _property(String label, String value) => Padding(
      padding: const EdgeInsets.only(bottom: 6),
      child: Row(crossAxisAlignment: CrossAxisAlignment.start, children: [
        SizedBox(width: 58, child: Text(label, style: AppStyle.caption)),
        Expanded(
            child: Text(value,
                style: AppStyle.caption.copyWith(color: AppStyle.secondary))),
      ]));

  String _size(BigInt bytes) {
    final amount = bytes.toDouble();
    if (amount < 1024) return '$bytes B';
    if (amount < 1024 * 1024) return '${(amount / 1024).toStringAsFixed(1)} KB';
    if (amount < 1024 * 1024 * 1024) {
      return '${(amount / (1024 * 1024)).toStringAsFixed(1)} MB';
    }
    return '${(amount / (1024 * 1024 * 1024)).toStringAsFixed(1)} GB';
  }
}
