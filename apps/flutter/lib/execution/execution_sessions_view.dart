import '../l10n/app_language.dart';
import '../infrastructure/desktop_environment.dart';
import '../ui/app_icons.dart';
import 'dart:async';
import 'package:flutter/material.dart';
import '../ui/app_components.dart';
import 'package:flutter/services.dart';
import 'execution_output_view.dart';
import 'execution_command.dart';
import '../ui/app_style.dart';
import '../src/rust/api.dart';
import 'execution_sessions_controller.dart';

const _label = TextStyle(fontSize: 11, color: AppStyle.muted);
const _body = TextStyle(fontSize: 12, color: AppStyle.secondary);
TextStyle get _mono => TextStyle(
    fontFamily: DesktopEnvironment.current.monoFontFamily,
    fontFamilyFallback: ['monospace'],
    fontSize: DesignTokens.previewCodeSize,
    height: DesignTokens.previewCodeLineHeight,
    color: AppStyle.primary);

String taskStatus(ExecutionSessionStatusDto status, [AppLocalizations? l10n]) {
  final contextLabels = l10n ?? lookupAppLocalizations(const Locale('en'));
  return switch (status) {
    ExecutionSessionStatusDto.running => contextLabels.running,
    ExecutionSessionStatusDto.stopping => contextLabels.stopping,
    ExecutionSessionStatusDto.succeeded => contextLabels.completed,
    ExecutionSessionStatusDto.failed => contextLabels.failed,
    ExecutionSessionStatusDto.stopped => contextLabels.stopped,
  };
}

