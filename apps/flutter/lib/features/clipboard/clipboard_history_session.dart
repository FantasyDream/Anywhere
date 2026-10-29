import 'dart:async';

import 'package:flutter/material.dart';

import '../../infrastructure/clipboard_history_backend.dart';
import '../../infrastructure/native_preview.dart';
import '../../infrastructure/presentation_metrics.dart';
import '../../l10n/app_language.dart';
import '../../l10n/clipboard_labels.dart';
import '../../src/rust/api.dart' as api;
import '../../tools/tool_page.dart';
import '../../ui/input_focus.dart';
import 'clipboard_history_page.dart';
import 'clipboard_history_presentation.dart';

/// Presentation state only. Persistence, matching, quotas and capture live in Rust.
enum _HistoryNotice {
  loadFailed,
  missing,
  previewFailed,
  operationFailed,
  copyFailed,
  returnFailed,
  returnUnavailable
}

class ClipboardHistorySession extends ToolPageSession {
  ClipboardHistorySession(this.backend,
      {this.rowExtent = clipboardHistoryRowHeight,
      this.queryOperation = PresentationOperation.clipboardQuery}) {
    query.addListener(_queryChanged);
    unawaited(refresh());
  }
  final ClipboardHistoryBackend backend;
  final double rowExtent;
  final PresentationOperation queryOperation;
  final performance = PresentationMetrics();
  int _queryRevision = 0;
  final query = TextEditingController();
  final focus = FocusNode(debugLabel: 'clipboard-history-search');
  final scroll = ScrollController();
  List<api.ClipboardItemDto> items = const [];
  late final preview = NativePreviewController(
    onMove: (direction) => unawaited(select(selectedIndex + direction)),
    onActivate: () => unawaited(copy()),
    onSearch: () => requestTextInput(focus),
    onFailure: () {
      if (disposed) return;
      _setNotice(
          _HistoryNotice.previewFailed, 'This entry could not be previewed.');
      notifyListeners();
    },
  )..addListener(notifyListeners);
  bool recording = false, available = false, loading = true, mutating = false;
  String? _notice, status;
  api.ClipboardNoticeDto? _backendNotice;
  _HistoryNotice? _noticeKind;
  String? get notice => _notice;
  set notice(String? value) {
    _notice = value;
    _noticeKind = null;
    _backendNotice = null;
  }

  void _setNotice(_HistoryNotice kind, String value) {
    notice = value;
    _noticeKind = kind;
  }

  String? noticeFor(AppLocalizations l10n) => switch (_noticeKind) {
        _HistoryNotice.loadFailed => l10n.historyLoadFailed,
        _HistoryNotice.missing => l10n.historyEntryMissing,
        _HistoryNotice.previewFailed => l10n.historyPreviewFailed,
        _HistoryNotice.operationFailed => l10n.historyOperationFailed,
        _HistoryNotice.copyFailed => l10n.historyCopyFailed,
        _HistoryNotice.returnFailed => l10n.shelfReturnFailed,
        _HistoryNotice.returnUnavailable => l10n.shelfReturnUnavailable,
        null => _backendNotice?.labelFor(l10n) ?? notice,
      };
  String? statusFor(AppLocalizations l10n) =>
      status == null ? null : l10n.copiedPaste;
  int selectedIndex = -1;
  int _searchEpoch = 0;
  Future<void>? _refreshing;
  bool _refreshQueued = false;
  Timer? _debounce;
  String _lastQuery = '';
  bool _composing = false;
  api.ClipboardItemDto? get selected =>
      selectedIndex < 0 ? null : items[selectedIndex];
  @override
  String get id => 'clipboard.history';
  @override
  String get title => 'Clipboard History';
  @override
  double get width => 648;
  @override
  double get height => 600;

  void _queryChanged() {
    // Focus and caret changes also notify TextEditingController. Only actual
    // query edits may clear results or invalidate the current selection.
    if (disposed) return;
    if (!query.value.composing.isCollapsed) {
      if (!_composing) {
        _composing = true;
        performance.cancel();
        _debounce?.cancel();
        ++_searchEpoch;
        preview.edited();
        loading = false;
        notifyListeners();
      }
      return;
    }
    final committed = _composing;
    _composing = false;
    // Composition may have cancelled the debounce for this same text. Resume
    // it on commit even when the final characters did not change.
    if (!committed && query.text == _lastQuery) return;
    _lastQuery = query.text;
    status = null;
    _debounce?.cancel();
    // Invalidate both response paths immediately, before the debounce fires.
    ++_searchEpoch;
    preview.edited();
    performance.begin(queryOperation,
        generation: 0, revision: ++_queryRevision, presentation: 0);
    items = const [];
    selectedIndex = -1;
    _syncPreview();
    loading = true;
    notifyListeners();
    _debounce =
        Timer(const Duration(milliseconds: 120), () => unawaited(refresh()));
  }

  /// Measures metadata search only; explicit native previews have a separate lifetime.
  /// Includes debounce, backend matching and bridge time; never composition.
  void measureQueryFrame({
    required bool visible,
    required int presentation,
    required bool Function() current,
  }) {
    if (!visible || !canInteract) {
      performance.cancel();
      return;
    }
    if (loading || !available || _composing) return;
    final revision = _queryRevision;
    performance.ready(
        generation: 0,
        revision: revision,
        presentation: presentation,
        rows: items.length,
        afterFrame: (callback) =>
            WidgetsBinding.instance.addPostFrameCallback((_) => callback()),
        current: () =>
            !disposed &&
            canInteract &&
            !loading &&
            !_composing &&
            revision == _queryRevision &&
            current());
  }

