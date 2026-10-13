import '../src/rust/api.dart';
import '../infrastructure/application_icon_cache.dart';
import 'activation_prefill.dart';
import 'file_search_controller.dart';

typedef BeginActivation = int Function();
typedef Activate = Future<LauncherStateDto?> Function(
  int session, {
  bool promptAccessibility,
  bool capturedEnvironment,
});
typedef PromptAccessibility = bool Function();
typedef UpdateInput = Future<LauncherStateDto?> Function(
    int session, String input, int revision,
    {String? scopeId, InputModeDto inputMode});
typedef Execute = Future<ActionResultDto> Function(
  int session,
  int revision,
  String candidate,
);
typedef WaitForSnapshot = Future<SnapshotWaitDto> Function(
    int session, int after);
int snapshotCursor(LauncherStateDto state) =>
    (state.revision << 32) | state.snapshotSeq;

class LauncherBackend {
  const LauncherBackend({
    required this.beginActivation,
    required this.activate,
    required this.updateInput,
    required this.execute,
    required this.deactivate,
    this.waitForSnapshot,
    this.waitForSnapshotDelta,
    this.loadIcon,
    this.promptAccessibility,
    this.shortcutActions,
    this.supportedShortcutActions,
    this.shortcutApplications,
    this.resolveIntent,
    this.waitIntent,
    this.resolveBinding,
    this.waitBinding,
    this.loadFilePreview,
    this.updateFiles,
    this.fileMore,
    this.fileMoreDelta,
    this.freezeFiles,
    this.objectActions,
    this.suspend,
    this.resume,
    this.chooseActivationInput,
  });

  final Future<LauncherStateDto?> Function(
      int session, String input, int revision, String filter)? updateFiles;
  final Future<LauncherStateDto?> Function(int session, int revision)? fileMore;
  final Future<SnapshotWaitDto> Function(int, int, int, int)? fileMoreDelta;
  final Future<void> Function(int session, int revision)? freezeFiles;
  final Future<List<CandidateDto>> Function(int, int, String)? objectActions;
  final LoadFilePreview? loadFilePreview;
  final ChooseActivationInput? chooseActivationInput;
  final BeginActivation beginActivation;
  final Activate activate;
  final UpdateInput updateInput;
  final Execute execute;
  final Future<void> Function(int) deactivate;
  final Future<void> Function(int)? suspend;
  final Future<LauncherStateDto?> Function(int)? resume;
  final WaitForSnapshot? waitForSnapshot;
  final Future<SnapshotWaitDto> Function(int, int, int, int)?
      waitForSnapshotDelta;
  final LoadIcon? loadIcon;
  final PromptAccessibility? promptAccessibility;
  final Future<List<ShortcutActionDto>> Function()? shortcutActions;
  final Future<List<String>> Function(List<String>)? supportedShortcutActions;
  final Future<List<ShortcutApplicationDto>> Function()? shortcutApplications;
  final Future<BindingResolutionDto> Function(
      int session, int revision, List<ShortcutRouteDto> routes)? resolveIntent;
  final Future<BindingResolutionDto> Function(
      int session, int revision, List<ShortcutRouteDto> routes)? waitIntent;
  final Future<List<CandidateDto>> Function(
      int session, int revision, String action)? resolveBinding;
  final Future<BindingResolutionDto> Function(
      int session, int revision, String action)? waitBinding;
}
