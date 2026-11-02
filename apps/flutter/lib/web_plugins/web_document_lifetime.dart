import '../src/rust/api.dart' show WebDocumentDto;

/// Owns document bindings across async open/reload/hide/dispose. Native view and
/// widget state stay in the surface; unclaimed late bindings are always closed.
class WebDocumentLifetime {
  WebDocumentLifetime({required this.close});
  final Future<void> Function(String handle) close;
  WebDocumentDto? _document;
  int _generation = 0;
  bool _disposed = false;
  WebDocumentDto? get document => _document;
  int invalidate() => ++_generation;
  bool current(int generation) => !_disposed && generation == _generation;
  bool owns(WebDocumentDto document) =>
      !_disposed && identical(_document, document);

  Future<bool> accept(WebDocumentDto document, int generation,
      {required bool interactive}) async {
    if (!current(generation) || !interactive) {
      await _close(document);
      return false;
    }
    final previous = _document;
    _document = document;
    if (previous != null && previous.handle != document.handle) {
      // Resume creates a new opaque runtime binding. The old binding may
      // already be revoked by the broker, but releasing it here keeps the
      // Flutter lifetime correct when a host implementation retains it.
      await _close(previous);
    }
    return true;
  }

  Future<void> _close(WebDocumentDto document) async {
    try {
      await close(document.handle);
    } catch (_) {
      // A newer binding or catalog revocation may already have invalidated it.
    }
  }

  /// Detach and close the current binding exactly once before replacing it.
  Future<void> closeCurrent() async {
    final document = _document;
    _document = null;
    if (document != null) await _close(document);
  }

  Future<void> dispose() async {
    _disposed = true;
    invalidate();
    await closeCurrent();
  }
}
