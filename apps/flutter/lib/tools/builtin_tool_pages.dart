import 'detail_tool.dart';
import 'tool_page.dart';
import '../features/clipboard/registration.dart';

/// App-owned composition only; feature factories live with their presentations.
ToolPageRegistry builtinToolPages(
        {Future<void> Function(String)? showWindow}) =>
    ToolPageRegistry([
      ...detailPages,
      ...clipboardPages,
    ], windows: [
      ...clipboardWindows(showWindow),
    ]);
