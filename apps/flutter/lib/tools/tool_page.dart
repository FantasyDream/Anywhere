import 'package:flutter/material.dart';
import '../src/rust/api.dart';

/// A native presentation session, owned by Launcher retention, not by a plugin loader.
abstract class ToolPageSession extends ChangeNotifier {
  String get id;
  String get title;
  String get subtitle => 'Tool';
  double get width => 648;
  double get height => 480;
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

/// Immutable UI catalogue. A new page contributes a native factory; Core supplies data.
class ToolPageRegistry {
  ToolPageRegistry(Iterable<ToolPageRegistration> pages) {
    for (final page in pages) {
      if (page.id.length > 100 ||
          !RegExp(r'^[a-z][a-z0-9_-]*(\.[a-z][a-z0-9_-]*)+$')
              .hasMatch(page.id) ||
          _pages.containsKey(page.id)) {
        throw ArgumentError('Invalid or duplicate tool page: ${page.id}');
      }
      _pages[page.id] = page;
    }
  }
  final Map<String, ToolPageRegistration> _pages = {};
  Set<HostViewDto> get hostViews =>
      _pages.values.map((p) => p.view).whereType<HostViewDto>().toSet();
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
