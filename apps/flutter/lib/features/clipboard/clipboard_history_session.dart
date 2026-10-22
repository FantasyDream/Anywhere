import '../../l10n/app_language.dart';
import '../../l10n/clipboard_labels.dart';
import 'dart:async';
import 'package:flutter/material.dart';
import '../../src/rust/api.dart' as api;
import '../../infrastructure/clipboard_history_backend.dart';
import 'clipboard_history_page.dart';
import 'clipboard_history_presentation.dart';
import '../../tools/tool_page.dart';

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
      {this.rowExtent = clipboardHistoryRowHeight, bool previewEnabled = true})
      : _previewEnabled = previewEnabled {
    query.addListener(_queryChanged);
    unawaited(refresh());
  }
  final ClipboardHistoryBackend backend;
  final double rowExtent;
  bool _previewEnabled;
  bool get previewEnabled => _previewEnabled;
  final query = TextEditingController();
  final focus = FocusNode(debugLabel: 'clipboard-history-search');
  final scroll = ScrollController();
  List<api.ClipboardItemDto> items = const [];
  api.ClipboardDetailDto? preview;
  bool recording = false, available = false, loading = true, mutating = false;
  String? _notice, status;
  api.ClipboardNoticeDto? _backendNotice;
  _HistoryNotice? _noticeKind;
  String? get notice => _notice;
  bool get hasNotice => _backendNotice != null || _notice != null;
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
  int selectedIndex = 0;
  int _searchEpoch = 0, _detailEpoch = 0;
  Timer? _debounce;
  Future<void>? _previewTask;
  String _lastQuery = '';
  bool _composing = false;
  api.ClipboardItemDto? get selected =>
      items.isEmpty ? null : items[selectedIndex];
  @override
  String get id => 'clipboard.history';
  @override
  String get title => 'Clipboard History';
  @override
  double get width => 960;
  @override
  double get height => 600;

  void _queryChanged() {
    // Focus and caret changes also notify TextEditingController. Only actual
    // query edits may clear results or invalidate the current selection.
    if (disposed) return;
    if (!query.value.composing.isCollapsed) {
      if (!_composing) {
        _composing = true;
        _debounce?.cancel();
        ++_searchEpoch;
        ++_detailEpoch;
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
        !query.value.composing.isCollapsed ||
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
      selectedIndex = previous < 0 ? 0 : previous;
      // Row metadata is ready independently of image decoding. Keep both the
      // loading indicator and copy feedback independent of preview latency.
      loading = false;
      if (!unchanged && preview != null && preview!.item.id == selected?.id) {
        final cached = preview!;
        preview = api.ClipboardDetailDto(
            item: selected!,
            text: cached.text,
            imagePng: cached.imagePng,
            files: cached.files,
            formats: cached.formats);
      }
      if (_previewEnabled &&
          (preview == null || preview!.item.id != selected?.id)) {
        await select(selectedIndex);
      }
    } catch (_) {
      if (disposed || epoch != _searchEpoch) return;
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

  void setPreviewEnabled(bool enabled) {
    if (disposed || _previewEnabled == enabled) return;
    _previewEnabled = enabled;
    ++_detailEpoch;
    preview = null;
    notifyListeners();
    if (enabled) unawaited(select(selectedIndex));
  }

  Future<void> select(int index) async {
    if (!canInteract) return;
    if (items.isNotEmpty) {
      final next = index.clamp(0, items.length - 1);
      if (next == selectedIndex &&
          (preview?.item.id == items[next].id || _previewTask != null)) {
        _scrollSelection();
        return _previewTask;
      }
      if (next != selectedIndex) status = null;
    }
    ++_detailEpoch;
    preview = null;
    if (items.isEmpty) {
      notifyListeners();
      return;
    }
    selectedIndex = index.clamp(0, items.length - 1);
    notifyListeners();
    _scrollSelection();
    // Only one detail request per presentation. Arrow repeats replace the
    // desired selection instead of queueing every intermediate image decode.
    if (_previewEnabled) return _previewTask ??= _drainPreviews();
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

  Future<void> _drainPreviews() async {
    try {
      while (
          canInteract && !_composing && _previewEnabled && items.isNotEmpty) {
        final epoch = _detailEpoch;
        final id = selected!.id;
        try {
          final detail = await backend.detail(id);
          if (!canInteract || _composing || !_previewEnabled) return;
          if (epoch != _detailEpoch || selected?.id != id) continue;
          preview = detail;
          if (detail == null) {
            _setNotice(_HistoryNotice.missing,
                'This entry is no longer available. Refresh history.');
          }
        } catch (_) {
          if (!canInteract || _composing || !_previewEnabled) return;
          if (epoch != _detailEpoch || selected?.id != id) continue;
          _setNotice(_HistoryNotice.previewFailed,
              'This entry could not be previewed.');
        }
        notifyListeners();
        return;
      }
    } finally {
      _previewTask = null;
    }
  }

  Future<bool> _change(Future<void> Function() operation,
      {String? success,
      String? failure,
      _HistoryNotice failureKind = _HistoryNotice.operationFailed,
      bool refreshAfter = true}) async {
    if (!canInteract || mutating) return false;
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
