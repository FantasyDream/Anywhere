import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter/widgets.dart';

import 'host_settings.dart';

/// Search/exclusion drafts and their serialized writes, independent of the tab.
class SettingsEdits extends ChangeNotifier {
  SettingsEdits({
    required HostSettings Function() read,
    required Future<bool> Function(HostSettings) save,
    required VoidCallback reportFailure,
  })  : _read = read,
        _save = save,
        _reportFailure = reportFailure {
    reset();
  }
  final HostSettings Function() _read;
  final Future<bool> Function(HostSettings) _save;
  final VoidCallback _reportFailure;
  final excluded = TextEditingController();
  final roots = TextEditingController();
  final exclusions = TextEditingController();
  bool busy = false, _disposed = false;
  Future<bool>? _pending;

  List<String> _lines(String text) => text
      .split('\n')
      .map((s) => s.trim())
      .where((s) => s.isNotEmpty)
      .toSet()
      .toList();
  List<String> _excludedIds(String text) => text
      .split(RegExp(r'[\n,]'))
      .map((s) => s.trim())
      .where((s) => s.isNotEmpty)
      .toSet()
      .toList();
  List<String> get excludedIds => _excludedIds(excluded.text);
  List<String> get rootValues => _lines(roots.text);
  List<String> get exclusionValues => _lines(exclusions.text);
  bool get hasDrafts =>
      !listEquals(excludedIds, _read().excludedApps) ||
      !listEquals(rootValues, _read().fileSearchRoots) ||
      !listEquals(exclusionValues, _read().fileSearchExcludedNames);

  void reset() {
    excluded.text = _read().excludedApps.join('\n');
    roots.text = _read().fileSearchRoots.join('\n');
    exclusions.text = _read().fileSearchExcludedNames.join('\n');
    _pending = null;
  }

  Future<bool> waitForPending() => _pending ?? Future.value(true);

  Future<bool> apply(HostSettings Function(HostSettings) update,
      {bool excludedApps = false, bool fileSearch = false}) {
    if (_disposed) return Future.value(false);
    final submittedApps = excluded.text;
    final submittedRoots = roots.text;
    final submittedExclusions = exclusions.text;
    Future<bool> run() async {
      if (_disposed) return false;
      try {
        var next = update(_read());
        if (excludedApps) {
          next = next.copyWith(excludedApps: _excludedIds(submittedApps));
        }
        if (fileSearch) {
          next = next.copyWith(
              fileSearchRoots: _lines(submittedRoots),
              fileSearchExcludedNames: _lines(submittedExclusions));
        }
        final saved = await _save(next);
        if (saved && !_disposed) {
          if (excludedApps && excluded.text == submittedApps) {
            excluded.text = _read().excludedApps.join('\n');
          }
          if (fileSearch && roots.text == submittedRoots) {
            roots.text = _read().fileSearchRoots.join('\n');
          }
          if (fileSearch && exclusions.text == submittedExclusions) {
            exclusions.text = _read().fileSearchExcludedNames.join('\n');
          }
        }
        return saved;
      } catch (_) {
        if (!_disposed) _reportFailure();
        return false;
      }
    }

    final next = (_pending ?? Future.value(true)).then((_) => run());
    _pending = next;
    busy = true;
    notifyListeners();
    unawaited(next.then((_) {
      if (!_disposed && identical(_pending, next)) {
        busy = false;
        notifyListeners();
      }
    }));
    return next;
  }

  Future<bool> saveDrafts() =>
      apply((current) => current, excludedApps: true, fileSearch: true);

  @override
  void dispose() {
    _disposed = true;
    excluded.dispose();
    roots.dispose();
    exclusions.dispose();
    super.dispose();
  }
}
