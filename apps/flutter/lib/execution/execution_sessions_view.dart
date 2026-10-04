import 'dart:async';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'execution_output_view.dart';
import 'execution_command.dart';
import '../launcher/launcher_style.dart';
import '../src/rust/api.dart';
import 'execution_sessions_controller.dart';

const _label = TextStyle(fontSize: 11, color: LauncherStyle.muted);
const _body = TextStyle(fontSize: 12, color: LauncherStyle.secondary);
const _mono = TextStyle(
    fontFamily: 'Menlo',
    fontFamilyFallback: ['monospace'],
    fontSize: 11.5,
    height: 1.65,
    color: LauncherStyle.secondary);

String taskStatus(ExecutionSessionStatusDto status) => switch (status) {
      ExecutionSessionStatusDto.running => 'Running',
      ExecutionSessionStatusDto.stopping => 'Stopping',
      ExecutionSessionStatusDto.succeeded => 'Completed',
      ExecutionSessionStatusDto.failed => 'Failed',
      ExecutionSessionStatusDto.stopped => 'Stopped',
    };

Color _statusColor(ExecutionSessionStatusDto status) => switch (status) {
      ExecutionSessionStatusDto.running => LauncherStyle.accent,
      ExecutionSessionStatusDto.succeeded => LauncherStyle.success,
      ExecutionSessionStatusDto.failed => LauncherStyle.warning,
      _ => LauncherStyle.muted,
    };

class ExecutionSessionsView extends StatefulWidget {
  const ExecutionSessionsView(
      {super.key,
      required this.controller,
      required this.onOpenLauncher,
      required this.onHide});
  final ExecutionSessionsController controller;
  final Future<void> Function() onOpenLauncher, onHide;
  @override
  ExecutionSessionsViewState createState() => ExecutionSessionsViewState();
}

class ExecutionSessionsViewState extends State<ExecutionSessionsView> {
  ExecutionSessionsController get controller => widget.controller;
  final _search = TextEditingController();
  final _searchFocus = FocusNode();
  final _listFocus = FocusNode();
  final _listScroll = ScrollController();
  String? _lastSelection;
  String _lastFilter = '';
  double get _rowHeight => 80 * MediaQuery.textScalerOf(context).scale(12) / 12;

  @override
  void initState() {
    super.initState();
    controller.addListener(_syncSelection);
    _syncSelection();
  }

