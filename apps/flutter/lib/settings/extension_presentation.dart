import 'package:flutter/widgets.dart';

import '../l10n/app_language.dart';
import '../src/rust/api.dart';
import '../ui/app_icons.dart';

enum ExtensionPhase {
  enabled,
  disabled,
  needsPermission,
  failed,
  unavailable,
  restartRequired
}

extension ExtensionPresentation on ExtensionStatusDto {
  IconData get glyph => !firstParty
      ? AppIcons.plugin
      : switch (id) {
          'clipboard' => AppIcons.history,
          'application_history' => AppIcons.history,
          'saved_directories' => AppIcons.folderOpen,
          'git' => AppIcons.git,
          'url' => AppIcons.link,
          'maven' => AppIcons.package,
          'gradle' => AppIcons.build,
          'node' => AppIcons.terminal,
          'rust' => AppIcons.code,
          'spring_boot' => AppIcons.run,
          _ => AppIcons.features,
        };
  ExtensionPhase get phase {
    if (active) return ExtensionPhase.enabled;
    if (!configurable) return ExtensionPhase.unavailable;
    if (!enabled) return ExtensionPhase.disabled;
    if (needsApproval) return ExtensionPhase.needsPermission;
    if (!firstParty && restartRequired) {
      return ExtensionPhase.restartRequired;
    }
    return ExtensionPhase.failed;
  }

  String stateLabelFor(AppLocalizations l10n) => switch (phase) {
        ExtensionPhase.enabled => l10n.enabled,
        ExtensionPhase.disabled => l10n.disabled,
        ExtensionPhase.needsPermission => l10n.awaitingPermission,
        ExtensionPhase.failed => l10n.couldNotStart,
        ExtensionPhase.unavailable => l10n.unavailable,
        ExtensionPhase.restartRequired => l10n.restartRequired,
      };
  bool get needsAttention =>
      phase == ExtensionPhase.failed || phase == ExtensionPhase.unavailable;

  String nameFor(AppLocalizations l10n) => !firstParty
      ? name
      : switch (id) {
          'clipboard' => l10n.clipboardHistory,
          'application_history' => l10n.appHistory,
          'saved_directories' => l10n.savedDirectories,
          _ => name,
        };
}
