import 'dart:async';

import 'package:flutter/material.dart';

import '../../infrastructure/application_history_backend.dart';
import '../../src/rust/api.dart';
import '../../tools/object_action_coordinator.dart';
import '../../tools/tool_page.dart';
import 'page.dart';

class AppHistorySession extends ToolPageSession {
  AppHistorySession(this.backend,
      {required this.sourceId,
      required this.title,
      this.rowExtent = 64,
      this.localizedTitles = const {}}) {
    query.addListener(_changed);
    unawaited(refresh());
  }
  final AppHistoryQueryBackend backend;
  final String sourceId;
  final double rowExtent;
  final _actionRequests = ObjectActionCoordinator();
  @override
  final String title;
  final Map<String, String> localizedTitles;
  String labelFor(String language) => localizedTitles[language] ?? title;
  final query = TextEditingController();
  final focus = FocusNode(debugLabel: 'app-history-search');
  final scroll = ScrollController();
  List<AppHistoryItemDto> items = const [];
  List<CandidateDto> objectActions = const [];
  AppHistoryNoticeDto? notice;
  bool loading = true, busy = false, failed = false, copied = false;
  int selectedIndex = 0, _revision = 0;
  String _lastText = '';
  Timer? _debounce;
  bool _visible = true, _running = false, _again = false, _composing = false;
  bool _resetSelection = true;
  Future<void> _cancellation = Future.value();
  AppHistoryItemDto? get selected =>
      items.isEmpty ? null : items[selectedIndex];
  bool get interactive =>
      _visible && canInteract && !busy && query.value.composing.isCollapsed;
  @override
  String get id => 'application.history';
  @override
  double get width => 760;
  @override
  double get height => 520;

  void _invalidateActions() {
    _actionRequests.invalidate();
    objectActions = const [];
  }

  void _cancelRead() {
    // Every earlier cancel must finish before the next read. Awaiting only the
    // latest Future permits an older native callback to cancel that new read.
    _cancellation =
        _cancellation.then((_) => backend.cancel()).catchError((Object _) {});
  }

  void _changed() {
    if (disposed) return;
    final composing = !query.value.composing.isCollapsed;
    if (composing) {
      if (!_composing) {
        ++_revision;
        _invalidateActions();
        _debounce?.cancel();
        _cancelRead();
      }
      _composing = true;
      return;
    }
    final committed = _composing;
    _composing = false;
    if (!committed && query.text == _lastText) return;
    _lastText = query.text;
    _invalidateActions();
    ++_revision;
    _resetSelection = true;
    copied = false;
    _cancelRead();
    _debounce?.cancel();
    loading = true;
    notifyListeners();
    _debounce =
        Timer(const Duration(milliseconds: 120), () => unawaited(refresh()));
  }

  @override
  void setVisible(bool visible) {
    if (_visible == visible || disposed) return;
    _visible = visible;
    _invalidateActions();
    ++_revision;
    _debounce?.cancel();
    if (visible) {
      unawaited(refresh());
    } else {
      _cancelRead();
    }
  }

  Future<void> refresh() async {
    if (busy && _visible && canInteract) {
      _again = true;
      return;
    }
    if (!interactive) return;
    if (_running) {
      _again = true;
      return;
    }
    _running = true;
    _invalidateActions();
    loading = true;
    notifyListeners();
    do {
      _again = false;
      final revision = _revision;
      final text = query.text;
      try {
        await _cancellation;
        if (!interactive || revision != _revision) continue;
        var result = await backend.search(text, sourceId);
        for (var attempt = 0;
            attempt < 4 &&
                result.notice == AppHistoryNoticeDto.busy &&
                interactive &&
                revision == _revision;
            attempt++) {
          await Future<void>.delayed(const Duration(milliseconds: 80));
          if (interactive && revision == _revision) {
            result = await backend.search(text, sourceId);
          }
        }
        if (!interactive || revision != _revision) continue;
        final previous = _resetSelection ? null : selected?.objectKey;
        items = List.unmodifiable(result.items);
        final index = items.indexWhere((item) => item.objectKey == previous);
        selectedIndex = index < 0 ? 0 : index;
        _resetSelection = false;
        notice = result.notice;
        failed = false;
      } catch (_) {
        if (!disposed && revision == _revision) {
          failed = true;
          items = const [];
        }
      }
    } while (_again && interactive);
    _running = false;
    if (!disposed) {
      loading = _debounce?.isActive == true;
      notifyListeners();
      if (scroll.hasClients) {
        WidgetsBinding.instance.addPostFrameCallback((_) {
          if (!disposed && _visible) _revealSelection();
        });
      }
    }
  }

