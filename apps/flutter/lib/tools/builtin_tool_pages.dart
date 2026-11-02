import '../features/application_history/registration.dart';
import '../features/clipboard/registration.dart';
import '../web_plugins/web_tool_session.dart';
import 'detail_tool.dart';
import 'tool_page.dart';

/// App-owned composition only; feature factories live with their presentations.
ToolPageRegistry builtinToolPages(
        {Future<void> Function(String)? showWindow}) =>
    ToolPageRegistry([
      ...detailPages,
      ...webToolPages,
      ...clipboardPages,
      ...applicationHistoryPages,
    ], windows: [
      ...clipboardWindows(showWindow),
    ]);
