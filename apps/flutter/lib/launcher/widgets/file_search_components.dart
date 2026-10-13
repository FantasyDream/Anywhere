import 'package:flutter/material.dart';
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
              LauncherFilterOption(type.name, type.label,
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
          const Padding(
              padding: EdgeInsets.fromLTRB(16, 12, 16, 8),
              child: Text('Preview', style: AppStyle.caption)),
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
                                    _property('Kind', appearance.label),
                                    if (!preview.isDirectory)
                                      _property(
                                          'Size', _size(preview.sizeBytes)),
                                    if (modified != null)
                                      _property('Modified',
                                          '${localizations.formatMediumDate(modified)} · ${localizations.formatTimeOfDay(TimeOfDay.fromDateTime(modified))}'),
                                    const SizedBox(height: 12),
                                    const Text('Location',
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
                                                  BorderRadius.circular(8)),
                                          child: SelectableText(preview.text!,
                                              style: const TextStyle(
                                                  fontFamily: 'monospace',
                                                  fontSize: 11,
                                                  height: 1.55,
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
