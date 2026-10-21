import '../l10n/app_language.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import '../ui/app_style.dart';
import 'execution_sessions_view.dart';
import 'execution_sessions_controller.dart';

class ExecutionSessionsApp extends StatefulWidget {
  const ExecutionSessionsApp(
      {super.key,
      this.controller,
      this.channel = const MethodChannel('anywhere/executions')});
  final ExecutionSessionsController? controller;
  final MethodChannel channel;
  @override
  State<ExecutionSessionsApp> createState() => _ExecutionSessionsAppState();
}

class _ExecutionSessionsAppState extends State<ExecutionSessionsApp> {
  late final controller = widget.controller ??
      ExecutionSessionsController(ExecutionSessionsBackend());
  final _viewKey = GlobalKey<ExecutionSessionsViewState>();
  bool _visible = true;
  @override
  void initState() {
    super.initState();
    widget.channel.setMethodCallHandler((call) async {
      if (call.method == 'select' && call.arguments is String) {
        controller.select(call.arguments as String);
      }
      if (call.method == 'find') {
        _viewKey.currentState?.focusSearch();
      }
      if (call.method == 'stop') {
        await controller.stopSelected();
      }
      if (call.method == 'refresh') {
        await controller.refresh();
      }
      if (call.method == 'visibility') {
        _visible = call.arguments == true;
        controller.setVisible(_visible);
      }
    });
    _ready();
  }

  Future<void> _ready() async {
    try {
      final id = await widget.channel.invokeMethod<String>('ready');
      if (!mounted) {
        return;
      }
      if (id != null) {
        controller.select(id);
      }
      controller.setVisible(_visible);
    } catch (_) {
      if (mounted) {
        controller.setVisible(_visible);
      }
    }
  }

  @override
  void dispose() {
    widget.channel.setMethodCallHandler(null);
    controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => LocalizedApp(
        debugShowCheckedModeBanner: false,
        theme: AppStyle.theme,
        builder: AppStyle.windowBuilder,
        home: ListenableBuilder(
          listenable: controller,
          builder: (context, _) => ExecutionSessionsView(
              key: _viewKey,
              controller: controller,
              onOpenLauncher: () => _hostAction('showLauncher'),
              onHide: () => _hostAction('hide')),
        ),
      );

  Future<void> _hostAction(String method) async {
    try {
      await widget.channel.invokeMethod<void>(method);
    } catch (_) {
      controller.reportError('The window could not be changed. Try again.');
    }
  }
}
