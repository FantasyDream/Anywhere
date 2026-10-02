import 'dart:async';
import 'package:flutter/foundation.dart';
import '../src/rust/api.dart';

bool isTaskActive(ExecutionSessionStatusDto status) =>
    status == ExecutionSessionStatusDto.running ||
    status == ExecutionSessionStatusDto.stopping;

class ExecutionSessionsBackend {
  ExecutionSessionsBackend({
    Future<List<ExecutionSessionSummaryDto>> Function()? list,
    Future<ExecutionSessionDetailDto?> Function(String)? detail,
    Future<void> Function(String)? stop,
    Future<void> Function(String)? remove,
  })  : list = list ?? executionSessions,
        detail = detail ?? ((id) => executionSession(id: id)),
        stop = stop ?? ((id) => stopExecutionSession(id: id)),
        remove = remove ?? ((id) => removeExecutionSession(id: id));
  final Future<List<ExecutionSessionSummaryDto>> Function() list;
  final Future<ExecutionSessionDetailDto?> Function(String) detail;
  final Future<void> Function(String) stop;
  final Future<void> Function(String) remove;
}

class ExecutionSessionsController extends ChangeNotifier {
  ExecutionSessionsController(this.backend);
  final ExecutionSessionsBackend backend;
  List<ExecutionSessionSummaryDto> sessions = const [];
  ExecutionSessionDetailDto? detail;
  String? selectedId;
  String? error;
  bool busy = false;
  bool _visible = false,
      _disposed = false,
      _refreshing = false,
      _queued = false;
  int _epoch = 0;
  Timer? _timer;

  void setVisible(bool visible) {
    if (_disposed) {
      return;
    }
    _visible = visible;
    _epoch++;
    _timer?.cancel();
    _timer = null;
    if (visible) {
      _timer = Timer.periodic(
          const Duration(milliseconds: 500), (_) => unawaited(refresh()));
      unawaited(refresh());
    }
  }

  void select(String id) {
    if (_disposed) {
      return;
    }
    selectedId = id;
    detail = null;
    _epoch++;
    notifyListeners();
    unawaited(refresh());
  }

  Future<void> refresh() async {
    if (!_visible || _disposed) {
      return;
    }
    if (_refreshing) {
      _queued = true;
      return;
    }
    _refreshing = true;
    final epoch = _epoch;
    try {
      final next = await backend.list();
      if (!_current(epoch)) {
        return;
      }
      sessions = next;
      if (!next.any((session) => session.id == selectedId)) {
        selectedId = next.firstOrNull?.id;
        detail = null;
      }
      final id = selectedId;
      final nextDetail = id == null ? null : await backend.detail(id);
      if (!_current(epoch) || id != selectedId) {
        return;
      }
      detail = nextDetail;
      error = null;
      notifyListeners();
    } catch (_) {
      if (_current(epoch)) {
        error = 'Tasks could not be refreshed. Try again.';
        notifyListeners();
      }
    } finally {
      _refreshing = false;
      if (_queued) {
        _queued = false;
        unawaited(refresh());
      }
    }
  }

  Future<void> stopSelected() => _act(backend.stop);
  Future<void> removeSelected() => _act(backend.remove);

  Future<void> _act(Future<void> Function(String) action) async {
    final id = selectedId;
    if (id == null || busy || _disposed) {
      return;
    }
    busy = true;
    notifyListeners();
    try {
      await action(id);
      if (_disposed) {
        return;
      }
      await refresh();
    } catch (_) {
      if (!_disposed) {
        error = 'The task could not be updated. Refresh and try again.';
      }
    } finally {
      if (!_disposed) {
        busy = false;
        notifyListeners();
      }
    }
  }

  bool _current(int epoch) => !_disposed && _visible && epoch == _epoch;

  @override
  void dispose() {
    _disposed = true;
    _epoch++;
    _timer?.cancel();
    super.dispose();
  }
}
