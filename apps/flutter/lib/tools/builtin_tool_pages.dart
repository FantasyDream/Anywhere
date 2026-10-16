import 'detail_tool.dart';
import 'tool_page.dart';

/// App-owned static pages. Third-party WebView registration is deferred.
ToolPageRegistry builtinToolPages() =>
    ToolPageRegistry(detailPages);
