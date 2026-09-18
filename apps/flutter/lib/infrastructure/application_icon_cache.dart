import 'dart:async';
import 'package:flutter/foundation.dart';
import '../src/rust/api.dart';

typedef LoadIcon = Future<Uint8List?> Function(String candidateId);

/// A bounded cache whose pending requests belong to a single query/session epoch.
class ApplicationIconCache extends ChangeNotifier {
  ApplicationIconCache(this._load);
  final LoadIcon? _load;
  final _bytes = <String, Uint8List>{};
  final _pending = <String>{};
  final _failed = <String>{};
  int _epoch = 0;
  bool _disposed = false;
  Uint8List? bytesFor(String id) => _bytes[id];

  void retryFailures() {
    ++_epoch;
    _pending.clear();
    _failed.clear();
  }

  void clear() {
    retryFailures();
    _bytes.clear();
  }

  void request(Iterable<CandidateDto> candidates) {
    final load = _load;
    if (load == null || _disposed) return;
    for (final candidate in candidates) {
      final id = candidate.id;
      if (candidate.kind != 'application' ||
          _bytes.containsKey(id) ||
          _failed.contains(id) ||
          !_pending.add(id)) {
        continue;
      }
      final epoch = _epoch;
      unawaited(() async {
        Uint8List? bytes;
        try {
          bytes = await load(id);
        } catch (_) {
          bytes = null;
        }
        if (_disposed || epoch != _epoch) return;
        _pending.remove(id);
        if (bytes == null || bytes.isEmpty) {
          _failed.add(id);
        } else {
          _bytes[id] = bytes;
          while (_bytes.length > 128) {
            _bytes.remove(_bytes.keys.first);
          }
        }
        notifyListeners();
      }());
    }
  }

  @override
  void dispose() {
    _disposed = true;
    clear();
    super.dispose();
  }
}
