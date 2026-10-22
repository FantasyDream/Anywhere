import 'dart:collection';

/// Immutable append-only snapshot. Shares complete chunks with the predecessor;
/// merging a tail copies at most one 256-item chunk, never the whole loaded list.
class StableList<T> extends ListBase<T> {
  StableList._(this.chunks) {
    var end = 0;
    for (final chunk in chunks) {
      end += chunk.length;
      _ends.add(end);
    }
  }
  factory StableList.from(Iterable<T> items) =>
      StableList<T>._(const []).append(items);
  static const chunkSize = 256;
  final List<List<T>> chunks;
  final List<int> _ends = [];
  @override
  int get length => _ends.isEmpty ? 0 : _ends.last;
  @override
  set length(int value) => throw UnsupportedError('Immutable snapshot');
  @override
  void operator []=(int index, T value) =>
      throw UnsupportedError('Immutable snapshot');
  @override
  T operator [](int index) {
    RangeError.checkValidIndex(index, this);
    var lo = 0, hi = _ends.length;
    while (lo < hi) {
      final mid = (lo + hi) ~/ 2;
      if (index < _ends[mid]) {
        hi = mid;
      } else {
        lo = mid + 1;
      }
    }
    return chunks[lo][index - (lo == 0 ? 0 : _ends[lo - 1])];
  }

  StableList<T> append(Iterable<T> items) {
    final next = [...chunks];
    var pending = <T>[];
    if (next.isNotEmpty && next.last.length < chunkSize) {
      pending.addAll(next.removeLast());
    }
    for (final item in items) {
      pending.add(item);
      if (pending.length == chunkSize) {
        next.add(List.unmodifiable(pending));
        pending = [];
      }
    }
    if (pending.isNotEmpty) next.add(List.unmodifiable(pending));
    return StableList._(List.unmodifiable(next));
  }

  @override
  Iterator<T> get iterator => chunks.expand((c) => c).iterator;
}
