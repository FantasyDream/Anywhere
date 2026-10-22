import 'package:flutter/material.dart';
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

  static const catalog = <String, ActionIconAppearance>{
    'terminal': ActionIconAppearance(AppIcons.terminal, DesignTokens.iconGreen),
    'folder': ActionIconAppearance(AppIcons.folder, DesignTokens.iconBlue),
    'folder-open':
        ActionIconAppearance(AppIcons.folderOpen, DesignTokens.iconBlue),
    'file': ActionIconAppearance(AppIcons.file, DesignTokens.iconNeutral),
    'file-open': ActionIconAppearance(AppIcons.file, DesignTokens.iconBlue,
        mark: AppIcons.external),
    'file-reveal': ActionIconAppearance(AppIcons.folder, DesignTokens.iconBlue,
        mark: AppIcons.search),
    'copy': ActionIconAppearance(AppIcons.copy, DesignTokens.iconNeutral),
    'copy-path': ActionIconAppearance(AppIcons.folder, DesignTokens.iconBlue,
        mark: AppIcons.copy),
    'copy-selection': ActionIconAppearance(
        AppIcons.files, DesignTokens.iconPurple,
        stamp: '{}'),
    'copy-url': ActionIconAppearance(AppIcons.link, DesignTokens.iconBlue,
        mark: AppIcons.copy),
    'globe': ActionIconAppearance(AppIcons.globe, DesignTokens.iconBlue),
    'code': ActionIconAppearance(AppIcons.code, DesignTokens.iconPurple),
    'git': ActionIconAppearance(AppIcons.git, DesignTokens.iconCoral),
    'git-status': ActionIconAppearance(AppIcons.status, DesignTokens.iconCoral),
    'git-log': ActionIconAppearance(AppIcons.history, DesignTokens.iconCoral),
    'git-diff': ActionIconAppearance(AppIcons.diff, DesignTokens.iconCoral),
    'git-staged': ActionIconAppearance(AppIcons.staged, DesignTokens.iconGreen),
    'git-branches': ActionIconAppearance(AppIcons.git, DesignTokens.iconCoral),
    'copy-branch': ActionIconAppearance(AppIcons.git, DesignTokens.iconCoral,
        mark: AppIcons.copy),
    'copy-commit': ActionIconAppearance(AppIcons.hash, DesignTokens.iconCoral,
        mark: AppIcons.copy),
    'copy-short-commit':
        ActionIconAppearance(AppIcons.hash, DesignTokens.iconCoral, stamp: '…'),
    'search': ActionIconAppearance(AppIcons.search, DesignTokens.iconNeutral),
    'file-search':
        ActionIconAppearance(AppIcons.fileSearch, DesignTokens.iconBlue),
    'clipboard':
        ActionIconAppearance(AppIcons.clipboard, DesignTokens.iconNeutral),
    'clipboard-history': ActionIconAppearance(
        AppIcons.clipboard, DesignTokens.iconPurple,
        mark: AppIcons.history),
    'clipboard-shelf':
        ActionIconAppearance(AppIcons.shelf, DesignTokens.iconAmber),
    'menu': ActionIconAppearance(AppIcons.menu, DesignTokens.iconNeutral),
    'app': ActionIconAppearance(AppIcons.apps, DesignTokens.iconBlue),
    'developer-check':
        ActionIconAppearance(AppIcons.checkCode, DesignTokens.iconGreen),
    'developer-test':
        ActionIconAppearance(AppIcons.test, DesignTokens.iconGreen),
    'developer-build':
        ActionIconAppearance(AppIcons.build, DesignTokens.iconAmber),
    'developer-package':
        ActionIconAppearance(AppIcons.package, DesignTokens.iconAmber),
    'developer-run': ActionIconAppearance(AppIcons.run, DesignTokens.iconGreen),
    'developer-script':
        ActionIconAppearance(AppIcons.terminal, DesignTokens.iconPurple),
  };

  static ActionIconAppearance forKey(String key, {bool application = false}) =>
      catalog[key] ??
      ActionIconAppearance(
          application ? AppIcons.apps : AppIcons.bolt, AppStyle.secondary);
}
