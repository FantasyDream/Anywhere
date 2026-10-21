import 'dart:async';
import 'dart:collection';

/// Native resource responses can each contain up to 16 MiB. Admission is shared
/// across page replacements so a rapid reload cannot multiply in-flight buffers.
class WebResourcePool {
  WebResourcePool({this.running = 2, this.queued = 16});
  final int running, queued;
  int _active = 0;
  final _queue = Queue<(Object, Completer<bool>)>();
  Future<bool> acquire(Object owner) {
    if (_active < running) {
      ++_active;
      return Future.value(true);
    }
    if (_queue.length >= queued) return Future.value(false);
    final ready = Completer<bool>();
    _queue.add((owner, ready));
    return ready.future;
  }

  void release() {
    assert(_active > 0);
    if (_queue.isNotEmpty) {
      _queue.removeFirst().$2.complete(true);
    } else {
      --_active;
    }
  }

  void cancel(Object owner) {
    final removed = _queue.where((item) => identical(item.$1, owner)).toList();
    _queue.removeWhere((item) => identical(item.$1, owner));
    for (final item in removed) {
      item.$2.complete(false);
    }
  }
}
