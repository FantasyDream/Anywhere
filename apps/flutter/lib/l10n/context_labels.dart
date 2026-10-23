import 'dart:ui' show Locale;

import 'app_language.dart';

String contextTypeLabel(String id, [AppLocalizations? labels]) {
  final l10n = labels ?? lookupAppLocalizations(const Locale('en'));
  return switch (id) {
    'core.directory' => l10n.folder,
    'core.file' => l10n.file,
    'core.file_selection' => l10n.selectedFiles,
    'core.text' => l10n.text,
    'core.url' => l10n.webLink,
    'core.json' || 'json.document' => 'JSON',
    'git.repository' => l10n.gitRepository,
    'project.node' => l10n.nodeProject,
    'project.maven' => l10n.mavenProject,
    'project.gradle' => l10n.gradleProject,
    'project.rust' => l10n.rustProject,
    'java.spring_boot' => l10n.springProject,
    'core.application' => l10n.currentApp,
    'core.launcher' => l10n.anywhereTool,
    _ => id,
  };
}
