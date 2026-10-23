import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import '../../infrastructure/clipboard_history_backend.dart';
import '../../infrastructure/presentation_metrics.dart';
import '../../infrastructure/tool_window_lifecycle.dart';
import '../../l10n/app_language.dart';
import '../../ui/app_style.dart';
import 'clipboard_history_presentation.dart';
import 'clipboard_history_session.dart';
import 'clipboard_shelf_page.dart';

/// An independent presentation of the same Rust history, with its own query and
/// selection. It never initializes another Application or enables recording.
class ClipboardShelfApp extends StatefulWidget {
  const ClipboardShelfApp(
      {super.key,
      this.backend,
      this.channel = const MethodChannel('anywhere/clipboard-shelf'),
      this.refreshInterval = const Duration(seconds: 1)});
  final ClipboardHistoryBackend? backend;
  final MethodChannel channel;
  final Duration refreshInterval;
  @override
  State<ClipboardShelfApp> createState() => _ClipboardShelfAppState();
}

class _ClipboardShelfAppState extends State<ClipboardShelfApp> {
  late final session = ClipboardHistorySession(
      widget.backend ?? RustClipboardHistoryBackend(),
      rowExtent: clipboardShelfRowHeight,
      previewEnabled: false,
      queryOperation: PresentationOperation.shelfQuery);
  Timer? _timer;
  bool _visible = true, _polling = false;
  int _presentation = 0, _visibilityEpoch = 0;
  @override
  void initState() {
    super.initState();
    session.bindInteraction(() => mounted && _visible);
    widget.channel.setMethodCallHandler((call) async {
      if (call.method == 'prepareToClose') {
        await prepareToolWindowToClose();
      } else if (call.method == 'visibility') {
        ++_visibilityEpoch;
        _visibility(call.arguments == true);
      } else if (call.method == 'present') {
        ++_visibilityEpoch;
        _visibility(true);
        setState(() => ++_presentation);
        unawaited(session.refresh());
      }
    });
    _timer = Timer.periodic(widget.refreshInterval, (_) => unawaited(_poll()));
    unawaited(_ready());
  }

  Future<void> _ready() async {
    final epoch = _visibilityEpoch;
    try {
      final visible = await widget.channel.invokeMethod<bool>('ready');
      if (mounted && epoch == _visibilityEpoch && visible != null) {
        _visibility(visible);
      }
    } catch (_) {
      // The view remains usable if an older host lacks the visibility handshake.
    }
  }

  void _visibility(bool visible) {
    if (!mounted || _visible == visible) return;
    setState(() => _visible = visible);
    if (visible) unawaited(session.refresh());
  }

  Future<void> _poll() async {
    if (!_visible || _polling || !mounted) return;
    // Do not interfere with an active input-method composition.
    if (!session.query.value.composing.isCollapsed) return;
    _polling = true;
    try {
      await session.refresh(background: true);
    } finally {
      _polling = false;
    }
  }

  Future<void> _close() async {
    try {
      await widget.channel.invokeMethod<void>('close');
    } catch (_) {
      if (mounted) {
        session.reportNotice(context.l10n.theWindowCouldNotBeClosedUse);
      }
    }
  }

  Future<void> _returnToTarget() async {
    // Only the explicit copy-and-return command changes the destination focus.
    // The shelf stays visible; never simulate Paste in another app.
    final epoch = _visibilityEpoch;
    String? outcome;
    try {
      outcome = await widget.channel.invokeMethod<String>('returnToTarget');
    } catch (_) {
      // Keep the successfully copied payload and report the focus failure below.
    }
    if (!mounted || !_visible || epoch != _visibilityEpoch) return;
    if (outcome == 'returned' || outcome == 'skipped') return;
    session.reportReturnFailure(unavailable: outcome == 'unavailable');
  }

  @override
  void dispose() {
    _timer?.cancel();
    widget.channel.setMethodCallHandler(null);
    session.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => LocalizedApp(
        debugShowCheckedModeBanner: false,
        theme: AppStyle.theme,
        builder: AppStyle.windowBuilder,
        home: Scaffold(
            backgroundColor: AppStyle.surface,
            body: SafeArea(
                child: Padding(
              padding: const EdgeInsets.only(top: 24),
              child: ListenableBuilder(
                  listenable: session,
                  builder: (context, _) => ClipboardShelfPage(
                      session: session,
                      onReturnToTarget: _returnToTarget,
                      onClose: () => unawaited(_close()),
                      presentation: _presentation,
                      visible: _visible)),
            ))),
      );
}
