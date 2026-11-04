import '../ui/design_tokens.g.dart';
import 'launcher_view_state.dart';

/// Shared geometry for both Host sizing and Flutter rows.
abstract final class LauncherLayout {
  static const rowHeight = 56.0,
      inputHeight = DesignTokens.pageHeaderHeight,
      scopeHeight = DesignTokens.pageToolbarHeight,
      headerHeight = 24.0,
      navigationHeight = 108.0;
}

/// Pixel policy stays outside semantic state and does not resolve any labels.
extension LauncherViewLayout on LauncherViewState {
  double get width => toolPage?.width ?? 648;
  double get inputAreaHeight =>
      LauncherLayout.inputHeight + (isContent ? 24 : 0);
  double get selectedTop => selectedIndex * LauncherLayout.rowHeight;
  double get _outputHeight =>
      ((output?.split('\n').length ?? 0).clamp(3, 14) * 20.0) + 24;
  double get height {
    if (phase == LauncherPhase.settings) return 508;
    if (toolPage != null) return toolPage!.height;
    if (isFileSearch) return 600;
    if (navigationOpen) {
      return inputAreaHeight + 1 + LauncherLayout.navigationHeight;
    }
    final body = output != null
        ? _outputHeight
        : visibleEntries.isEmpty
            ? 64.0
            : (visibleEntries.length * LauncherLayout.rowHeight + 8).clamp(
                8, LauncherViewState.maxRows * LauncherLayout.rowHeight + 8);
    return (inputAreaHeight +
            (hasControls ? LauncherLayout.scopeHeight : 0) +
            (hasPanel || hasControls ? 1 : 0) +
            (hasNotice ? 44 : 0) +
            (hasPanel
                ? (hasResultHeading ? LauncherLayout.headerHeight : 0) + body
                : 0))
        .clamp(LauncherLayout.inputHeight, 520.0);
  }
}
