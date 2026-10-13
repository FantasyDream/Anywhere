import '../infrastructure/desktop_environment.dart';
import '../ui/app_icons.dart';
import 'package:flutter/material.dart';
import '../ui/app_style.dart';
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
            title: 'File search',
            description: 'Search file names in the File Search action',
            value: enabled,
            onChanged: onToggle),
        const Divider(),
        const Padding(
            padding: EdgeInsets.symmetric(horizontal: 12, vertical: 8),
            child: Text(
                'System search shows early results. Filename scanning uses accessible, authorized directories.',
                style: AppStyle.caption)),
        SettingsToggleRow(
            title: 'Include hidden files',
            description: 'Include dotfiles and files in hidden directories',
            value: includeHidden,
            onChanged: onHidden),
        Padding(
            padding: const EdgeInsets.all(12),
            child:
                Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
              const Text('Search directories', style: AppStyle.caption),
              const SizedBox(height: 6),
              TextField(
                  key: const Key('file-search-roots'),
                  controller: roots,
                  minLines: 2,
                  maxLines: 4,
                  style: const TextStyle(fontSize: 12),
                  decoration: InputDecoration(
                      isDense: true,
                      hintText:
                          '${switch (DesktopEnvironment.current.platform) {
                        TargetPlatform.windows => r'C:\Users\you\Documents',
                        TargetPlatform.macOS => '/Users/you/Documents',
                        _ => '/home/you/Documents',
                      }}\nLeave empty to search your home directory')),
              const SizedBox(height: 12),
              const Text('Excluded directory names', style: AppStyle.caption),
              const SizedBox(height: 6),
              TextField(
                  key: const Key('file-search-exclusions'),
                  controller: exclusions,
                  minLines: 2,
                  maxLines: 4,
                  style: const TextStyle(fontSize: 12),
                  decoration: const InputDecoration(
                      isDense: true, hintText: 'One directory name per line')),
              const SizedBox(height: 8),
              const Text(
                  'Files and folders are scanned, including gitignored files. Exclusions narrow the search range. Symbolic links are listed without scanning their targets.',
                  style: AppStyle.caption),
              const SizedBox(height: 10),
              if (DesktopEnvironment.current.fileSearchAccess) ...[
                const Text(
                    'Directory access is prepared at startup and when the search range changes. Protected locations are skipped. Full Disk Access is optional and must be enabled in System Settings.',
                    style: AppStyle.caption),
                const SizedBox(height: 8),
                Wrap(spacing: 6, runSpacing: 6, children: [
                  OutlinedButton(
                      onPressed: onAuthorize,
                      child: const Text('Authorize saved directories')),
                  TextButton(
                      onPressed: onDiskAccess,
                      child: const Text('Full Disk Access…')),
                ]),
                const SizedBox(height: 10),
              ],
              Align(
                  alignment: Alignment.centerRight,
                  child: FilledButton(
                      onPressed: onSave,
                      child: const Text('Save file search'))),
            ])),
      ]);
}
