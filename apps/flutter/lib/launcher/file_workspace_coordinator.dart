import '../src/rust/api.dart';
import 'file_search_controller.dart';
import 'launcher_navigation.dart';

/// Owns tool state and asynchronous object/load requests. The Launcher controls
/// navigation and presentation; Rust controls which objects and actions exist.
class FileWorkspaceCoordinator extends FileSearchController {
  FileWorkspaceCoordinator(super.load);
  LauncherLocation? returnLocation;
  bool moreLoading = false;
  int _moreRequest = 0, _actionRequest = 0;
  void invalidateOperations() {
    ++_moreRequest;
    ++_actionRequest;
    moreLoading = false;
  }

  Future<LauncherStateDto?> loadMore(
      int session,
      int revision,
      Future<LauncherStateDto?> Function(int, int) load,
      bool Function() current) async {
    if (moreLoading) return null;
    final request = ++_moreRequest;
    moreLoading = true;
    notifyListeners();
    try {
      final state = await load(session, revision);
      return request == _moreRequest &&
              current() &&
              state?.sessionId == session &&
              state?.revision == revision
          ? state
          : null;
    } catch (_) {
      if (request == _moreRequest && current()) rethrow;
      return null;
    } finally {
      if (request == _moreRequest) {
        moreLoading = false;
        notifyListeners();
      }
    }
  }

  Future<List<CandidateDto>?> loadActions(
      Future<List<CandidateDto>> Function() load,
      bool Function() current) async {
    final request = ++_actionRequest;
    try {
      final items = await load();
      return request == _actionRequest && current() ? items : null;
    } catch (_) {
      if (request == _actionRequest && current()) rethrow;
      return null;
    }
  }

  @override
  void reset() {
    invalidateOperations();
    super.reset();
  }
}
