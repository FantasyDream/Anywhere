import 'package:flutter/material.dart';
import '../../ui/app_icons.dart';

const clipboardHistoryRowHeight = 64.0;
const clipboardShelfRowHeight = 48.0;

// Display hints only: capture, classification and payload limits stay in Rust.
IconData clipboardKindIcon(String kind) => switch (kind) {
      'image' => AppIcons.image,
      'files' => AppIcons.folder,
      'rich_text' => AppIcons.document,
      'binary' => AppIcons.file,
      _ => AppIcons.clipboard,
    };
String clipboardKindLabel(String kind) => switch (kind) {
      'image' => 'Image',
      'files' => 'Files',
      'rich_text' => 'Rich text',
      'binary' => 'Data',
      _ => 'Text',
    };
String clipboardDataSize(int bytes) => bytes >= 1024 * 1024
    ? '${(bytes / (1024 * 1024)).toStringAsFixed(1)} MB'
    : bytes >= 1024
        ? '${(bytes / 1024).toStringAsFixed(1)} KB'
        : '$bytes B';
String clipboardCopyDate(BuildContext context, int milliseconds,
    {bool compact = false}) {
  final date = DateTime.fromMillisecondsSinceEpoch(milliseconds);
  final localizations = MaterialLocalizations.of(context);
  final time = localizations.formatTimeOfDay(TimeOfDay.fromDateTime(date));
  if (compact && DateUtils.isSameDay(date, DateTime.now())) return time;
  return '${localizations.formatShortDate(date)} · $time';
}

String clipboardFileName(String path) =>
    path.substring(path.lastIndexOf(RegExp(r'[/\\]')) + 1);
