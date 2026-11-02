import 'package:flutter/material.dart';

import '../../l10n/app_language.dart';
import '../../ui/app_icons.dart';
import '../../ui/app_style.dart';

const clipboardHistoryRowHeight = 64.0;
const clipboardShelfRowHeight = 48.0;

// Display hints only: capture, classification and payload limits stay in Rust.
IconData clipboardKindIcon(String kind) => switch (kind) {
      'image' => AppIcons.image,
      'files' => AppIcons.files,
      'rich_text' => AppIcons.document,
      'binary' => AppIcons.file,
      _ => AppIcons.text,
    };
Color clipboardKindTint(String kind) => switch (kind) {
      'image' => DesignTokens.iconBlue,
      'files' => DesignTokens.iconAmber,
      'rich_text' => DesignTokens.iconPurple,
      'binary' => DesignTokens.iconNeutral,
      _ => DesignTokens.iconNeutral,
    };
String clipboardKindLabel(String kind, AppLocalizations l10n) => switch (kind) {
      'image' => l10n.image,
      'files' => l10n.files,
      'rich_text' => l10n.richText,
      'binary' => l10n.data,
      _ => l10n.text,
    };
String clipboardCopyDate(BuildContext context, int milliseconds,
    {bool compact = false}) {
  final date = DateTime.fromMillisecondsSinceEpoch(milliseconds);
  final localizations = MaterialLocalizations.of(context);
  final time = localizations.formatTimeOfDay(TimeOfDay.fromDateTime(date));
  if (compact && DateUtils.isSameDay(date, DateTime.now())) return time;
  return '${localizations.formatShortDate(date)} · $time';
}
