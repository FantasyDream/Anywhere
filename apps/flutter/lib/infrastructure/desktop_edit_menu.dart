import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

/// Native menus send editing intentions. Flutter owns the focused editor,
/// selection, clipboard operations and undo history in each engine.
class DesktopEditMenu extends StatefulWidget {
  const DesktopEditMenu({super.key, required this.child});
  final Widget child;
  @override
  State<DesktopEditMenu> createState() => _DesktopEditMenuState();
}

class _DesktopEditMenuState extends State<DesktopEditMenu> {
  static const _channel = MethodChannel('anywhere/editing');
  @override
  void initState() {
    super.initState();
    _channel.setMethodCallHandler((call) async {
      if (call.method != 'perform') throw MissingPluginException();
      final intent = switch (call.arguments) {
        'undo' => const UndoTextIntent(SelectionChangedCause.keyboard),
        'redo' => const RedoTextIntent(SelectionChangedCause.keyboard),
        'cut' =>
          const CopySelectionTextIntent.cut(SelectionChangedCause.keyboard),
        'copy' => CopySelectionTextIntent.copy,
        'paste' => const PasteTextIntent(SelectionChangedCause.keyboard),
        'selectAll' =>
          const SelectAllTextIntent(SelectionChangedCause.keyboard),
        _ => null,
      };
      final context = FocusManager.instance.primaryFocus?.context;
      if (!mounted || context == null || intent == null) return;
      final result = Actions.maybeInvoke(context, intent);
      if (result is Future) await result;
    });
  }

  @override
  void dispose() {
    _channel.setMethodCallHandler(null);
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => widget.child;
}
