/// App-level termination. Hiding any individual window does not stop tasks.
class ApplicationLifecycle {
  ApplicationLifecycle(
      {required this.closeSettings,
      required this.shutdownTasks,
      this.cancelLauncher});
  final Future<bool> Function() closeSettings;
  final Future<void> Function() shutdownTasks;
  final Future<void> Function()? cancelLauncher;
  Future<bool>? _quitting;
  bool _shutdownComplete = false;

  bool get isQuitting => _quitting != null || _shutdownComplete;

  Future<bool> prepareToQuit() => _shutdownComplete
      ? Future.value(true)
      : _quitting ??= _prepare().whenComplete(() => _quitting = null);
  Future<bool> _prepare() async {
    if (!await closeSettings()) return false;
    await cancelLauncher?.call();
    await shutdownTasks();
    _shutdownComplete = true;
    return true;
  }
}
