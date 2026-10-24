import 'dart:async';

import 'package:flutter/material.dart';

import '../../infrastructure/application_history_backend.dart';
import '../../src/rust/api.dart';
import '../../tools/tool_page.dart';
import 'page.dart';

class AppHistorySession extends ToolPageSession {
  AppHistorySession(this.backend) {
    query.addListener(_changed);
    unawaited(refresh());
  }
  final AppHistoryQueryBackend backend;
  final query = TextEditingController();
  final focus = FocusNode(debugLabel: 'app-history-search');
  final scroll = ScrollController();
  List<AppHistoryItemDto> items = const [];
  List<AppHistorySourceDto> sources = const [];
  AppHistoryNoticeDto? notice;
  bool loading = true, busy = false, failed = false, copied = false;
  int selectedIndex = 0, _revision = 0;
  String filter = '', _lastText = '';
  Timer? _debounce;
  bool _visible = true, _running = false, _again = false, _composing = false;
  Future<void> _cancellation = Future.value();
  AppHistoryItemDto? get selected =>
      items.isEmpty ? null : items[selectedIndex];
  bool get interactive =>
      _visible && canInteract && !busy && query.value.composing.isCollapsed;
  @override
  String get id => 'application.history';
  @override
  String get title => 'App History';
  @override
  double get width => 760;
  @override
  double get height => 520;

  void _cancelRead() {
    _cancellation = backend.cancel().catchError((Object _) {});
  }

  void _changed() {
    if (disposed) return;
    final composing = !query.value.composing.isCollapsed;
    if (composing) {
      if (!_composing) {
        ++_revision;
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
    ++_revision;
    copied = false;
    _cancelRead();
    _debounce?.cancel();
    loading = true;
    notifyListeners();
    _debounce =
        Timer(const Duration(milliseconds: 120), () => unawaited(refresh()));
  }

  void setVisible(bool visible) {
    if (_visible == visible || disposed) return;
    _visible = visible;
    ++_revision;
    _debounce?.cancel();
    if (visible) {
      unawaited(refresh());
    } else {
      _cancelRead();
    }
  }

  Future<void> refresh() async {
    if (!interactive) return;
    if (_running) {
      _again = true;
      return;
    }
    _running = true;
    loading = true;
    notifyListeners();
    do {
      _again = false;
      final revision = _revision;
      final text = query.text, source = filter;
      try {
        await _cancellation;
        if (!interactive || revision != _revision) continue;
        final available = await backend.sources();
        if (!interactive || revision != _revision) continue;
        if (filter.isNotEmpty &&
            !available.any((s) => s.id == filter && s.path.isNotEmpty)) {
          filter = '';
          ++_revision;
          _again = true;
          continue;
        }
        var result = await backend.search(text, source);
        for (var attempt = 0;
            attempt < 4 &&
                result.notice == AppHistoryNoticeDto.busy &&
                interactive &&
                revision == _revision;
            attempt++) {
          await Future<void>.delayed(const Duration(milliseconds: 80));
          if (interactive && revision == _revision) {
            result = await backend.search(text, source);
          }
        }
        if (!interactive || revision != _revision) continue;
        final previous = selected?.objectKey;
        sources = available;
        items = List.unmodifiable(result.items);
        final index = items.indexWhere((item) => item.objectKey == previous);
        selectedIndex = index < 0 ? 0 : index;
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
    }
  }

  void select(int index) {
    if (!interactive || items.isEmpty) return;
    selectedIndex = index.clamp(0, items.length - 1);
    copied = false;
    if (scroll.hasClients) {
      final top = selectedIndex * 64.0;
      if (top < scroll.offset ||
          top + 64 > scroll.offset + scroll.position.viewportDimension) {
        scroll.jumpTo(top.clamp(0, scroll.position.maxScrollExtent));
      }
    }
    notifyListeners();
  }

  void selectFilter(String id) {
    if (!interactive || filter == id) return;
    filter = id;
    ++_revision;
    copied = false;
    _cancelRead();
    unawaited(refresh());
  }

  void cycleFilter(bool backwards) {
    final ids = [
      '',
      ...sources.where((s) => s.path.isNotEmpty).map((s) => s.id)
    ];
    selectFilter(
        ids[(ids.indexOf(filter) + (backwards ? -1 : 1)) % ids.length]);
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
      if (!disposed) {
        busy = false;
        notifyListeners();
      }
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
          visible: visible);
  @override
  void dispose() {
    ++_revision;
    _debounce?.cancel();
    unawaited(backend.close().catchError((Object _) {}));
    query.dispose();
    focus.dispose();
    scroll.dispose();
    super.dispose();
  }
}