  void _syncSelection() {
    if (_search.text != controller.filter) {
      _search.value = TextEditingValue(
          text: controller.filter,
          selection: TextSelection.collapsed(offset: controller.filter.length));
    }
    if (_lastSelection == controller.selectedId &&
        _lastFilter == controller.filter) {
      return;
    }
    _lastSelection = controller.selectedId;
    _lastFilter = controller.filter;
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!mounted || !_listScroll.hasClients) return;
      final index = controller.visibleSessions
          .indexWhere((s) => s.id == controller.selectedId);
      if (index < 0) return;
      final top = index * _rowHeight;
      final bottom = top + _rowHeight;
      final position = _listScroll.position;
      if (top < position.pixels) {
        _listScroll.jumpTo(top.clamp(0, position.maxScrollExtent));
      } else if (bottom > position.pixels + position.viewportDimension) {
        _listScroll.jumpTo((bottom - position.viewportDimension)
            .clamp(0, position.maxScrollExtent));
      }
    });
  }

  void focusSearch() {
    _searchFocus.requestFocus();
    _search.selection =
        TextSelection(baseOffset: 0, extentOffset: _search.text.length);
  }

  void _hideOrClear() {
    if (controller.filter.isNotEmpty) {
      controller.setFilter('');
      _listFocus.requestFocus();
    } else {
      unawaited(widget.onHide());
    }
  }

  KeyEventResult _listKey(FocusNode node, KeyEvent event) {
    if (event is! KeyDownEvent && event is! KeyRepeatEvent) {
      return KeyEventResult.ignored;
    }
    if (HardwareKeyboard.instance.isMetaPressed ||
        HardwareKeyboard.instance.isControlPressed ||
        HardwareKeyboard.instance.isAltPressed ||
        HardwareKeyboard.instance.isShiftPressed) {
      return KeyEventResult.ignored;
    }
    if (event.logicalKey == LogicalKeyboardKey.arrowUp ||
        event.logicalKey == LogicalKeyboardKey.arrowDown) {
      controller.moveSelection(
          event.logicalKey == LogicalKeyboardKey.arrowUp ? -1 : 1);
      return KeyEventResult.handled;
    }
    return KeyEventResult.ignored;
  }

  @override
  void dispose() {
    controller.removeListener(_syncSelection);
    _search.dispose();
    _searchFocus.dispose();
    _listFocus.dispose();
    _listScroll.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final running =
        controller.sessions.where((s) => isTaskActive(s.status)).length;
    final detail = controller.detail;
    final tasks = controller.visibleSessions;
    return CallbackShortcuts(
        bindings: {
          const SingleActivator(LogicalKeyboardKey.keyF, meta: true):
              focusSearch,
          const SingleActivator(LogicalKeyboardKey.keyL, meta: true): () =>
              unawaited(widget.onOpenLauncher()),
          const SingleActivator(LogicalKeyboardKey.keyR, meta: true): () =>
              unawaited(controller.refresh()),
          const SingleActivator(LogicalKeyboardKey.period, meta: true): () =>
              unawaited(controller.stopSelected()),
          const SingleActivator(LogicalKeyboardKey.arrowUp,
              meta: true, alt: true): () => controller.moveSelection(-1),
          const SingleActivator(LogicalKeyboardKey.arrowDown,
              meta: true, alt: true): () => controller.moveSelection(1),
          const SingleActivator(LogicalKeyboardKey.escape): _hideOrClear,
        },
        child: Scaffold(
          body: Material(
            color: LauncherStyle.surface,
            child: Column(children: [
              // The native material extends behind the title bar. Leave room for its traffic lights.
              const SizedBox(height: 28),
              _toolbar(running),
              if (controller.error != null)
                Padding(
                    padding: const EdgeInsets.fromLTRB(18, 0, 18, 12),
                    child: _Notice(controller.error!,
                        onDismiss: controller.dismissError)),
              const Divider(height: 1, color: LauncherStyle.divider),
              Expanded(
                  child: controller.sessions.isEmpty
                      ? const Focus(autofocus: true, child: _EmptyTasks())
                      : Row(
                          crossAxisAlignment: CrossAxisAlignment.stretch,
                          children: [
                              _sidebar(tasks),
                              const VerticalDivider(
                                  width: 1, color: LauncherStyle.divider),
                              Expanded(
                                  child: detail == null
                                      ? (tasks.isEmpty
                                          ? const Center(
                                              child: Text('No matching tasks',
                                                  style: _body))
                                          : const Center(
                                              child: SizedBox.square(
                                                  dimension: 18,
                                                  child:
                                                      CircularProgressIndicator(
                                                          strokeWidth: 1.5,
                                                          color: LauncherStyle
                                                              .accent))))
                                      : _TaskDetail(
                                          key: ValueKey(detail.summary.id),
                                          detail: detail,
                                          controller: controller)),
                            ])),
              const Divider(height: 1, color: LauncherStyle.divider),
              Padding(
                  padding:
                      const EdgeInsets.symmetric(horizontal: 18, vertical: 9),
                  child: Row(children: [
                    const Tooltip(
                        message:
                            'Closing this window keeps tasks running. Quitting Anywhere stops them.',
                        child: Icon(Icons.info_outline_rounded,
                            size: 13, color: LauncherStyle.muted)),
                    const SizedBox(width: 7),
                    const Expanded(
                        child: Text('Tasks keep running when hidden',
                            style: _label)),
                    const Text('↑↓ Tasks   ⌘F Find   ⌘. Stop   Esc Hide',
                        style: _label),
                  ])),
            ]),
          ),
        ));
  }

  Widget _toolbar(int running) => Padding(
        padding: const EdgeInsets.fromLTRB(18, 12, 14, 12),
        child: Row(children: [
          const _IconPlate(icon: Icons.terminal_rounded),
          const SizedBox(width: 10),
          const Text('Running Tasks',
              style: TextStyle(
                  fontSize: 14,
                  fontWeight: FontWeight.w600,
                  color: LauncherStyle.primary,
                  letterSpacing: -.2)),
          const Spacer(),
          Text(
              running == 0
                  ? (controller.sessions.isEmpty
                      ? 'No running tasks'
                      : 'All tasks finished')
                  : '$running running',
              style: _label),
          const SizedBox(width: 12),
          Tooltip(
              message: 'Open Launcher · ⌘L',
              child: TextButton.icon(
                  onPressed: widget.onOpenLauncher,
                  icon: const Icon(Icons.search_rounded, size: 16),
                  label: const Text('Launcher'),
                  style: TextButton.styleFrom(
                      foregroundColor: LauncherStyle.secondary,
                      minimumSize: const Size(0, 34),
                      textStyle: _body))),
          const SizedBox(width: 4),
          IconButton(
            tooltip: 'Refresh · ⌘R',
            onPressed: controller.refresh,
            constraints: const BoxConstraints.tightFor(width: 34, height: 34),
            padding: EdgeInsets.zero,
            visualDensity: VisualDensity.compact,
            icon: const Icon(Icons.refresh_rounded,
                size: 17, color: LauncherStyle.secondary),
          ),
        ]),
      );

  Widget _sidebar(List<ExecutionSessionSummaryDto> tasks) => SizedBox(
      width: 252,
      child: Column(children: [
        Padding(
            padding: const EdgeInsets.fromLTRB(10, 12, 10, 8),
            child: TextField(
                key: const Key('task-search'),
                controller: _search,
                focusNode: _searchFocus,
                onChanged: controller.setFilter,
                onSubmitted: (_) => _listFocus.requestFocus(),
                style: _body,
                cursorColor: LauncherStyle.accent,
                decoration: InputDecoration(
                  hintText: 'Find a task…',
                  hintStyle: _label,
                  prefixIcon: const Icon(Icons.search_rounded,
                      size: 16, color: LauncherStyle.muted),
                  prefixIconConstraints:
                      const BoxConstraints(minWidth: 30, minHeight: 34),
                  suffixIcon: controller.filter.isEmpty
                      ? null
                      : IconButton(
                          tooltip: 'Clear search',
                          icon: const Icon(Icons.close_rounded, size: 15),
                          color: LauncherStyle.secondary,
                          onPressed: () => controller.setFilter('')),
                  suffixIconConstraints:
                      const BoxConstraints.tightFor(width: 34, height: 34),
                  filled: true,
                  fillColor: const Color(0x0cffffff),
                  contentPadding:
                      const EdgeInsets.symmetric(vertical: 9, horizontal: 8),
                  isDense: true,
                  enabledBorder: OutlineInputBorder(
                      borderRadius: BorderRadius.circular(8),
                      borderSide:
                          const BorderSide(color: LauncherStyle.divider)),
                  focusedBorder: OutlineInputBorder(
                      borderRadius: BorderRadius.circular(8),
                      borderSide: const BorderSide(
                          color: LauncherStyle.selectedBorder)),
                ))),
        Padding(
            padding: const EdgeInsets.fromLTRB(16, 4, 16, 10),
            child: Row(children: [
              const Text('Tasks', style: _label),
              const Spacer(),
              Text('${tasks.length}', style: _label)
            ])),
        Expanded(
            child: Focus(
                focusNode: _listFocus,
                autofocus: true,
                onKeyEvent: _listKey,
                child: ListView.builder(
                  key: const Key('task-list'),
                  controller: _listScroll,
                  padding: const EdgeInsets.fromLTRB(8, 0, 8, 8),
                  itemExtent: _rowHeight,
                  itemCount: tasks.length,
                  itemBuilder: (context, index) => _TaskRow(
                      key: ValueKey('task:${tasks[index].id}'),
                      session: tasks[index],
                      selected: controller.selectedId == tasks[index].id,
                      onTap: () {
                        controller.select(tasks[index].id);
                        _listFocus.requestFocus();
                      }),
                ))),
      ]));
}

