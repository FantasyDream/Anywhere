import 'dart:async';
import 'package:flutter/foundation.dart';
import '../src/rust/api.dart';
import 'execution_snapshot.dart';

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
  List<ExecutionSessionSummaryDto> _sessions = const [];
  List<ExecutionSessionSummaryDto> get sessions => _sessions;
  ExecutionSessionDetailDto? detail;
  String? selectedId;
  String? _actionError, _refreshError;
  String get filter => _filter;
  String _filter = '';
  String? get error => _actionError ?? _refreshError;
  List<ExecutionSessionSummaryDto> _visibleSessions = const [];
  List<ExecutionSessionSummaryDto> get visibleSessions => _visibleSessions;

  void _updateVisibleSessions() {
    final words = _filter.toLowerCase().trim().split(RegExp(r'\s+'));
    _visibleSessions = _filter.trim().isEmpty
        ? sessions
        : List.unmodifiable(sessions.where((task) {
            final text = '${task.title} ${task.directory}'.toLowerCase();
            return words.every(text.contains);
          }));
  }

  static const activePollInterval = Duration(milliseconds: 500);
  static const idlePollInterval = Duration(seconds: 2);
  Duration get pollInterval => sessions.any((task) => isTaskActive(task.status))
      ? activePollInterval
      : idlePollInterval;

  void _scheduleRefresh() {
    _timer?.cancel();
    if (_visible && !_disposed) {
      _timer = Timer(pollInterval, () => unawaited(refresh()));
    }
  }

  bool busy = false;
  bool _visible = false,
      _disposed = false,
      _refreshing = false,
      _queued = false;
  int _epoch = 0;
  Timer? _timer;

  void setVisible(bool visible) {
    if (_disposed || _visible == visible) {
      return;
    }
    _visible = visible;
    _epoch++;
    _timer?.cancel();
    _timer = null;
    if (visible) {
      unawaited(refresh());
    }
  }

  void select(String id) {
    if (_disposed) {
      return;
    }
    if (!visibleSessions.any((task) => task.id == id)) {
      _filter = '';
      _updateVisibleSessions();
    }
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
    _updateVisibleSessions();
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
    _timer?.cancel();
    final epoch = _epoch;
    var changed = false;
    try {
      final next = await backend.list();
      if (!_current(epoch)) {
        return;
      }
      final oldIndex =
          visibleSessions.indexWhere((task) => task.id == selectedId);
      changed = !sameTaskList(sessions, next);
      if (changed) {
        _sessions = List.unmodifiable(next);
        _updateVisibleSessions();
      }
      final visible = visibleSessions;
      final nextSelection = visible.any((session) => session.id == selectedId)
          ? selectedId
          : visible.isEmpty
              ? null
              : visible[oldIndex.clamp(0, visible.length - 1)].id;
      if (nextSelection != selectedId) {
        selectedId = nextSelection;
        detail = null;
        changed = true;
      }
      final id = selectedId;
      final summary = id == null ? null : _task(id);
      final needsDetail = id != null &&
          (detail == null || !sameTaskSummary(summary, detail?.summary));
      final nextDetail = id == null
          ? null
          : needsDetail
              ? await backend.detail(id)
              : detail;
      if (!_current(epoch) || id != selectedId) {
        return;
      }
      changed = changed || needsDetail || _refreshError != null;
      detail = nextDetail;
      _refreshError = null;
      if (changed) notifyListeners();
    } catch (_) {
      if (_current(epoch)) {
        const message = 'Tasks could not be refreshed. Try again.';
        if (_refreshError != message || changed) {
          _refreshError = message;
          notifyListeners();
        }
      }
    } finally {
      _refreshing = false;
      if (_queued) {
        _queued = false;
        unawaited(refresh());
      } else {
        _scheduleRefresh();
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