Color _statusColor(ExecutionSessionStatusDto status) => switch (status) {
      ExecutionSessionStatusDto.running => AppStyle.accent,
      ExecutionSessionStatusDto.succeeded => AppStyle.success,
      ExecutionSessionStatusDto.failed => AppStyle.danger,
      _ => AppStyle.muted,
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
  final _taskActions = GlobalKey<PopupMenuButtonState<String>>();
  final _windowActions = GlobalKey<PopupMenuButtonState<String>>();
  String? _lastSelection;
  String _lastFilter = '';
  double get _rowHeight {
    final scale = MediaQuery.textScalerOf(context).scale(12) / 12;
    // Larger text can wrap the status and exit code onto separate lines.
    return (scale > 1.2 ? 100 : 86) * scale;
  }

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
          DesktopEnvironment.current.command(LogicalKeyboardKey.keyK): () {
            if (!_search.value.composing.isCollapsed || controller.busy) return;
            (_taskActions.currentState ?? _windowActions.currentState)
                ?.showButtonMenu();
          },
          DesktopEnvironment.current.command(LogicalKeyboardKey.keyF):
              focusSearch,
          DesktopEnvironment.current.command(LogicalKeyboardKey.keyL): () =>
              unawaited(widget.onOpenLauncher()),
          DesktopEnvironment.current.command(LogicalKeyboardKey.keyR): () =>
              unawaited(controller.refresh()),
          DesktopEnvironment.current.command(LogicalKeyboardKey.period): () =>
              unawaited(controller.stopSelected()),
          DesktopEnvironment.current.command(LogicalKeyboardKey.arrowUp,
              alt: true): () => controller.moveSelection(-1),
          DesktopEnvironment.current.command(LogicalKeyboardKey.arrowDown,
              alt: true): () => controller.moveSelection(1),
          const SingleActivator(LogicalKeyboardKey.escape): _hideOrClear,
        },
        child: Scaffold(
          body: Material(
            color: AppStyle.window,
            child: Column(children: [
              // The native material extends behind the title bar. Leave room for its traffic lights.
              SizedBox(
                  height:
                      DesktopEnvironment.current.titleBarInset == 0 ? 0 : 28),
              _toolbar(running),
              if (controller.error != null)
                Padding(
                    padding: const EdgeInsets.fromLTRB(18, 0, 18, 12),
                    child: _Notice(controller.error!,
                        onDismiss: controller.dismissError)),
              const Divider(height: 1, color: AppStyle.divider),
              Expanded(
                  child: controller.sessions.isEmpty
                      ? const Focus(autofocus: true, child: _EmptyTasks())
                      : Row(
                          crossAxisAlignment: CrossAxisAlignment.stretch,
                          children: [
                              _sidebar(tasks),
                              const VerticalDivider(
                                  width: 1, color: AppStyle.divider),
                              Expanded(
                                  child: detail == null
                                      ? (tasks.isEmpty
                                          ? Center(
                                              child: Text(
                                                  context.l10n.noMatchingTasks,
                                                  style: _body))
                                          : const Center(
                                              child: SizedBox.square(
                                                  dimension: 18,
                                                  child:
                                                      CircularProgressIndicator(
                                                          strokeWidth: 1.5,
                                                          color: AppStyle
                                                              .accent))))
                                      : _TaskDetail(
                                          key: ValueKey(detail.summary.id),
                                          detail: detail,
                                          actionMenu: _taskActions,
                                          controller: controller)),
                            ])),
              const Divider(height: 1, color: AppStyle.divider),
              Padding(
                  padding:
                      const EdgeInsets.symmetric(horizontal: 18, vertical: 9),
                  child: Row(children: [
                    Expanded(
                        child: Text(context.l10n.tasksKeepRunningWhenHidden,
                            style: _label,
                            maxLines: 1,
                            overflow: TextOverflow.ellipsis)),
                    TextButton(
                        onPressed: () => showDialog<void>(
                            context: context,
                            builder: (context) => AlertDialog(
                                    title: Text(context.l10n.keyboardHelp),
                                    content: Text(context.l10n.taskKeyboardHelp(
                                        DesktopEnvironment.current
                                            .commandLabel('F'),
                                        DesktopEnvironment.current
                                            .commandLabel('.'))),
                                    actions: [
                                      TextButton(
                                          onPressed: () =>
                                              Navigator.pop(context),
                                          child: Text(context.l10n.done))
                                    ])),
                        child: Text(context.l10n.keyboardHelp)),
                  ])),
            ]),
          ),
        ));
  }

  Widget _toolbar(int running) => AppToolbar(
          title: context.l10n.runningTasks,
          subtitle: running == 0
              ? (controller.sessions.isEmpty
                  ? context.l10n.noRunningTasks
                  : context.l10n.allTasksFinished)
              : context.l10n.runningCount(running),
          actions: [
            PopupMenuButton<String>(
                key: _windowActions,
                tooltip: context.l10n.toolOptions,
                onSelected: (action) {
                  if (action == 'launcher') {
                    unawaited(widget.onOpenLauncher());
                  } else {
                    unawaited(controller.refresh());
                  }
                },
                itemBuilder: (_) => [
                      PopupMenuItem(
                          value: 'refresh',
                          child: Text(context.l10n.shortcutHint(
                              context.l10n.refresh,
                              DesktopEnvironment.current.commandLabel('R')))),
                      PopupMenuItem(
                          value: 'launcher',
                          child: Text(context.l10n.shortcutHint(
                              context.l10n.launcher,
                              DesktopEnvironment.current.commandLabel('L')))),
                    ],
                child: Padding(
                    padding: const EdgeInsets.all(8),
                    child: Row(mainAxisSize: MainAxisSize.min, children: [
                      const Icon(AppIcons.more, size: 16),
                      const SizedBox(width: 6),
                      Text(context.l10n.toolOptions, style: AppStyle.caption),
                    ]))),
          ]);

  Widget _sidebar(List<ExecutionSessionSummaryDto> tasks) => Container(
      width: 252,
      color: AppStyle.sidebar,
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
                cursorColor: AppStyle.accent,
                decoration: InputDecoration(
                  hintText: context.l10n.findATask,
                  hintStyle: _label,
                  prefixIcon: const Icon(AppIcons.search,
                      size: 16, color: AppStyle.muted),
                  prefixIconConstraints:
                      const BoxConstraints(minWidth: 30, minHeight: 34),
                  suffixIcon: controller.filter.isEmpty
                      ? null
                      : IconButton(
                          tooltip: context.l10n.clearSearch,
                          icon: const Icon(AppIcons.close, size: 15),
                          color: AppStyle.secondary,
                          onPressed: () => controller.setFilter('')),
                  suffixIconConstraints:
                      const BoxConstraints.tightFor(width: 34, height: 34),
                  filled: true,
                  fillColor: AppStyle.field,
                  contentPadding:
                      const EdgeInsets.symmetric(vertical: 9, horizontal: 8),
                  isDense: true,
                  enabledBorder: OutlineInputBorder(
                      borderRadius:
                          BorderRadius.circular(AppStyle.controlRadius),
                      borderSide:
                          const BorderSide(color: AppStyle.border, width: .8)),
                  focusedBorder: OutlineInputBorder(
                      borderRadius:
                          BorderRadius.circular(AppStyle.controlRadius),
                      borderSide: const BorderSide(
                          color: AppStyle.selectedBorder, width: 1.5)),
                ))),
        Padding(
            padding: const EdgeInsets.fromLTRB(16, 4, 16, 10),
            child: Row(children: [
              Text(context.l10n.tasks, style: _label),
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
        child: AppSelectionSurface(
          selected: selected,
          onTap: onTap,
          child: Padding(
              padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 8),
              child: Row(children: [
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
                                  fontWeight: FontWeight.w500,
                                  color: AppStyle.primary))),
                      const SizedBox(height: 4),
                      Tooltip(
                          message: session.directory,
                          child: Text(project,
                              maxLines: 1,
                              overflow: TextOverflow.ellipsis,
                              style: selected
                                  ? _body.copyWith(color: AppStyle.primary)
                                  : _body)),
                      const SizedBox(height: 4),
                      Wrap(
                          spacing: 8,
                          crossAxisAlignment: WrapCrossAlignment.center,
                          children: [
                            _TaskStatus(session.status, selected: selected),
                            if (session.exitCode != null)
                              Text(context.l10n.exitCode('${session.exitCode}'),
                                  style: selected
                                      ? _label.copyWith(color: AppStyle.primary)
                                      : _label),
                          ]),
                    ])),
              ])),
        ),
      ),
    );
  }
}