class _IconPlate extends StatelessWidget {
  const _IconPlate({required this.icon});
  final IconData icon;
  @override
  Widget build(BuildContext context) => Container(
      width: 28,
      height: 28,
      decoration: BoxDecoration(
          gradient: const LinearGradient(
              begin: Alignment.topLeft,
              end: Alignment.bottomRight,
              colors: [Color(0x59444a5c), Color(0x33282c38)]),
          borderRadius: BorderRadius.circular(8),
          border: Border.all(color: const Color(0x22ffffff), width: .5)),
      child: Icon(icon, size: 16, color: LauncherStyle.secondary));
}

class _TaskRow extends StatelessWidget {
  const _TaskRow(
      {super.key,
      required this.session,
      required this.selected,
      required this.onTap});
  final ExecutionSessionSummaryDto session;
  final bool selected;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    final project = session.directory
            .split(RegExp(r'[/\\]'))
            .where((s) => s.isNotEmpty)
            .lastOrNull ??
        session.directory;
    return Semantics(
      button: true,
      selected: selected,
      child: Padding(
        padding: const EdgeInsets.only(bottom: 4),
        child: Material(
          color: selected ? LauncherStyle.selectedSurface : Colors.transparent,
          shape: RoundedRectangleBorder(
              borderRadius: BorderRadius.circular(11),
              side: BorderSide(
                  color: selected
                      ? LauncherStyle.selectedBorder
                      : Colors.transparent,
                  width: .8)),
          child: InkWell(
            borderRadius: BorderRadius.circular(11),
            onTap: onTap,
            child: Padding(
                padding:
                    const EdgeInsets.symmetric(horizontal: 10, vertical: 8),
                child: Row(children: [
                  Container(
                      width: 3,
                      height: 26,
                      decoration: BoxDecoration(
                          color: selected
                              ? LauncherStyle.accent
                              : Colors.transparent,
                          borderRadius: BorderRadius.circular(99))),
                  const SizedBox(width: 9),
                  Expanded(
                      child: Column(
                          crossAxisAlignment: CrossAxisAlignment.start,
                          children: [
                        Tooltip(
                            message: session.title,
                            child: Text(session.title,
                                maxLines: 1,
                                overflow: TextOverflow.ellipsis,
                                style: const TextStyle(
                                    fontSize: 13,
                                    fontWeight: FontWeight.w600,
                                    color: LauncherStyle.primary))),
                        const SizedBox(height: 4),
                        Tooltip(
                            message: session.directory,
                            child: Text(project,
                                maxLines: 1,
                                overflow: TextOverflow.ellipsis,
                                style: _body)),
                        const SizedBox(height: 4),
                        Row(children: [
                          _StatusDot(session.status),
                          const SizedBox(width: 5),
                          Text(taskStatus(session.status), style: _label),
                          const Spacer(),
                          if (session.exitCode != null)
                            Text('Exit ${session.exitCode}', style: _label),
                        ]),
                      ])),
                ])),
          ),
        ),
      ),
    );
  }
}

