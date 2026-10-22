import '../src/rust/api.dart';

/// Tool adapters register navigation here; adding a host view does not add a
/// domain-specific execution branch to the Launcher controller.
class HostViewNavigator {
  final Map<HostViewDto, Future<void> Function(ActionResultDto?)> _routes = {};
  void register(HostViewDto view, Future<void> Function() open) {
    _register(view, (_) => open());
  }

  void registerPayload(
      HostViewDto view, Future<void> Function(ActionResultDto) open) {
    _register(view, (result) {
      if (result == null || result.view != view) {
        throw StateError('Host view payload is missing or mismatched');
      }
      return open(result);
    });
  }

  void _register(
      HostViewDto view, Future<void> Function(ActionResultDto?) open) {
    if (_routes.containsKey(view)) {
      throw ArgumentError('Invalid or duplicate host view');
    }
    _routes[view] = open;
  }

  bool accepts(HostViewDto? view) => _routes.containsKey(view);
  Future<void> open(HostViewDto? view, {ActionResultDto? result}) async {
    final route = _routes[view];
    if (route == null) throw StateError('Host view is not available');
    await route(result);
  }
}