class _TaskStatus extends StatelessWidget {
  const _TaskStatus(this.status, {this.selected = false});
  final ExecutionSessionStatusDto status;
  final bool selected;
  @override
  Widget build(BuildContext context) =>
      AppStatusBadge(taskStatus(status, context.l10n),
          color: selected ? AppStyle.primary : _statusColor(status),
          icon: switch (status) {
            ExecutionSessionStatusDto.running => AppIcons.running,
            ExecutionSessionStatusDto.stopping => AppIcons.hourglass,
            ExecutionSessionStatusDto.succeeded => AppIcons.success,
            ExecutionSessionStatusDto.failed => AppIcons.error,
            ExecutionSessionStatusDto.stopped => AppIcons.paused,
          });
}

class _TaskDetail extends StatelessWidget {
  const _TaskDetail(
      {super.key,
      required this.detail,
      required this.controller,
      required this.actionMenu});
  final ExecutionSessionDetailDto detail;
  final GlobalKey<PopupMenuButtonState<String>> actionMenu;
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
                                                    color: AppStyle.primary)))),
                                    const SizedBox(width: 16),
                                    if (active)
                                      Tooltip(
                                          message: context.l10n.shortcutHint(
                                              context.l10n.stop,
                                              DesktopEnvironment.current
                                                  .commandLabel('.')),
                                          child: TextButton.icon(
                                              onPressed: controller.busy || summary.status == ExecutionSessionStatusDto.stopping
                                                  ? null
                                                  : () => controller
                                                      .stop(summary.id),
                                              icon: const Icon(AppIcons.stop,
                                                  size: 16),
                                              label: Text(
                                                  summary.status == ExecutionSessionStatusDto.stopping
                                                      ? context
                                                          .l10n.stoppingProgress
                                                      : context.l10n.stop),
                                              style: TextButton.styleFrom(
                                                  foregroundColor: AppStyle.danger,
                                                  backgroundColor: AppStyle.danger.withValues(alpha: .08),
                                                  disabledForegroundColor: AppStyle.muted,
                                                  minimumSize: const Size(0, 24),
                                                  padding: const EdgeInsets.symmetric(horizontal: 10),
                                                  textStyle: _body,
                                                  shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(AppStyle.controlRadius), side: const BorderSide(color: AppStyle.divider, width: .5))))),
                                    PopupMenuButton<String>(
                                      key: actionMenu,
                                      tooltip: context.l10n.moreActions,
                                      enabled: !controller.busy,
                                      color: AppStyle.card,
                                      icon: const Icon(AppIcons.more,
                                          size: 18, color: AppStyle.secondary),
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
                                                  .showSnackBar(SnackBar(
                                                      content: Text(context
                                                          .l10n.commandCopied),
                                                      duration: Duration(
                                                          seconds: 2)));
                                            }
                                          } catch (_) {
                                            if (!context.mounted) return;
                                            controller.reportError(context.l10n
                                                .commandCouldNotBeCopiedTryAgain);
                                          }
                                        }
                                      },
                                      itemBuilder: (_) => [
                                        PopupMenuItem(
                                            value: 'copy',
                                            child: Text(
                                                context.l10n.copyCommand,
                                                style: _body)),
                                        if (!active)
                                          PopupMenuItem(
                                              value: 'remove',
                                              child: Text(
                                                  context.l10n.removeResult,
                                                  style: _body)),
                                      ],
                                    ),
                                  ]),
                              const SizedBox(height: 8),
                              Wrap(
                                  spacing: 12,
                                  crossAxisAlignment: WrapCrossAlignment.center,
                                  children: [
                                    _TaskStatus(summary.status),
                                    if (summary.exitCode != null)
                                      Text(
                                          context.l10n
                                              .exitCode('${summary.exitCode}'),
                                          style: _label),
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
                                    iconColor: AppStyle.muted,
                                    collapsedIconColor: AppStyle.muted,
                                    title: Text(context.l10n.commandDetails,
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
                                _Notice(summary.error!, kind: NoticeKind.error),
                                const SizedBox(height: 10)
                              ],
                              if (summary.outputTruncated) ...[
                                _Notice(
                                    context
                                        .l10n.showingTheLatestOutput128KiBPer,
                                    kind: NoticeKind.info),
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

class _Notice extends AppNotice {
  const _Notice(super.message, {super.onDismiss, super.kind});
}

class _EmptyTasks extends StatelessWidget {
  const _EmptyTasks();
  @override
  Widget build(BuildContext context) => AppEmptyState(
      icon: AppIcons.terminal,
      title: context.l10n.noTasksYet,
      message: context.l10n.chooseAProjectBuildTestOrRun);
}
