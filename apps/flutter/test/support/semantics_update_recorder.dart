import 'dart:ui' as ui;

import 'package:flutter_test/flutter_test.dart';

/// Replays the native bridge contract on the serialized incremental updates.
/// An update may keep unchanged nodes, but cannot publish an unclaimed child.
class SemanticsUpdateRecorder {
  final Map<int, List<int>> _children = {};
  final List<String> failures = [];

  void record(Map<int, List<int>> updates) {
    _children.addAll(updates);
    final reachable = <int>{};
    final pending = [0];
    while (pending.isNotEmpty) {
      final id = pending.removeLast();
      if (!reachable.add(id)) continue;
      pending.addAll(_children[id] ?? const []);
    }
    final missing = reachable.difference(_children.keys.toSet());
    final orphaned = updates.keys.toSet().difference(reachable);
    if (missing.isNotEmpty || orphaned.isNotEmpty) {
      failures.add('missing children: $missing; orphaned updates: $orphaned');
    }
    _children.removeWhere((id, _) => !reachable.contains(id));
  }
}

class SemanticsRecordingBinding extends AutomatedTestWidgetsFlutterBinding {
  SemanticsUpdateRecorder? recorder;

  @override
  ui.SemanticsUpdateBuilder createSemanticsUpdateBuilder() =>
      _RecordingBuilder(super.createSemanticsUpdateBuilder(), recorder);
}

class _RecordingBuilder implements ui.SemanticsUpdateBuilder {
  _RecordingBuilder(this.delegate, this.recorder);
  final ui.SemanticsUpdateBuilder delegate;
  final SemanticsUpdateRecorder? recorder;
  final Map<int, List<int>> updates = {};

  @override
  dynamic noSuchMethod(Invocation invocation) {
    if (invocation.memberName == #updateNode) {
      updates[invocation.namedArguments[#id] as int] = List<int>.of(
          invocation.namedArguments[#childrenInTraversalOrder] as List<int>);
      return Function.apply(delegate.updateNode, invocation.positionalArguments,
          invocation.namedArguments);
    }
    if (invocation.memberName == #updateCustomAction) {
      return Function.apply(delegate.updateCustomAction,
          invocation.positionalArguments, invocation.namedArguments);
    }
    return super.noSuchMethod(invocation);
  }

  @override
  ui.SemanticsUpdate build() {
    recorder?.record(updates);
    return delegate.build();
  }
}
