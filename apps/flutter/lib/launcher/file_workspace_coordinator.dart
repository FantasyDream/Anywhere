import '../src/rust/api.dart';
import 'file_search_controller.dart';
import 'launcher_backend.dart';
import 'launcher_navigation.dart';
import 'launcher_session_coordinator.dart';

/// Owns tool state and asynchronous object/load requests. The Launcher controls
/// navigation and presentation; Rust controls which objects and actions exist.
class FileWorkspaceCoordinator extends FileSearchController {
  FileWorkspaceCoordinator(super.load);
  LauncherLocation? returnLocation;
  bool moreLoading = false;
  int _moreRequest = 0;
  void invalidateOperations() {
    ++_moreRequest;
    moreLoading = false;
  }

  Future<LauncherStateDto?> loadSnapshot(
      int session,
      int revision,
      LauncherBackend backend,
      LauncherStateDto? Function() snapshot,
      bool Function() current) {
    final held = snapshot();
    final count = held?.candidates.length ?? 0;
    final version = held?.candidateListVersion ?? 0;
    return loadMore(session, revision, (id, rev) async {
      if (backend.fileMoreDelta == null) return backend.fileMore!(id, rev);
      final packet = await backend.fileMoreDelta!(id, rev, version, count);
      final latest = snapshot();
      if (latest?.sessionId == id &&
          latest?.revision == rev &&
          packet.state != null &&
          snapshotCursor(packet.state!) <= snapshotCursor(latest!)) {
        return latest;
      }
      return LauncherSessionCoordinator.materialize(latest, packet, count);
    }, current);
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

  @override
  void reset() {
    invalidateOperations();
    super.reset();
  }
}
