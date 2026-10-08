import 'dart:async';
import 'dart:collection';
import 'package:flutter/foundation.dart';
import '../src/rust/api.dart';

typedef LoadIcon = Future<Uint8List?> Function(String candidateId);

/// Bounded icon cache. Only the requested rows are pinned, and the cap always holds.
class ApplicationIconCache extends ChangeNotifier {
  ApplicationIconCache(this._load,
      {this.byteBudget = 2 * 1024 * 1024, this.maxEntryBytes = 256 * 1024})
      : assert(byteBudget > 0),
        assert(maxEntryBytes > 0);
  final int byteBudget, maxEntryBytes;
  int _retainedBytes = 0;
  int get retainedBytes => _retainedBytes;
  int get entryCount => _bytes.length;
  static const capacity = 128;
  static const maxInFlight = 4;
  final LoadIcon? _load;
  final _bytes = <String, Uint8List>{};
  final _pending = <String>{};
  final _failed = <String>{};
  final _queue = Queue<String>();
  var _pinned = <String>{};
  var _active = 0;
  var _epoch = 0;
  var _disposed = false;

  Uint8List? bytesFor(String id) {
    final bytes = _bytes.remove(id);
    if (bytes != null) _bytes[id] = bytes;
    return bytes;
  }

  void retryFailures() {
    _failed.clear();
  }

  void clear() {
    ++_epoch;
    _pending.clear();
    _failed.clear();
    _queue.clear();
    _bytes.clear();
    _pinned.clear();
    _retainedBytes = 0;
    _active = 0;
  }

  void request(Iterable<CandidateDto> candidates) {
    final load = _load;
    if (load == null || _disposed) return;
    final wanted = <String>[
      for (final candidate in candidates)
        if (candidate.kind == CandidateKindDto.application) candidate.id,
    ];
    _pinned = wanted.toSet();
    _queue.removeWhere((id) {
      if (_pinned.contains(id)) return false;
      _pending.remove(id);
      return true;
    });
    for (final id in wanted) {
      if (_bytes.containsKey(id) || _failed.contains(id) || !_pending.add(id)) {
        continue;
      }
      _queue.add(id);
    }
    _pump();
  }

  void _pump() {
    final load = _load;
    if (load == null || _disposed) return;
    while (_active < maxInFlight && _queue.isNotEmpty) {
      final id = _queue.removeFirst();
      if (!_pending.contains(id) ||
          _bytes.containsKey(id) ||
          _failed.contains(id)) {
        _pending.remove(id);
        continue;
      }
      final epoch = _epoch;
      _active++;
      unawaited(() async {
        Uint8List? bytes;
        try {
          bytes = await load(id);
        } catch (_) {
          bytes = null;
        }
        if (_disposed || epoch != _epoch) {
          _pump();
          return;
        }
        _active--;
        _pending.remove(id);
        if (bytes == null ||
            bytes.isEmpty ||
            bytes.lengthInBytes > maxEntryBytes ||
            bytes.lengthInBytes > byteBudget) {
          _failed.add(id);
          if (_failed.length > capacity) _failed.remove(_failed.first);
        } else {
          _store(id, bytes);
        }
        notifyListeners();
        _pump();
      }());
    }
  }

  void _store(String id, Uint8List bytes) {
    _retainedBytes -= _bytes.remove(id)?.lengthInBytes ?? 0;
    _bytes[id] = bytes;
    _retainedBytes += bytes.lengthInBytes;
    while (_bytes.length > capacity || _retainedBytes > byteBudget) {
      String? evict;
      for (final key in _bytes.keys) {
        if (!_pinned.contains(key)) {
          evict = key;
          break;
        }
      }
      evict ??= _bytes.keys.first;
      _retainedBytes -= _bytes.remove(evict)!.lengthInBytes;
    }
  }

  @override
  void dispose() {
    _disposed = true;
    clear();
    super.dispose();
  }
}
