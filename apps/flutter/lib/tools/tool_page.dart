import 'package:flutter/material.dart';
import '../src/rust/api.dart';

/// A native presentation session owned by its Launcher page or independent window.
abstract class ToolPageSession extends ChangeNotifier {
  String get id;
  String get title;
  String get subtitle => 'Tool';
  double get width => 648;
  double get height => 480;

  /// macOS platform views must be clipped by the native window, not a Flutter
  /// physical-shape layer, which can hide their content under Impeller.
  bool get embedsPlatformView => false;
  bool get disposed => _disposed;
  bool _disposed = false;
  bool Function()? _interaction;
  bool get canInteract => !disposed && (_interaction?.call() ?? true);
  void bindInteraction(bool Function() active) {
    _interaction = active;
  }

  PluginDetailDto? get legacyDetail => null;
  Widget build(BuildContext context,
      {required VoidCallback onBack,
      required int presentation,
      required bool visible,
      String? notice,
      Future<void> Function()? onCopy});
  @override
  void dispose() {
    _disposed = true;
    _interaction = null;
    super.dispose();
  }
}

class ToolPageRegistration {
  const ToolPageRegistration(
      {required this.id, required this.open, this.view, this.accepts});
  final String id;
  final HostViewDto? view;
  final bool Function(ActionResultDto)? accepts;
  final ToolPageSession Function(ActionResultDto) open;
}

/// Detached windows use the same typed Host result, without a Launcher session.
class ToolWindowRegistration {
  const ToolWindowRegistration({required this.id, required this.show});
  final String id;
  final Future<void> Function() show;
}

/// Immutable UI catalogue. A new page contributes a native factory; Core supplies data.
class ToolPageRegistry {
  ToolPageRegistry(Iterable<ToolPageRegistration> pages,
      {Iterable<ToolWindowRegistration> windows = const []}) {
    for (final page in pages) {
      if (page.id.length > 100 ||
          !RegExp(r'^[a-z][a-z0-9_-]*(\.[a-z][a-z0-9_-]*)+$')
              .hasMatch(page.id) ||
          _pages.containsKey(page.id)) {
        throw ArgumentError('Invalid or duplicate tool page: ${page.id}');
      }
      _pages[page.id] = page;
    }
    for (final window in windows) {
      if (!RegExp(r'^[a-z][a-z0-9_-]*(\.[a-z][a-z0-9_-]*)+$')
              .hasMatch(window.id) ||
          window.id.length > 100 ||
          _pages.containsKey(window.id) ||
          _windows.containsKey(window.id)) {
        throw ArgumentError('Invalid or duplicate tool window: ${window.id}');
      }
      _windows[window.id] = window;
    }
  }
  final Map<String, ToolWindowRegistration> _windows = {};
  Future<void> Function()? windowFor(ActionResultDto result) =>
      result.view == HostViewDto.nativeTool
          ? _windows[result.nativeTool?.id]?.show
          : null;
  final Map<String, ToolPageRegistration> _pages = {};
  Set<HostViewDto> get hostViews => {
        ..._pages.values.map((p) => p.view).whereType<HostViewDto>(),
        if (_windows.isNotEmpty) HostViewDto.nativeTool
      };
  ToolPageSession open(ActionResultDto result) {
    final matches = _pages.values
        .where(
            (p) => p.view == result.view && (p.accepts?.call(result) ?? true))
        .toList();
    if (matches.length != 1) {
      throw StateError('Tool page is unknown or ambiguous');
    }
    return matches.single.open(result);
  }

  ToolPageSession openResult(ActionResultDto result) {
    final page = _pages['result.detail'];
    if (page == null) throw StateError('Result page is unavailable');
    return page.open(result);
  }
}
