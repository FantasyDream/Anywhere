import 'dart:async';

import 'package:anywhere/launcher/file_workspace_coordinator.dart';
import 'package:anywhere/src/rust/api.dart';
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
}
