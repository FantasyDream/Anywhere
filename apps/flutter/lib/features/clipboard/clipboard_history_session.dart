import 'dart:async';
import 'package:flutter/material.dart';
import '../../src/rust/api.dart' as api;
import '../../infrastructure/clipboard_history_backend.dart';
import 'clipboard_history_page.dart';
import '../../tools/tool_page.dart';

/// Presentation state only. Persistence, matching, quotas and capture live in Rust.
class ClipboardHistorySession extends ToolPageSession {
  ClipboardHistorySession(this.backend) {
    query.addListener(_queryChanged);
    unawaited(refresh());
  }
  final ClipboardHistoryBackend backend;
  final query = TextEditingController();
  final focus = FocusNode(debugLabel: 'clipboard-history-search');
  final scroll = ScrollController();
  List<api.ClipboardItemDto> items = const [];
  api.ClipboardDetailDto? preview;
  bool recording = false, available = false, loading = true, mutating = false;
  String? notice, status;
  int selectedIndex = 0;
  int _searchEpoch = 0, _detailEpoch = 0;
  Timer? _debounce;
  Future<void>? _previewTask;
  String _lastQuery = '';
  api.ClipboardItemDto? get selected =>
      items.isEmpty ? null : items[selectedIndex];
  @override
  String get id => 'clipboard.history';
  @override
  String get title => 'Clipboard History';
  @override
  double get width => 880;
  @override
  double get height => 560;

  void _queryChanged() {
    // Focus and caret changes also notify TextEditingController. Only actual
    // query edits may clear results or invalidate the current selection.
    if (disposed || query.text == _lastQuery) return;
    _lastQuery = query.text;
    _debounce?.cancel();
    // Invalidate both response paths immediately, before the debounce fires.
    ++_searchEpoch;
    ++_detailEpoch;
    items = const [];
    preview = null;
    loading = true;
    notifyListeners();
    _debounce =
        Timer(const Duration(milliseconds: 120), () => unawaited(refresh()));
  }

  Future<void> refresh({bool background = false}) async {
    if (!canInteract ||
        (background && (loading || mutating || _debounce?.isActive == true))) {
      return;
    }
    final epoch = ++_searchEpoch;
    var changed = false;
    if (!background) {
      loading = true;
      notifyListeners();
    }
    try {
      final result = await backend.search(query.text);
      if (disposed || epoch != _searchEpoch) return;
      final unchanged = _sameItems(result.items);
      if (background &&
          unchanged &&
          recording == result.recording &&
          available == result.available &&
          notice == result.notice) {
        return;
      }
      changed = true;
      recording = result.recording;
      available = result.available;
      notice = result.notice;
      status = null;
      // A selection made while the search was running takes precedence.
      final selectedId = selected?.id;
      items = List.unmodifiable(result.items);
      final previous = items.indexWhere((item) => item.id == selectedId);
      selectedIndex = previous < 0 ? 0 : previous;
      if (preview == null || preview!.item.id != selected?.id) {
        await select(selectedIndex);
      }
    } catch (_) {
      if (disposed || epoch != _searchEpoch) return;
      // Keep usable rows and the current preview on a transient refresh failure.
      notice = 'Clipboard history could not be loaded. Try Refresh.';
      changed = true;
    } finally {
      if (!disposed && epoch == _searchEpoch) {
        loading = false;
        if (!background || changed) notifyListeners();
      }
    }
  }

  bool _sameItems(List<api.ClipboardItemDto> other) {
    if (items.length != other.length) return false;
    for (var i = 0; i < items.length; ++i) {
      final a = items[i], b = other[i];
      if (a.id != b.id ||
          a.copiedAtMs != b.copiedAtMs ||
          a.summary != b.summary ||
          a.kind != b.kind ||
          a.byteCount != b.byteCount) {
        return false;
      }
    }
    return true;
  }

  void reportNotice(String message) {
    if (disposed) return;
    notice = message;
    notifyListeners();
  }

  Future<void> select(int index) async {
    if (!canInteract) return;
    ++_detailEpoch;
    preview = null;
    if (items.isEmpty) {
      notifyListeners();
      return;
    }
    selectedIndex = index.clamp(0, items.length - 1);
    notifyListeners();
    if (scroll.hasClients) {
      final top = selectedIndex * 64.0;
      final bottom = top + 64;
      final offset = scroll.offset;
      if (top < offset || bottom > offset + scroll.position.viewportDimension) {
        scroll.jumpTo(
            (top < offset ? top : bottom - scroll.position.viewportDimension)
                .clamp(0, scroll.position.maxScrollExtent));
      }
    }
    // Only one detail request per presentation. Arrow repeats replace the
    // desired selection instead of queueing every intermediate image decode.
    return _previewTask ??= _drainPreviews();
  }

  Future<void> _drainPreviews() async {
    try {
      while (canInteract && items.isNotEmpty) {
        final epoch = _detailEpoch;
        final id = selected!.id;
        try {
          final detail = await backend.detail(id);
          if (!canInteract) return;
          if (epoch != _detailEpoch || selected?.id != id) continue;
          preview = detail;
          if (detail == null) {
            notice = 'This entry is no longer available. Refresh history.';
          }
        } catch (_) {
          if (!canInteract) return;
          if (epoch != _detailEpoch || selected?.id != id) continue;
          notice = 'This entry could not be previewed.';
        }
        notifyListeners();
        return;
      }
    } finally {
      _previewTask = null;
    }
  }

  Future<bool> _change(Future<void> Function() operation,
      {String? success, String? failure}) async {
    if (!canInteract || mutating) return false;
    mutating = true;
    status = null;
    notifyListeners();
    try {
      await operation();
      if (!canInteract) return false;
      await refresh();
      if (!canInteract) return false;
      if (success != null) {
        status = success;
        notice = null;
      }
      return true;
    } catch (_) {
      if (!disposed) {
        notice = failure ??
            'The operation could not be completed. Refresh and try again.';
      }
      return false;
    } finally {
      if (!disposed) {
        mutating = false;
        notifyListeners();
      }
    }
  }

  Future<bool> copy() async {
    final id = selected?.id;
    if (id != null) {
      return _change(() => backend.copy(id),
          success: 'Copied. Paste in the destination app.',
          failure:
              'This entry could not be copied. For files, check that the originals still exist and are accessible.');
    }
    return false;
  }

  Future<void> delete() async {
    final id = selected?.id;
    if (id != null) await _change(() => backend.delete(id));
  }

  @override
  Widget build(BuildContext context,
          {required VoidCallback onBack,
          required int presentation,
          required bool visible,
          String? notice,
          Future<void> Function()? onCopy}) =>
      ClipboardHistoryPage(
          session: this,
          onBack: onBack,
          presentation: presentation,
          visible: visible);

  @override
  void dispose() {
    _debounce?.cancel();
    ++_searchEpoch;
    ++_detailEpoch;
    query.dispose();
    focus.dispose();
    scroll.dispose();
    items = const [];
    preview = null;
    super.dispose();
  }
}
