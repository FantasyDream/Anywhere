import 'package:flutter/material.dart';

import '../../l10n/app_language.dart';
import '../../ui/app_components.dart';
import '../../ui/app_style.dart';
import '../../ui/file_type_appearance.dart';
import '../file_search_controller.dart';
import '../launcher_view_state.dart';
import 'launcher_components.dart';

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

  String _typeLabel(BuildContext context, FileFilter type) =>
      type.labelFor(context.l10n);

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
                  context.l10n.searchThisFileType),
          ],
          onSelect: (id) {
            files.setFilter(
                FileFilter.values.firstWhere((type) => type.name == id));
            onSelect();
          }));
}

/// Search progress describes the file query, not the selected operation.
class FileSearchStatus extends StatelessWidget {
  const FileSearchStatus({super.key, required this.label, this.onRetry});
  final String label;
  final VoidCallback? onRetry;

  @override
  Widget build(BuildContext context) => Padding(
      key: const Key('file-search-status'),
      padding: const EdgeInsets.fromLTRB(20, 0, 14, 4),
      child: Row(children: [
        Expanded(
            child: Text(label,
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: AppStyle.caption.copyWith(fontSize: 10.5))),
        if (onRetry != null)
          TextButton(onPressed: onRetry, child: Text(context.l10n.retry)),
      ]));
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
                                  files.errorFor(context.l10n) ??
                                      context.l10n.selectAFileToPreview,
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
                                    Row(
                                        crossAxisAlignment:
                                            CrossAxisAlignment.start,
                                        children: [
                                          FileTypeIcon(
                                              appearance: appearance!,
                                              size: 26),
                                          const SizedBox(width: 8),
                                          Expanded(
                                              child: Text(name ?? path!,
                                                  maxLines: 2,
                                                  overflow:
                                                      TextOverflow.ellipsis,
                                                  style: AppStyle.body.copyWith(
                                                      fontWeight:
                                                          FontWeight.w600))),
                                        ]),
                                    const SizedBox(height: 8),
                                    Tooltip(
                                        message: path!,
                                        child: SelectableText(path,
                                            maxLines: 2,
                                            style: AppStyle.caption)),
                                    const SizedBox(height: 8),
                                    AppDisclosure(
                                        key: const Key('file-preview-details'),
                                        compact: true,
                                        title: context.l10n.fileInformation,
                                        initiallyExpanded: preview.text == null,
                                        child: Column(children: [
                                          _property(
                                              context.l10n.kind,
                                              appearance
                                                  .labelFor(context.l10n)),
                                          if (!preview.isDirectory)
                                            _property(context.l10n.size,
                                                _size(preview.sizeBytes)),
                                          if (modified != null)
                                            _property(context.l10n.modified,
                                                '${localizations.formatMediumDate(modified)} · ${localizations.formatTimeOfDay(TimeOfDay.fromDateTime(modified))}'),
                                        ])),
                                    if (_notice(context)
                                        case final notice?) ...[
                                      const SizedBox(height: 10),
                                      Text(notice, style: AppStyle.caption),
                                    ],
                                    if (preview.text != null) ...[
                                      const SizedBox(height: 12),
                                      Container(
                                          width: double.infinity,
                                          padding: const EdgeInsets.all(10),
                                          decoration: BoxDecoration(
                                              color: AppStyle.actionSurface,
                                              borderRadius:
                                                  BorderRadius.circular(
                                                      AppStyle.controlRadius)),
                                          child: SelectableText(preview.text!,
                                              style: AppStyle.code)),
                                    ],
                                  ])))),
        ]);
  }

  String? _notice(BuildContext context) {
    final preview = files.preview;
    if (preview == null || preview.isDirectory) return null;
    final l10n = context.l10n;
    final notices = <String>[
      if (preview.text == null) l10n.filePreviewUnsupported,
      if (preview.truncatedAtBytes case final limit?)
        l10n.filePreviewTruncated('${limit ~/ 1024} KiB'),
      if (preview.encoding case final encoding?)
        if (preview.encodingGuessed)
          l10n.filePreviewEncodingGuessed(encoding)
        else if (encoding != 'UTF-8')
          l10n.filePreviewDecoded(encoding),
    ];
    return notices.isEmpty ? null : notices.join(' · ');
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

extension FileFilterLocalization on FileFilter {
  String labelFor(AppLocalizations l10n) => switch (this) {
        FileFilter.all => l10n.allTypes,
        FileFilter.files => l10n.files,
        FileFilter.folders => l10n.folders,
        FileFilter.documents => l10n.documents,
        FileFilter.pdf => l10n.pdf,
        FileFilter.images => l10n.images,
        FileFilter.audio => l10n.audio,
        FileFilter.video => l10n.video,
        FileFilter.archives => l10n.archives,
        FileFilter.code => l10n.code,
      };
}
