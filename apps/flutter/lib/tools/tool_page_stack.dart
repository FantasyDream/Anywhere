import 'tool_page.dart';

/// Owns retained parent pages and disposes each session when it leaves the stack.
/// Execution eligibility and window presentation remain with the Launcher.
class ToolPageStack {
  ToolPageSession? _current;
  final List<ToolPageSession> _parents = [];
  ToolPageSession? get current => _current;

  void replace(ToolPageSession? page) {
    if (identical(_current, page)) return;
    _current?.dispose();
    for (final parent in _parents) {
      parent.dispose();
    }
    _parents.clear();
    _current = page;
  }

  void push(ToolPageSession page) {
    if (_parents.length >= 4) {
      page.dispose();
      throw StateError('Too many nested tool pages');
    }
    final previous = _current;
    if (previous != null) {
      previous.setVisible(false);
      _parents.add(previous);
    }
    _current = page;
  }

  void back() {
    _current?.dispose();
    _current = _parents.isEmpty ? null : _parents.removeLast();
    _current?.setVisible(true);
  }
}
