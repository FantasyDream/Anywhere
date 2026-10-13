import 'runtime_policy.dart';
import 'package:flutter/services.dart';

import '../settings/host_settings.dart';
import '../src/rust/api.dart' as bridge;
import 'desktop_environment.dart';

class SearchAccessSuperseded implements Exception {}

/// Native authorization stays at the Host boundary. Queries never invoke this
/// port; only startup, saved preferences and an explicit access review do.
class FileSearchAccess {
  static const channel = MethodChannel('anywhere/window');
  static int _generation = 0;
  // All writes share this queue, including revocation. An obsolete write cannot
  // finish after a newer configuration and restore its predecessor.
  static Future<void> _writes = Future.value();

  static Future<void> apply(HostSettings settings,
      {bool force = false,
      Future<void> Function(bool enabled, List<String> roots, bool restricted)?
          write}) async {
    final generation = ++_generation;
    bool current() => generation == _generation;
    write ??= (enabled, roots, restricted) => bridge.setFileSearch(
        enabled: enabled,
        roots: roots,
        engine: settings.fileSearchEngine,
        includeHidden: settings.fileSearchIncludeHidden,
        excludedDirectoryNames: settings.fileSearchExcludedNames,
        restrictToRoots: restricted);
    final writer = write;
    Future<void> publish(bool enabled, List<String> roots, bool restricted) {
      final next = _writes.then((_) async {
        if (!current()) throw SearchAccessSuperseded();
        await writer(enabled, roots, restricted);
      });
      _writes = next.catchError((Object _) {});
      return next;
    }

    var roots = settings.fileSearchRoots;
    final restricted = DesktopEnvironment.current.fileSearchAccess;
    if (restricted) {
      // Fail closed while a native authorization dialog is pending, including
      // when Settings changes while a background search still exists.
      await publish(false, [], true);
      final result = await channel
          .invokeMapMethod<String, Object?>('prepareFileSearchAccess', {
        'enabled': settings.fileSearchEnabled,
        'roots': roots,
        'force': force,
      });
      if (!current() || result?['superseded'] == true) {
        throw SearchAccessSuperseded();
      }
      if (result == null || result['roots'] is! List) {
        throw StateError('Search directory authorization is unavailable');
      }
      roots = List<String>.from(result['roots'] as List);
      if (roots.length > RuntimePolicy.maxSearchRoots ||
          roots.any((root) => !root.startsWith('/'))) {
        throw StateError('Invalid authorized search directory');
      }
    }
    await publish(
        settings.fileSearchEnabled && (!restricted || roots.isNotEmpty),
        roots,
        restricted);
  }
}
