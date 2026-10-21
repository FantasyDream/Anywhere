import 'dart:async';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import '../../ui/app_style.dart';
import '../../infrastructure/clipboard_history_backend.dart';
import 'clipboard_shelf_page.dart';
import 'clipboard_history_presentation.dart';
import 'clipboard_history_session.dart';

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
      previewEnabled: false);
  Timer? _timer;
  bool _visible = true, _polling = false;
  int _presentation = 0, _visibilityEpoch = 0;
  @override
  void initState() {
    super.initState();
    session.bindInteraction(() => mounted && _visible);
    widget.channel.setMethodCallHandler((call) async {
      if (call.method == 'visibility') {
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
        session.reportNotice(
            'The window could not be closed. Use the window close button.');
      }
    }
  }

  Future<void> _copied() async {
    // The shelf stays visible. Return keyboard focus to the last destination so
    // the normal paste shortcut works; never simulate Paste in another app.
    try {
      await widget.channel.invokeMethod<void>('returnToTarget');
    } catch (_) {
      // Copy has already succeeded; the user can switch to the destination.
    }
  }

  @override
  void dispose() {
    _timer?.cancel();
    widget.channel.setMethodCallHandler(null);
    session.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => MaterialApp(
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
                      onCopied: _copied,
                      onClose: () => unawaited(_close()),
                      presentation: _presentation,
                      visible: _visible)),
            ))),
      );
}
