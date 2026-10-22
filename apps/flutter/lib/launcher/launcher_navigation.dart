import '../src/rust/api.dart';

/// Overlays are mutually exclusive; an object action panel always carries its target
/// and return selection together with its offers.
sealed class LauncherOverlay {
  const LauncherOverlay();
}

final class LauncherHome extends LauncherOverlay {
  const LauncherHome();
}

final class RootChooser extends LauncherOverlay {
  const RootChooser(this.mode);
  final InputModeDto mode;
}

final class BindingChoices extends LauncherOverlay {
  BindingChoices(List<CandidateDto> items) : items = List.unmodifiable(items);
  final List<CandidateDto> items;
}

final class ObjectActions extends LauncherOverlay {
  ObjectActions(
      {required this.target,
      required this.title,
      required this.parent,
      required List<CandidateDto> items})
      : items = List.unmodifiable(items);
  final String target, title;
  final String? parent;
  final List<CandidateDto> items;
}

/// Named parent location for tool navigation; context mode is independent of Scope.
class LauncherLocation {
  const LauncherLocation(
      {required this.input,
      required this.mode,
      this.scope,
      this.menu,
      this.source,
      required this.universal});
  final String input;
  final InputModeDto mode;
  final String? scope;
  final List<String>? menu;
  final ContextSourceDto? source;
  final bool universal;
}

enum WakeMode { currentApp, universal }

WakeMode parseWakeMode(String? value) =>
    value == 'universal' ? WakeMode.universal : WakeMode.currentApp;

/// Pure navigation transitions. Context eligibility and Scope membership arrive
/// from Rust; this model only holds the user's location and overlay.
class LauncherNavigation {
  LauncherOverlay overlay = const LauncherHome();
  String? scope;
  List<String>? _menu;
  List<String>? get menu => _menu;
  set menu(List<String>? path) =>
      _menu = path == null ? null : List.unmodifiable(path);
  WakeMode wakeMode = WakeMode.currentApp;
  InputModeDto? get chooserMode =>
      switch (overlay) { RootChooser(:final mode) => mode, _ => null };
  set chooserMode(InputModeDto? mode) =>
      overlay = mode == null ? const LauncherHome() : RootChooser(mode);
  void cycleRoot() {
    if (chooserMode == null) return;
    chooserMode = chooserMode == InputModeDto.search
        ? InputModeDto.environment
        : InputModeDto.search;
  }

  void selectScope(String? id) {
    scope = id;
    menu = null;
    overlay = const LauncherHome();
  }

  void backMenu() {
    final path = menu;
    if (path != null) {
      menu = path.length <= 1 ? null : path.sublist(0, path.length - 1);
    }
  }
}
