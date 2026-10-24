import '../../infrastructure/application_history_backend.dart';
import '../../src/rust/api.dart';
import '../../tools/tool_page.dart';
import 'session.dart';

final applicationHistoryPages = [
  ToolPageRegistration(
      id: 'application.history',
      view: HostViewDto.nativeTool,
      accepts: (result) => result.nativeTool?.id == 'application.history',
      open: (_) => AppHistorySession(RustAppHistoryQueryBackend())),
];
