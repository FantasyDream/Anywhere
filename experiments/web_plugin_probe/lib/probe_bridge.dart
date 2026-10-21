import 'dart:convert';

/// A deliberately unprivileged protocol probe, not the production capability broker.
/// Origin/frame come from WebMessageListener's native metadata, never JSON fields.
class ProbeBridge {
  ProbeBridge(this.epoch);
  static final origin = Uri.parse('anywhere-plugin://probe.local');
  static const maxFrameBytes = 64 * 1024;
  final int epoch;
  bool interactive = true;
  bool _disposed = false;
  bool _navigated = false;
  final Set<String> _ids = {};

  // Each binding gets one document. Reload must create a new native view/binding;
  // a same-origin page must not silently inherit the previous document's epoch.
  bool permitNavigation(Uri? target, {required bool isMainFrame}) {
    if (_disposed ||
        !interactive ||
        _navigated ||
        !isMainFrame ||
        target?.toString() != 'anywhere-plugin://probe.local/index.html') {
      return false;
    }
    _navigated = true;
    return true;
  }

  bool permitsOrigin(Uri? value) =>
      value != null &&
      value.scheme == origin.scheme &&
      value.host == origin.host &&
      value.userInfo.isEmpty &&
      !value.hasPort &&
      (value.path.isEmpty || value.path == '/') &&
      !value.hasQuery &&
      !value.hasFragment;

  Map<String, Object?> handle(String raw,
      {required Uri? sourceOrigin, required bool isMainFrame}) {
    String? id;
    Map<String, Object?> error(String code) => {
          'requestId': id,
          'error': {'code': code}
        };
    if (_disposed || !interactive) return error('INACTIVE');
    if (!isMainFrame || !permitsOrigin(sourceOrigin)) {
      return error('UNTRUSTED_DOCUMENT');
    }
    if (raw.length > maxFrameBytes || utf8.encode(raw).length > maxFrameBytes) {
      return error('FRAME_TOO_LARGE');
    }
    Object? decoded;
    try {
      decoded = jsonDecode(raw);
    } on FormatException {
      return error('INVALID_REQUEST');
    }
    if (decoded is! Map<String, dynamic>) return error('INVALID_REQUEST');
    if (decoded.length != 4 ||
        decoded.keys.toSet().difference(
            {'protocol', 'requestId', 'method', 'params'}).isNotEmpty ||
        decoded['protocol'] != 1 ||
        decoded['requestId'] is! String) {
      return error('INVALID_REQUEST');
    }
    id = decoded['requestId'] as String;
    if (!RegExp(r'^[a-zA-Z0-9_-]{1,64}$').hasMatch(id)) {
      return error('INVALID_REQUEST');
    }
    if (_ids.contains(id)) return error('DUPLICATE_REQUEST');
    // A short-lived probe has a fixed call allowance, not an unbounded replay set.
    if (_ids.length >= 128) return error('QUOTA_EXCEEDED');
    _ids.add(id);
    if (decoded['method'] != 'probe.echo') return error('METHOD_NOT_ALLOWED');
    final params = decoded['params'];
    if (params is! Map<String, dynamic> ||
        params.length != 1 ||
        params['text'] is! String ||
        utf8.encode(params['text'] as String).length > 4096) {
      return error('INVALID_PARAMS');
    }
    return {
      'requestId': id,
      'result': {'text': params['text'], 'epoch': epoch}
    };
  }

  void dispose() {
    _disposed = true;
    interactive = false;
    _ids.clear();
  }
}
