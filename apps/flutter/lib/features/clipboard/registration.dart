import '../../infrastructure/clipboard_history_backend.dart';
import '../../src/rust/api.dart';
import '../../tools/tool_page.dart';
import 'clipboard_history_session.dart';

final clipboardPages = [
  ToolPageRegistration(
      id: 'clipboard.history',
      view: HostViewDto.nativeTool,
      accepts: (result) => result.nativeTool?.id == 'clipboard.history',
      open: (_) => ClipboardHistorySession(RustClipboardHistoryBackend())),
];

List<ToolWindowRegistration> clipboardWindows(
        Future<void> Function(String)? showWindow) =>
    [
      if (showWindow != null)
        ToolWindowRegistration(
            id: 'clipboard.shelf', show: () => showWindow('clipboard.shelf')),
    ];
