import 'dart:async';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import '../launcher/launcher_style.dart';
import '../src/rust/api.dart';
import 'execution_sessions_controller.dart';

/// Follow live output only while the reader remains at the bottom.
class ExecutionOutputView extends StatefulWidget {
  const ExecutionOutputView(
      {super.key, required this.detail, required this.onError});
  final ExecutionSessionDetailDto detail;
  final void Function(String) onError;
  @override
  State<ExecutionOutputView> createState() => _ExecutionOutputViewState();
}

class _ExecutionOutputViewState extends State<ExecutionOutputView> {
  final _scroll = ScrollController();
  bool _diagnostics = false,
      _following = true,
      _scrolling = false,
      _copied = false;
  Timer? _copyTimer;
  String get _text =>
      _diagnostics ? widget.detail.stderr : widget.detail.stdout;
  bool get _active => isTaskActive(widget.detail.summary.status);

  @override
  void initState() {
    super.initState();
    _diagnostics =
        widget.detail.summary.status == ExecutionSessionStatusDto.failed &&
            widget.detail.stderr.isNotEmpty;
    _scroll.addListener(_onScroll);
    _toLatest();
  }

  void _onScroll() {
    if (_scrolling || !_scroll.hasClients) return;
    final atBottom = _scroll.position.extentAfter <= 24;
    if (_following != atBottom) setState(() => _following = atBottom);
  }

  @override
  void didUpdateWidget(ExecutionOutputView oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (_following &&
        (oldWidget.detail.stdout != widget.detail.stdout ||
            oldWidget.detail.stderr != widget.detail.stderr)) {
      _toLatest();
    }
  }

  void _toLatest() {
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!mounted || !_scroll.hasClients || !_following) return;
      _scrolling = true;
      _scroll.jumpTo(_scroll.position.maxScrollExtent);
      _scrolling = false;
    });
  }

  void _switchStream(bool diagnostics) {
    if (_diagnostics == diagnostics) return;
    setState(() {
      _diagnostics = diagnostics;
      _following = true;
      _copied = false;
    });
    _toLatest();
  }

  Future<void> _copy() async {
    try {
      final copiedStream = _diagnostics;
      // Capture before awaiting, so switching streams never changes what gets copied.
      await Clipboard.setData(ClipboardData(text: _text));
      if (!mounted || _diagnostics != copiedStream) return;
      _copyTimer?.cancel();
      setState(() => _copied = true);
      _copyTimer = Timer(const Duration(seconds: 2), () {
        if (mounted) setState(() => _copied = false);
      });
    } catch (_) {
      widget.onError('Output could not be copied. Try again.');
    }
  }

  @override
  void dispose() {
    _copyTimer?.cancel();
    _scroll.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => Container(
        decoration: BoxDecoration(
            color: const Color(0x6510141d),
            borderRadius: BorderRadius.circular(11),
            border: Border.all(color: LauncherStyle.divider, width: .6)),
        clipBehavior: Clip.antiAlias,
        child:
            Column(crossAxisAlignment: CrossAxisAlignment.stretch, children: [
          Padding(
              padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 3),
              child: Row(children: [
                _tab('Output', false),
                _tab('Diagnostics', true),
                const Spacer(),
                Tooltip(
                    message: _following
                        ? 'Follow new output'
                        : 'Jump to latest output',
                    child: TextButton.icon(
                        key: const Key('follow-output'),
                        onPressed: () {
                          setState(() => _following = true);
                          _toLatest();
                        },
                        icon: Icon(
                            _following
                                ? Icons.vertical_align_bottom_rounded
                                : Icons.arrow_downward_rounded,
                            size: 15),
                        label: Text(_following ? 'Following' : 'Latest'),
                        style: TextButton.styleFrom(
                            foregroundColor: _following
                                ? LauncherStyle.muted
                                : LauncherStyle.accent,
                            textStyle: const TextStyle(fontSize: 11),
                            minimumSize: const Size(0, 32),
                            padding:
                                const EdgeInsets.symmetric(horizontal: 8)))),
                IconButton(
                    tooltip: _copied ? 'Copied' : 'Copy output',
                    onPressed: _text.isEmpty ? null : _copy,
                    constraints:
                        const BoxConstraints.tightFor(width: 34, height: 34),
                    padding: EdgeInsets.zero,
                    icon: Icon(
                        _copied ? Icons.check_rounded : Icons.copy_rounded,
                        size: 15,
                        color: _copied
                            ? LauncherStyle.success
                            : LauncherStyle.secondary)),
              ])),
          const Divider(height: 1, color: LauncherStyle.divider),
          Expanded(
              child: Scrollbar(
                  controller: _scroll,
                  child: SingleChildScrollView(
                    key: const Key('task-output-scroll'),
                    controller: _scroll,
                    primary: false,
                    padding: const EdgeInsets.all(14),
                    child: SelectableText(
                        _text.isEmpty
                            ? (_diagnostics
                                ? 'No diagnostic output.'
                                : _active
                                    ? 'Waiting for output…'
                                    : 'No standard output.')
                            : _text,
                        key: const Key('task-output-text'),
                        style: TextStyle(
                            fontFamily: 'Menlo',
                            fontFamilyFallback: const ['monospace'],
                            fontSize: 12,
                            height: 1.65,
                            color: _diagnostics
                                ? LauncherStyle.warning
                                : LauncherStyle.secondary)),
                  ))),
        ]),
      );

  Widget _tab(String label, bool diagnostics) => Semantics(
      selected: _diagnostics == diagnostics,
      child: TextButton(
          onPressed: () => _switchStream(diagnostics),
          style: TextButton.styleFrom(
              foregroundColor: _diagnostics == diagnostics
                  ? LauncherStyle.primary
                  : LauncherStyle.muted,
              backgroundColor: _diagnostics == diagnostics
                  ? const Color(0x12ffffff)
                  : Colors.transparent,
              minimumSize: const Size(0, 32),
              padding: const EdgeInsets.symmetric(horizontal: 8),
              textStyle:
                  const TextStyle(fontSize: 11.5, fontWeight: FontWeight.w500),
              shape: RoundedRectangleBorder(
                  borderRadius: BorderRadius.circular(6))),
          child: Row(mainAxisSize: MainAxisSize.min, children: [
            Text(label),
            if (diagnostics &&
                widget.detail.stderr.isNotEmpty &&
                !_diagnostics) ...[
              const SizedBox(width: 5),
              Container(
                  width: 5,
                  height: 5,
                  decoration: const BoxDecoration(
                      color: LauncherStyle.warning, shape: BoxShape.circle)),
            ],
          ])));
}
