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
  String? _actionError, _refreshError;
  String get filter => _filter;
  String _filter = '';
  String? get error => _actionError ?? _refreshError;
  List<ExecutionSessionSummaryDto> get visibleSessions {
    final words = _filter.toLowerCase().trim().split(RegExp(r'\s+'));
    return sessions
        .where((task) => words.every((word) =>
            '${task.title} ${task.directory}'.toLowerCase().contains(word)))
        .toList();
  }

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
    if (!visibleSessions.any((task) => task.id == id)) _filter = '';
    if (selectedId == id && detail != null) return;
    selectedId = id;
    detail = null;
    _epoch++;
    notifyListeners();
    unawaited(refresh());
  }

  void setFilter(String value) {
    if (_disposed || _filter == value) return;
    _filter = value;
    final visible = visibleSessions;
    if (!visible.any((task) => task.id == selectedId)) {
      selectedId = visible.firstOrNull?.id;
      detail = null;
      _epoch++;
      unawaited(refresh());
    }
    notifyListeners();
  }

  void moveSelection(int delta) {
    final visible = visibleSessions;
    if (visible.isEmpty) return;
    final index = visible.indexWhere((task) => task.id == selectedId);
    select(visible[(index + delta).clamp(0, visible.length - 1)].id);
  }

  void dismissError() {
    _actionError = null;
    _refreshError = null;
    notifyListeners();
  }

  void reportError(String message) {
    if (_disposed) return;
    _actionError = message;
    notifyListeners();
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
      final oldIndex =
          visibleSessions.indexWhere((task) => task.id == selectedId);
      sessions = next;
      final visible = visibleSessions;
      if (!visible.any((session) => session.id == selectedId)) {
        selectedId = visible.isEmpty
            ? null
            : visible[oldIndex.clamp(0, visible.length - 1)].id;
        detail = null;
      }
      final id = selectedId;
      final nextDetail = id == null ? null : await backend.detail(id);
      if (!_current(epoch) || id != selectedId) {
        return;
      }
      detail = nextDetail;
      _refreshError = null;
      notifyListeners();
    } catch (_) {
      if (_current(epoch)) {
        _refreshError = 'Tasks could not be refreshed. Try again.';
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

  ExecutionSessionSummaryDto? _task(String id) =>
      sessions.where((task) => task.id == id).firstOrNull;

  Future<void> stopSelected() async {
    final id = selectedId;
    if (id != null) await stop(id);
  }

  Future<void> removeSelected() async {
    final id = selectedId;
    if (id != null) await remove(id);
  }

  Future<void> stop(String id) async {
    if (_task(id)?.status == ExecutionSessionStatusDto.running) {
      await _act(id, backend.stop);
    }
  }

  Future<void> remove(String id) async {
    final task = _task(id);
    if (task != null && !isTaskActive(task.status)) {
      await _act(id, backend.remove);
    }
  }

  Future<void> _act(String id, Future<void> Function(String) action) async {
    if (busy || _disposed) {
      return;
    }
    busy = true;
    _actionError = null;
    notifyListeners();
    try {
      await action(id);
      if (_disposed) {
        return;
      }
      await refresh();
    } catch (_) {
      if (!_disposed) {
        _actionError = 'The task could not be updated. Refresh and try again.';
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
