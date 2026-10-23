import 'dart:convert';

import '../settings/action_shortcut.dart';
import '../settings/host_settings.dart';
import '../src/rust/api.dart';
import 'launcher_backend.dart';

class ShortcutOffers {
  ShortcutOffers(List<CandidateDto> candidates,
      {this.state, this.requiresChoice = false})
      : candidates = List.unmodifiable(candidates);
  final List<CandidateDto> candidates;
  final LauncherStateDto? state;
  final bool requiresChoice;
}

/// Coordinates current/waiting shortcut requests. Core chooses the eligible
/// actions and targets; navigation and execution presentation remain in Launcher.
class ShortcutCoordinator {
  ShortcutCoordinator(this.backend);
  final LauncherBackend backend;
  int _request = 0;
  bool waiting = false;
  int reserve() => ++_request;
  void invalidate() => ++_request;
  bool current(int request) => request == _request;

  bool bindingCurrent(ActionShortcut binding, HostSettings settings) =>
      settings.actionShortcuts.any((b) =>
          b.actionId == binding.actionId &&
          jsonEncode(b.toMap()) == jsonEncode(binding.toMap()));

  Future<ShortcutOffers> resolve(
      ActionShortcut binding, int session, int revision,
      {bool wait = false}) async {
    final routes = binding.routes.map((r) => r.toDto()).toList();
    final resolved = binding.isIntent
        ? await (wait ? backend.waitIntent! : backend.resolveIntent!)(
            session, revision, routes)
        : wait
            ? await backend.waitBinding!(session, revision, binding.actionId)
            : null;
    if (resolved != null) {
      if (resolved.state.sessionId != session ||
          resolved.state.revision != revision) {
        throw StateError('Shortcut context expired. Try again.');
      }
      return ShortcutOffers(resolved.candidates,
          state: resolved.state, requiresChoice: resolved.requiresChoice);
    }
    return ShortcutOffers(
        await backend.resolveBinding!(session, revision, binding.actionId));
  }
}