class _StatusDot extends StatelessWidget {
  const _StatusDot(this.status);
  final ExecutionSessionStatusDto status;
  @override
  Widget build(BuildContext context) => Container(
      width: 5,
      height: 5,
      decoration:
          BoxDecoration(color: _statusColor(status), shape: BoxShape.circle));
}

class _TaskDetail extends StatelessWidget {
  const _TaskDetail(
      {super.key, required this.detail, required this.controller});
  final ExecutionSessionDetailDto detail;
  final ExecutionSessionsController controller;

  @override
  Widget build(BuildContext context) {
    final summary = detail.summary;
    final active = isTaskActive(summary.status);
    return Padding(
      padding: const EdgeInsets.fromLTRB(20, 18, 20, 16),
      child: LayoutBuilder(
          builder: (context, constraints) =>
              Column(crossAxisAlignment: CrossAxisAlignment.stretch, children: [
                ConstrainedBox(
                    constraints:
                        BoxConstraints(maxHeight: constraints.maxHeight * .6),
                    child: SingleChildScrollView(
                        primary: false,
                        child: Column(
                            crossAxisAlignment: CrossAxisAlignment.stretch,
                            children: [
                              Row(
                                  crossAxisAlignment: CrossAxisAlignment.start,
                                  children: [
                                    Expanded(
                                        child: Tooltip(
                                            message: summary.title,
                                            child: Text(summary.title,
                                                maxLines: 2,
                                                overflow: TextOverflow.ellipsis,
                                                style: const TextStyle(
                                                    fontSize: 16,
                                                    height: 1.35,
                                                    fontWeight: FontWeight.w600,
                                                    letterSpacing: -.25,
                                                    color: LauncherStyle
                                                        .primary)))),
                                    const SizedBox(width: 16),
                                    if (active)
                                      Tooltip(
                                          message: 'Stop task · ⌘.',
                                          child: TextButton.icon(
                                              onPressed: controller.busy || summary.status == ExecutionSessionStatusDto.stopping
                                                  ? null
                                                  : () => controller
                                                      .stop(summary.id),
                                              icon: const Icon(Icons.stop_rounded,
                                                  size: 16),
                                              label: Text(
                                                  summary.status == ExecutionSessionStatusDto.stopping
                                                      ? 'Stopping…'
                                                      : 'Stop'),
                                              style: TextButton.styleFrom(
                                                  foregroundColor:
                                                      LauncherStyle.warning,
                                                  backgroundColor: LauncherStyle
                                                      .warning
                                                      .withValues(alpha: .08),
                                                  disabledForegroundColor:
                                                      LauncherStyle.muted,
                                                  minimumSize: const Size(0, 34),
                                                  padding: const EdgeInsets.symmetric(horizontal: 10),
                                                  textStyle: _body,
                                                  shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(8), side: const BorderSide(color: LauncherStyle.divider, width: .5))))),
                                    PopupMenuButton<String>(
                                      tooltip: 'More actions',
                                      enabled: !controller.busy,
                                      color: const Color(0xff252a35),
                                      icon: const Icon(Icons.more_horiz_rounded,
                                          size: 18,
                                          color: LauncherStyle.secondary),
                                      constraints:
                                          const BoxConstraints(minWidth: 170),
                                      onSelected: (action) async {
                                        if (action == 'remove') {
                                          await controller.remove(summary.id);
                                        } else {
                                          try {
                                            await Clipboard.setData(
                                                ClipboardData(
                                                    text:
                                                        formatExecutionCommand(
                                                            summary.command)));
                                            if (context.mounted) {
                                              ScaffoldMessenger.of(context)
                                                  .showSnackBar(const SnackBar(
                                                      content: Text(
                                                          'Command copied'),
                                                      duration: Duration(
                                                          seconds: 2)));
                                            }
                                          } catch (_) {
                                            controller.reportError(
                                                'Command could not be copied. Try again.');
                                          }
                                        }
                                      },
                                      itemBuilder: (_) => [
                                        const PopupMenuItem(
                                            value: 'copy',
                                            child: Text('Copy command',
                                                style: _body)),
                                        if (!active)
                                          const PopupMenuItem(
                                              value: 'remove',
                                              child: Text('Remove result',
                                                  style: _body)),
                                      ],
                                    ),
                                  ]),
                              const SizedBox(height: 8),
                              Row(children: [
                                _StatusDot(summary.status),
                                const SizedBox(width: 6),
                                Text(taskStatus(summary.status),
                                    style: _body.copyWith(
                                        color: _statusColor(summary.status))),
                                if (summary.exitCode != null) ...[
                                  const SizedBox(width: 12),
                                  Text('Exit ${summary.exitCode}',
                                      style: _label),
                                ],
                              ]),
                              const SizedBox(height: 10),
                              Tooltip(
                                  message: summary.directory,
                                  child: Text(summary.directory,
                                      maxLines: 1,
                                      overflow: TextOverflow.ellipsis,
                                      style: _body)),
                              Theme(
                                  data: Theme.of(context).copyWith(
                                      dividerColor: Colors.transparent),
                                  child: ExpansionTile(
                                    tilePadding: EdgeInsets.zero,
                                    childrenPadding:
                                        const EdgeInsets.only(bottom: 10),
                                    dense: true,
                                    visualDensity: VisualDensity.compact,
                                    minTileHeight: 30,
                                    iconColor: LauncherStyle.muted,
                                    collapsedIconColor: LauncherStyle.muted,
                                    title: const Text('Command details',
                                        style: _label),
                                    children: [
                                      Align(
                                          alignment: Alignment.centerLeft,
                                          child: SelectableText(
                                              formatExecutionCommand(
                                                  summary.command),
                                              style: _mono)),
                                      const SizedBox(height: 6),
                                      Align(
                                          alignment: Alignment.centerLeft,
                                          child: SelectableText(
                                              summary.directory,
                                              style: _mono)),
                                    ],
                                  )),
                              if (summary.error != null) ...[
                                _Notice(summary.error!),
                                const SizedBox(height: 10)
                              ],
                              if (summary.outputTruncated) ...[
                                const _Notice(
                                    'Showing the latest output (128 KiB per stream).'),
                                const SizedBox(height: 10)
                              ],
                            ]))),
                const SizedBox(height: 8),
                Expanded(
                    child: ExecutionOutputView(
                        detail: detail, onError: controller.reportError)),
              ])),
    );
  }
}

