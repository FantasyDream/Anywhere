import 'dart:convert';

import '../../infrastructure/application_history_backend.dart';
import '../../src/rust/api.dart';
import '../../tools/tool_page.dart';
import 'session.dart';

/// Routing metadata is supplied by the source Action, never inferred from focus.
AppHistorySession openAppHistory(String input, AppHistoryQueryBackend backend) {
  final value = jsonDecode(input);
  if (value is! Map<String, dynamic> ||
      value['source_id'] is! String ||
      !RegExp(r'^[a-z][a-z0-9_.-]{0,63}$')
          .hasMatch(value['source_id'] as String) ||
      value['title'] is! String ||
      (value['title'] as String).trim().isEmpty ||
      value['localized_titles'] is! Map<String, dynamic>) {
    throw const FormatException('Invalid application history route');
  }
  return AppHistorySession(backend,
      sourceId: value['source_id'] as String,
      title: value['title'] as String,
      localizedTitles:
          Map<String, String>.from(value['localized_titles'] as Map));
}

final applicationHistoryPages = [
  ToolPageRegistration(
      id: 'application.history',
      view: HostViewDto.nativeTool,
      accepts: (result) => result.nativeTool?.id == 'application.history',
      open: (result) => openAppHistory(
          result.nativeTool!.input, RustAppHistoryQueryBackend())),
];
