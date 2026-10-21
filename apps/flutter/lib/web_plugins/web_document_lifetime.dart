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
    _document = document;
    return true;
  }

  Future<void> _close(WebDocumentDto document) async {
    try {
      await close(document.handle);
    } catch (_) {
      // A newer binding or catalog revocation may already have invalidated it.
    }
  }

  Future<void> dispose() async {
    _disposed = true;
    invalidate();
    final document = _document;
    _document = null;
    if (document != null) await _close(document);
  }
}