class _Notice extends StatelessWidget {
  const _Notice(this.message, {this.onDismiss});
  final String message;
  final VoidCallback? onDismiss;
  @override
  Widget build(BuildContext context) => Container(
      padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 8),
      decoration: BoxDecoration(
          color: LauncherStyle.warning.withValues(alpha: .07),
          borderRadius: BorderRadius.circular(8)),
      child: Row(crossAxisAlignment: CrossAxisAlignment.start, children: [
        const Icon(Icons.info_outline_rounded,
            size: 14, color: LauncherStyle.warning),
        const SizedBox(width: 7),
        Expanded(
            child: Text(message,
                style: _body.copyWith(color: LauncherStyle.warning))),
        if (onDismiss != null)
          IconButton(
              tooltip: 'Dismiss message',
              onPressed: onDismiss,
              icon: const Icon(Icons.close_rounded,
                  size: 16, color: LauncherStyle.warning)),
      ]));
}

class _EmptyTasks extends StatelessWidget {
  const _EmptyTasks();
  @override
  Widget build(BuildContext context) => const Center(
      child: Padding(
          padding: EdgeInsets.all(32),
          child: Column(mainAxisSize: MainAxisSize.min, children: [
            _IconPlate(icon: Icons.terminal_rounded),
            SizedBox(height: 14),
            Text('No tasks yet.',
                style: TextStyle(
                    fontSize: 14,
                    fontWeight: FontWeight.w600,
                    color: LauncherStyle.primary)),
            SizedBox(height: 7),
            Text('Choose a project Build, Test or Run action in the Launcher.',
                textAlign: TextAlign.center, style: _body),
          ])));
}
