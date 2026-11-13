import 'dart:async';

import 'package:anywhere/infrastructure/bridge_api.dart';
import 'package:anywhere/launcher/file_workspace_coordinator.dart';
import 'package:anywhere/launcher/launcher_backend.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  for (final fails in [false, true]) {
    test(
        'disposed file workspace ignores a late ${fails ? 'failure' : 'reply'}',
        () async {
      final files = FileWorkspaceCoordinator(null);
      final reply = Completer<LauncherStateDto?>();
      var current = true;
      final pending =
          files.loadMore(1, 0, (_, __) => reply.future, () => current);
      current = false;
      files.dispose();
      if (fails) {
        reply.completeError(StateError('late failure'));
      } else {
        reply.complete(null);
      }
      expect(await pending, isNull);
    });
  }

  test('disposed file workspace ignores a late page reply', () async {
    final files = FileWorkspaceCoordinator(null);
    final reply = Completer<LauncherStateDto?>();
    var current = true;
    final backend = LauncherBackend(
        beginActivation: () => 1,
        activate: (_,
                {promptAccessibility = false,
                capturedEnvironment = false}) async =>
            null,
        updateInput: (_, __, ___,
                {scopeId, inputMode = InputModeDto.environment}) async =>
            null,
        execute: (_, __, ___) async => throw UnimplementedError(),
        deactivate: (_) async {},
        filePage: (_, __, ___) => reply.future);
    final pending = files.loadPage(1, 0, 'cursor', backend, () => current);
    current = false;
    files.dispose();
    reply.complete(null);
    expect(await pending, isNull);
  });
}