  void select(int index) {
    if (!interactive || items.isEmpty) return;
    selectedIndex = index.clamp(0, items.length - 1);
    _invalidateActions();
    copied = false;
    _revealSelection();
    notifyListeners();
  }

  void movePage(int direction) {
    final rows = scroll.hasClients
        ? (scroll.position.viewportDimension / rowExtent).floor().clamp(1, 100)
        : 6;
    select(selectedIndex + rows * direction);
  }

  Future<void> useSelection() async {
    final item = selected;
    if (item != null) await useItem(copy: !item.canOpen);
  }

  Future<bool> prepareActions() async {
    final item = selected, owner = host;
    if (!interactive || loading) return false;
    _invalidateActions();
    if (item == null || !item.canOpen || owner == null) return true;
    final revision = _revision;
    bool current() =>
        !disposed &&
        revision == _revision &&
        selected?.id == item.id &&
        _visible &&
        canInteract;
    busy = true;
    failed = false;
    notifyListeners();
    try {
      final actions = await _actionRequests.load(
          () => backend.actions(item.id,
              sessionId: owner.sessionId, revision: owner.revision),
          current);
      if (actions == null || !current()) {
        return false;
      }
      objectActions = List.unmodifiable(actions);
      return true;
    } catch (_) {
      if (current()) {
        // Unavailable enrichment must not hide Copy / Refresh recovery actions.
        failed = true;
        return true;
      }
      return false;
    } finally {
      _finishOperation();
    }
  }

  Future<void> runAction(CandidateDto action) async {
    final owner = host;
    if (!interactive ||
        loading ||
        owner == null ||
        !objectActions.any((a) => a.id == action.id)) {
      return;
    }
    busy = true;
    failed = false;
    notifyListeners();
    final revision = _revision;
    try {
      final succeeded = await owner.execute(action);
      if (!disposed && revision == _revision) failed = !succeeded;
    } catch (_) {
      if (!disposed && revision == _revision) failed = true;
    } finally {
      _finishOperation();
    }
  }

  void _finishOperation() {
    if (disposed) return;
    busy = false;
    notifyListeners();
    if (_again && interactive && _debounce?.isActive != true) {
      unawaited(refresh());
    }
  }

  void _revealSelection() {
    if (scroll.hasClients) {
      final top = selectedIndex * rowExtent;
      if (top < scroll.offset ||
          top + rowExtent > scroll.offset + scroll.position.viewportDimension) {
        scroll.jumpTo(top.clamp(0, scroll.position.maxScrollExtent));
      }
    }
  }

  Future<void> useItem({bool copy = false}) async {
    final item = selected;
    if (!interactive || loading || item == null || (!copy && !item.canOpen)) {
      return;
    }
    busy = true;
    failed = false;
    copied = false;
    notifyListeners();
    final revision = _revision;
    try {
      await backend.useItem(item.id, copy: copy);
      if (!disposed && revision == _revision) copied = copy;
    } catch (_) {
      if (!disposed && revision == _revision) failed = true;
    } finally {
      _finishOperation();
    }
  }

  @override
  Widget build(BuildContext context,
          {required VoidCallback onBack,
          required int presentation,
          required bool visible,
          String? notice,
          Future<void> Function()? onCopy}) =>
      AppHistoryPage(
          session: this,
          onBack: onBack,
          presentation: presentation,
          visible: visible,
          hostNotice: notice);
  @override
  void dispose() {
    ++_revision;
    _actionRequests.invalidate();
    _debounce?.cancel();
    unawaited(backend.close().catchError((Object _) {}));
    query.dispose();
    focus.dispose();
    scroll.dispose();
    super.dispose();
  }
}