  Future<void> refresh({bool background = false}) {
    if (!canInteract ||
        !query.value.composing.isCollapsed ||
        (background && (loading || mutating || _debounce?.isActive == true))) {
      return Future.value();
    }
    final pending = _refreshing;
    if (pending != null) {
      if (!background) {
        // Revoke the old result immediately; the next read uses the latest query.
        ++_searchEpoch;
        _refreshQueued = true;
        loading = true;
        notifyListeners();
      }
      return pending;
    }
    final done = Completer<void>();
    _refreshing = done.future;
    unawaited(_drainRefresh(background)
        .then(done.complete, onError: done.completeError));
    return done.future;
  }

  Future<void> _drainRefresh(bool background) async {
    try {
      do {
        _refreshQueued = false;
        await _readHistory(background: background);
        background = false;
      } while (_refreshQueued &&
          canInteract &&
          query.value.composing.isCollapsed &&
          _debounce?.isActive != true);
    } finally {
      _refreshQueued = false;
      _refreshing = null;
    }
  }

  Future<void> _readHistory({required bool background}) async {
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
          (_notice == null ||
              _noticeKind == _HistoryNotice.returnFailed ||
              _noticeKind == _HistoryNotice.returnUnavailable) &&
          _backendNotice == result.notice) {
        return;
      }
      changed = true;
      recording = result.recording;
      available = result.available;
      notice = null;
      _backendNotice = result.notice;
      status = null;
      // A selection made while the search was running takes precedence.
      final selectedId = selected?.id;
      items = List.unmodifiable(result.items);
      final previous = items.indexWhere((item) => item.id == selectedId);
      selectedIndex = previous;
      // Search returns metadata only; preparing a native preview is explicit.
      loading = false;
      _syncPreview();
    } catch (_) {
      if (disposed || epoch != _searchEpoch) return;
      performance.cancel();
      // Keep usable rows and the current preview on a transient refresh failure.
      _setNotice(_HistoryNotice.loadFailed,
          'Clipboard history could not be loaded. Try Refresh.');
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

  void reportReturnFailure({bool unavailable = false}) {
    if (disposed) return;
    _setNotice(
        unavailable
            ? _HistoryNotice.returnUnavailable
            : _HistoryNotice.returnFailed,
        'Copied, but the destination application could not be activated.');
    notifyListeners();
  }

  void _syncPreview() {
    final item = selected;
    preview.sync(canInteract && !_composing ? item?.id : null,
        item == null ? null : () => backend.preparePreview(item.id));
  }

  @override
  void setVisible(bool visible) {
    if (!visible) preview.close();
  }

  Future<void> select(int index) async {
    if (!canInteract || items.isEmpty) return;
    final next = index.clamp(0, items.length - 1);
    if (next != selectedIndex) status = null;
    selectedIndex = next;
    _syncPreview();
    _scrollSelection();
    notifyListeners();
  }

  void _scrollSelection() {
    if (scroll.hasClients) {
      final top = selectedIndex * rowExtent;
      final bottom = top + rowExtent;
      final offset = scroll.offset;
      if (top < offset || bottom > offset + scroll.position.viewportDimension) {
        scroll.jumpTo(
            (top < offset ? top : bottom - scroll.position.viewportDimension)
                .clamp(0, scroll.position.maxScrollExtent));
      }
    }
  }

  Future<bool> _change(Future<void> Function() operation,
      {String? success,
      String? failure,
      _HistoryNotice failureKind = _HistoryNotice.operationFailed,
      bool refreshAfter = true}) async {
    if (!canInteract || mutating) return false;
    preview.close();
    mutating = true;
    status = null;
    notifyListeners();
    try {
      await operation();
      if (!canInteract) return false;
      if (refreshAfter) await refresh();
      if (!canInteract) return false;
      if (success != null) {
        status = success;
        notice = null;
      }
      return true;
    } catch (_) {
      if (!disposed) {
        _setNotice(
            failureKind,
            failure ??
                'The operation could not be completed. Refresh and try again.');
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
          // Restoring a payload does not mutate history. Background collection
          // can update row metadata later without delaying the successful copy.
          refreshAfter: false,
          failureKind: _HistoryNotice.copyFailed,
          success: 'Copied. Paste in the destination app.',
          failure:
              'This entry could not be copied. For files, check that the originals still exist and are accessible.');
    }
    return false;
  }

  /// Explicit activation copies by identity. Do not wait for image preview I/O
  /// before restoring the payload; select updates the target synchronously.
  Future<bool> copyItem(String id) async {
    if (!canInteract || mutating) return false;
    final index = items.indexWhere((item) => item.id == id);
    if (index < 0) return false;
    unawaited(select(index));
    return copy();
  }

  Future<void> delete() async {
    final id = selected?.id;
    if (id != null) await deleteItem(id);
  }

  /// Confirmation targets an identity, even if collection refreshes the list.
  Future<bool> deleteItem(String id) async {
    if (!canInteract || mutating) return false;
    if (!items.any((item) => item.id == id)) {
      _setNotice(_HistoryNotice.missing,
          'This entry is no longer available. Refresh history.');
      notifyListeners();
      return false;
    }
    return _change(() => backend.delete(id));
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
    performance.cancel();
    _debounce?.cancel();
    ++_searchEpoch;
    preview.edited();
    preview.dispose();
    query.dispose();
    focus.dispose();
    scroll.dispose();
    items = const [];
    selectedIndex = -1;
    super.dispose();
  }
}
