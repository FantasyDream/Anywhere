/// App-level termination. Hiding any individual window does not stop tasks.
class ApplicationLifecycle {
  ApplicationLifecycle(
      {required this.closeSettings, required this.shutdownTasks});
  final Future<bool> Function() closeSettings;
  final Future<void> Function() shutdownTasks;
  Future<bool>? _quitting;

  Future<bool> prepareToQuit() =>
      _quitting ??= _prepare().whenComplete(() => _quitting = null);
  Future<bool> _prepare() async {
    if (!await closeSettings()) return false;
    await shutdownTasks();
    return true;
  }
}
