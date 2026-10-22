import '../l10n/app_language.dart';
import '../infrastructure/desktop_environment.dart';
import '../ui/app_icons.dart';
import 'package:flutter/material.dart';
import '../ui/app_style.dart';
import '../ui/app_components.dart';
import 'settings_components.dart';

/// Draft fields are owned by SettingsScreen so its close/save decision includes them.
class FileSearchSettings extends StatelessWidget {
  const FileSearchSettings(
      {super.key,
      required this.enabled,
      required this.includeHidden,
      required this.onHidden,
      required this.roots,
      required this.exclusions,
      required this.onToggle,
      required this.onSave,
      this.onAuthorize,
      this.onDiskAccess});
  final bool enabled;
  final bool includeHidden;
  final ValueChanged<bool> onHidden;
  final TextEditingController roots, exclusions;
  final ValueChanged<bool> onToggle;
  final VoidCallback? onSave;
  final VoidCallback? onAuthorize, onDiskAccess;
  @override
  Widget build(BuildContext context) =>
      Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
        SettingsToggleRow(
            icon: AppIcons.folder,
            title: context.l10n.fileSearchSetting,
            description: context.l10n.searchFileNamesInTheFileSearch,
            value: enabled,
            onChanged: onToggle),
        const Divider(),
        SettingsToggleRow(
            title: context.l10n.includeHiddenFiles,
            description:
                context.l10n.includeDotfilesAndFilesInHiddenDirectories,
            value: includeHidden,
            onChanged: onHidden),
        Padding(
            padding: const EdgeInsets.all(12),
            child:
                Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
              Text(context.l10n.searchDirectories, style: AppStyle.caption),
              const SizedBox(height: 6),
              TextField(
                  key: const Key('file-search-roots'),
                  controller: roots,
                  minLines: 2,
                  maxLines: 4,
                  style: const TextStyle(fontSize: 12),
                  decoration: InputDecoration(
                      isDense: true,
                      hintText: context.l10n.fileRootsHint(
                          switch (DesktopEnvironment.current.platform) {
                        TargetPlatform.windows => r'C:\Users\you\Documents',
                        TargetPlatform.macOS => '/Users/you/Documents',
                        _ => '/home/you/Documents',
                      }))),
              const SizedBox(height: 12),
              AppDisclosure(
                  key: const Key('file-search-rules'),
                  title: context.l10n.searchRules,
                  child: Column(
                      crossAxisAlignment: CrossAxisAlignment.stretch,
                      children: [
                        Text(context.l10n.excludedDirectoryNames,
                            style: AppStyle.caption),
                        const SizedBox(height: 6),
                        TextField(
                            key: const Key('file-search-exclusions'),
                            controller: exclusions,
                            minLines: 2,
                            maxLines: 4,
                            style: const TextStyle(fontSize: 12),
                            decoration: InputDecoration(
                                isDense: true,
                                hintText:
                                    context.l10n.oneDirectoryNamePerLine)),
                        const SizedBox(height: 8),
                        Text(
                            context.l10n
                                .filesAndFoldersAreScannedIncludingGitignored,
                            style: AppStyle.caption),
                      ])),
              if (DesktopEnvironment.current.fileSearchAccess) ...[
                const SizedBox(height: 8),
                OutlinedButton(
                    onPressed: onAuthorize,
                    child: Text(context.l10n.authorizeSavedDirectories)),
              ],
              AppDisclosure(
                  title: context.l10n.searchHelp,
                  child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        Text(
                            context.l10n
                                .systemSearchShowsEarlyResultsFilenameScanning,
                            style: AppStyle.caption),
                        if (DesktopEnvironment.current.fileSearchAccess) ...[
                          const SizedBox(height: 8),
                          Text(
                              context
                                  .l10n.directoryAccessIsPreparedAtStartupAnd,
                              style: AppStyle.caption),
                          TextButton(
                              onPressed: onDiskAccess,
                              child: Text(context.l10n.fullDiskAccess)),
                        ],
                      ])),
              const SizedBox(height: 8),
              Align(
                  alignment: Alignment.centerRight,
                  child: FilledButton(
                      onPressed: onSave,
                      child: Text(context.l10n.saveFileSearch))),
            ])),
      ]);
}
