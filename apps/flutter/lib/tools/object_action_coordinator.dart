import '../src/rust/api.dart';

/// Latest-target-only requests shared by files and module-owned objects.
class ObjectActionCoordinator {
  int _request = 0;
  void invalidate() => ++_request;
  Future<List<CandidateDto>?> load(Future<List<CandidateDto>> Function() read,
      bool Function() current) async {
    final request = ++_request;
    try {
      final items = await read();
      return request == _request && current() ? items : null;
    } catch (_) {
      if (request == _request && current()) rethrow;
      return null;
    }
  }
}
