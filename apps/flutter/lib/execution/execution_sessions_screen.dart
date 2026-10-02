import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import '../src/rust/api.dart';
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
  @override
  void initState() {
    super.initState();
    widget.channel.setMethodCallHandler((call) async {
      if (call.method == 'select' && call.arguments is String) {
        controller.select(call.arguments as String);
      }
      if (call.method == 'visibility') {
        controller.setVisible(call.arguments == true);
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
      controller.setVisible(true);
    } catch (_) {
      if (mounted) {
        controller.setVisible(true);
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
  Widget build(BuildContext context) => MaterialApp(
        debugShowCheckedModeBanner: false,
        theme: ThemeData.dark(useMaterial3: true)
            .copyWith(scaffoldBackgroundColor: const Color(0xff1c1e24)),
        home: ListenableBuilder(
            listenable: controller,
            builder: (context, _) {
              final detail = controller.detail;
              return Scaffold(
                  body: Column(children: [
                Padding(
                    padding: const EdgeInsets.fromLTRB(20, 14, 20, 12),
                    child: Row(children: [
                      const Icon(Icons.terminal_rounded, size: 22),
                      const SizedBox(width: 10),
                      const Text('Running Tasks',
                          style: TextStyle(
                              fontSize: 19, fontWeight: FontWeight.w600)),
                      const Spacer(),
                      Text(
                          '${controller.sessions.where((s) => isTaskActive(s.status)).length} running',
                          style: const TextStyle(color: Colors.white54)),
                      IconButton(
                          tooltip: 'Refresh',
                          onPressed: controller.refresh,
                          icon: const Icon(Icons.refresh_rounded)),
                    ])),
                if (controller.error != null)
                  Padding(
                      padding: const EdgeInsets.symmetric(horizontal: 20),
                      child: Text(controller.error!,
                          style: const TextStyle(color: Colors.orangeAccent))),
                const Divider(height: 1),
                Expanded(
                    child: controller.sessions.isEmpty
                        ? const Center(
                            child: Text(
                                'No tasks yet.\nChoose Cargo Check or Cargo Test in a Rust project.',
                                textAlign: TextAlign.center,
                                style: TextStyle(color: Colors.white60)))
                        : Row(
                            crossAxisAlignment: CrossAxisAlignment.stretch,
                            children: [
                                SizedBox(
                                    width: 240,
                                    child: ListView.builder(
                                        itemCount: controller.sessions.length,
                                        itemBuilder: (context, index) {
                                          final session =
                                              controller.sessions[index];
                                          return ListTile(
                                              selected: controller.selectedId ==
                                                  session.id,
                                              selectedTileColor: Colors.white
                                                  .withValues(alpha: .07),
                                              title: Text(session.title,
                                                  maxLines: 1,
                                                  overflow:
                                                      TextOverflow.ellipsis),
                                              subtitle: Text(
                                                  '${taskStatus(session.status)} · ${session.directory.split('/').last}',
                                                  maxLines: 2,
                                                  overflow:
                                                      TextOverflow.ellipsis),
                                              onTap: () => controller
                                                  .select(session.id));
                                        })),
                                const VerticalDivider(width: 1),
                                Expanded(
                                    child: detail == null
                                        ? const Center(
                                            child: CircularProgressIndicator())
                                        : _TaskDetail(
                                            detail: detail,
                                            controller: controller)),
                              ])),
                const Divider(height: 1),
                const Padding(
                    padding: EdgeInsets.all(12),
                    child: Text(
                        'Closing this window keeps tasks running. Quitting Anywhere stops them.',
                        style: TextStyle(fontSize: 12, color: Colors.white54))),
              ]));
            }),
      );
}

String taskStatus(ExecutionSessionStatusDto status) => switch (status) {
      ExecutionSessionStatusDto.running => 'Running',
      ExecutionSessionStatusDto.stopping => 'Stopping',
      ExecutionSessionStatusDto.succeeded => 'Completed',
      ExecutionSessionStatusDto.failed => 'Failed',
      ExecutionSessionStatusDto.stopped => 'Stopped',
    };

class _TaskDetail extends StatelessWidget {
  const _TaskDetail({required this.detail, required this.controller});
  final ExecutionSessionDetailDto detail;
  final ExecutionSessionsController controller;
  @override
  Widget build(BuildContext context) {
    final summary = detail.summary;
    return Column(crossAxisAlignment: CrossAxisAlignment.stretch, children: [
      Padding(
          padding: const EdgeInsets.all(18),
          child:
              Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
            Row(children: [
              Expanded(
                  child: Text(
                      '${summary.title} · ${taskStatus(summary.status)}${summary.exitCode == null ? '' : ' (${summary.exitCode})'}',
                      style: const TextStyle(fontWeight: FontWeight.w600))),
              if (isTaskActive(summary.status))
                FilledButton.tonal(
                    onPressed: controller.busy ||
                            summary.status == ExecutionSessionStatusDto.stopping
                        ? null
                        : controller.stopSelected,
                    child: const Text('Stop'))
              else
                TextButton(
                    onPressed:
                        controller.busy ? null : controller.removeSelected,
                    child: const Text('Remove')),
            ]),
            const SizedBox(height: 8),
            SelectableText(summary.directory,
                style: const TextStyle(color: Colors.white60, fontSize: 12)),
            const SizedBox(height: 6),
            SelectableText(
                summary.command
                    .map((arg) => arg.contains(' ') ? '"$arg"' : arg)
                    .join(' '),
                style: const TextStyle(fontFamily: 'monospace', fontSize: 12)),
            if (summary.outputTruncated)
              const Padding(
                  padding: EdgeInsets.only(top: 8),
                  child: Text('Showing the latest output (128 KiB per stream).',
                      style:
                          TextStyle(color: Colors.orangeAccent, fontSize: 12))),
            if (summary.error != null)
              Text(summary.error!,
                  style: const TextStyle(color: Colors.orangeAccent)),
          ])),
      const Divider(height: 1),
      Expanded(
          child: SingleChildScrollView(
              key: ValueKey(summary.id),
              padding: const EdgeInsets.all(18),
              child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    const Text('OUTPUT',
                        style: TextStyle(color: Colors.white38, fontSize: 11)),
                    const SizedBox(height: 8),
                    SelectableText(
                        detail.stdout.isEmpty
                            ? (isTaskActive(summary.status)
                                ? 'Waiting for output…'
                                : 'No standard output.')
                            : detail.stdout,
                        style: const TextStyle(
                            fontFamily: 'monospace',
                            fontSize: 12,
                            height: 1.5)),
                    if (detail.stderr.isNotEmpty) ...[
                      const SizedBox(height: 20),
                      const Text('STDERR',
                          style:
                              TextStyle(color: Colors.white38, fontSize: 11)),
                      const SizedBox(height: 8),
                      SelectableText(detail.stderr,
                          style: const TextStyle(
                              fontFamily: 'monospace',
                              fontSize: 12,
                              height: 1.5)),
                    ],
                  ]))),
    ]);
  }
}
