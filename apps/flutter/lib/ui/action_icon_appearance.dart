import 'package:material_ui/material_ui.dart';
import 'app_icons.dart';
import 'app_style.dart';

/// Presentation vocabulary selected by the owning feature's icon key, never
/// by a translated title, action ID or execution target.
class ActionIconAppearance {
  const ActionIconAppearance(this.glyph, this.tint, {this.mark, this.stamp});
  final IconData glyph;
  final Color tint;
  final IconData? mark;
  final String? stamp;

  static const _catalog = <String, _ActionIconDescriptor>{
    'terminal': _ActionIconDescriptor(AppIcons.terminal, _IconTone.green),
    'folder': _ActionIconDescriptor(AppIcons.folder, _IconTone.blue),
    'folder-open': _ActionIconDescriptor(AppIcons.folderOpen, _IconTone.blue),
    'file': _ActionIconDescriptor(AppIcons.file, _IconTone.neutral),
    'file-open': _ActionIconDescriptor(AppIcons.file, _IconTone.blue,
        mark: AppIcons.external),
    'file-reveal': _ActionIconDescriptor(AppIcons.folder, _IconTone.blue,
        mark: AppIcons.search),
    'copy': _ActionIconDescriptor(AppIcons.copy, _IconTone.neutral),
    'copy-path': _ActionIconDescriptor(AppIcons.folder, _IconTone.blue,
        mark: AppIcons.copy),
    'copy-selection':
        _ActionIconDescriptor(AppIcons.files, _IconTone.purple, stamp: '{}'),
    'copy-url': _ActionIconDescriptor(AppIcons.link, _IconTone.blue,
        mark: AppIcons.copy),
    'globe': _ActionIconDescriptor(AppIcons.globe, _IconTone.blue),
    'code': _ActionIconDescriptor(AppIcons.code, _IconTone.purple),
    'git': _ActionIconDescriptor(AppIcons.git, _IconTone.coral),
    'git-status': _ActionIconDescriptor(AppIcons.status, _IconTone.coral),
    'git-log': _ActionIconDescriptor(AppIcons.history, _IconTone.coral),
    'git-diff': _ActionIconDescriptor(AppIcons.diff, _IconTone.coral),
    'git-staged': _ActionIconDescriptor(AppIcons.staged, _IconTone.green),
    'git-branches': _ActionIconDescriptor(AppIcons.git, _IconTone.coral),
    'copy-branch': _ActionIconDescriptor(AppIcons.git, _IconTone.coral,
        mark: AppIcons.copy),
    'copy-commit': _ActionIconDescriptor(AppIcons.hash, _IconTone.coral,
        mark: AppIcons.copy),
    'copy-short-commit':
        _ActionIconDescriptor(AppIcons.hash, _IconTone.coral, stamp: '…'),
    'search': _ActionIconDescriptor(AppIcons.search, _IconTone.neutral),
    'file-search': _ActionIconDescriptor(AppIcons.fileSearch, _IconTone.blue),
    'clipboard': _ActionIconDescriptor(AppIcons.clipboard, _IconTone.neutral),
    'clipboard-history': _ActionIconDescriptor(
        AppIcons.clipboard, _IconTone.purple,
        mark: AppIcons.history),
    'clipboard-shelf': _ActionIconDescriptor(AppIcons.shelf, _IconTone.amber),
    'menu': _ActionIconDescriptor(AppIcons.menu, _IconTone.neutral),
    'app': _ActionIconDescriptor(AppIcons.apps, _IconTone.blue),
    'developer-check':
        _ActionIconDescriptor(AppIcons.checkCode, _IconTone.green),
    'developer-test': _ActionIconDescriptor(AppIcons.test, _IconTone.green),
    'developer-build': _ActionIconDescriptor(AppIcons.build, _IconTone.amber),
    'developer-package':
        _ActionIconDescriptor(AppIcons.package, _IconTone.amber),
    'developer-run': _ActionIconDescriptor(AppIcons.run, _IconTone.green),
    'developer-script':
        _ActionIconDescriptor(AppIcons.terminal, _IconTone.purple),
  };

  static ActionIconAppearance forKey(String key, {bool application = false}) {
    final descriptor = _catalog[key];
    if (descriptor == null) {
      return ActionIconAppearance(
          application ? AppIcons.apps : AppIcons.bolt, AppStyle.secondary);
    }
    return ActionIconAppearance(descriptor.glyph, _color(descriptor.tone),
        mark: descriptor.mark, stamp: descriptor.stamp);
  }

  static Color _color(_IconTone tone) => switch (tone) {
        _IconTone.blue => AppStyle.iconBlue,
        _IconTone.green => AppStyle.iconGreen,
        _IconTone.coral => AppStyle.iconCoral,
        _IconTone.purple => AppStyle.iconPurple,
        _IconTone.amber => AppStyle.iconAmber,
        _IconTone.neutral => AppStyle.iconNeutral,
      };
}

enum _IconTone { blue, green, coral, purple, amber, neutral }

final class _ActionIconDescriptor {
  const _ActionIconDescriptor(this.glyph, this.tone, {this.mark, this.stamp});
  final IconData glyph;
  final _IconTone tone;
  final IconData? mark;
  final String? stamp;
}
