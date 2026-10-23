import 'dart:async';

import 'package:anywhere/infrastructure/application_lifecycle.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  test('cancelled draft decision keeps running tasks and allows a later quit',
      () async {
    var allow = false;
    var stopped = 0;
    final lifecycle = ApplicationLifecycle(
      closeSettings: () async => allow,
      shutdownTasks: () async {
        stopped++;
      },
    );
    expect(await lifecycle.prepareToQuit(), isFalse);
    expect(stopped, 0);
    allow = true;
    expect(await lifecycle.prepareToQuit(), isTrue);
    expect(stopped, 1);
  });

  test('concurrent native quit requests share one decision and shutdown',
      () async {
    final decision = Completer<bool>();
    var prompted = 0;
    var stopped = 0;
    final lifecycle = ApplicationLifecycle(
      closeSettings: () {
        prompted++;
        return decision.future;
      },
      shutdownTasks: () async {
        stopped++;
      },
    );
    final first = lifecycle.prepareToQuit();
    final second = lifecycle.prepareToQuit();
    decision.complete(true);
    expect(await first, isTrue);
    expect(await second, isTrue);
    expect(prompted, 1);
    expect(stopped, 1);
  });
}
