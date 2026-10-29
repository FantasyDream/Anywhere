import 'package:flutter/material.dart';

import '../../l10n/app_language.dart';
import '../../ui/app_style.dart';
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
