import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter/foundation.dart';

import 'src/rust/api.dart';

typedef Activate = Future<LauncherStateDto?> Function();
typedef UpdateInput = Future<LauncherStateDto?> Function(
  int session,
  String input,
  int revision,
);
typedef Execute = Future<ActionResultDto> Function(int session, String action);

class LauncherApp extends StatelessWidget {
  const LauncherApp({
    super.key,
    required this.activate,
    required this.updateInput,
    required this.execute,
    required this.deactivate,
    required this.window,
    this.startupMessage = '',
  });
  final Activate activate;
  final UpdateInput updateInput;
  final Execute execute;
  final Future<void> Function(int) deactivate;
  final MethodChannel window;
  final String startupMessage;

  @override
  Widget build(BuildContext context) => MaterialApp(
        debugShowCheckedModeBanner: false,
        title: 'Context Launcher',
        theme: ThemeData(
          brightness: Brightness.dark,
          useMaterial3: true,
          colorScheme: ColorScheme.fromSeed(
            seedColor: const Color(0xffa7b4cb),
            brightness: Brightness.dark,
          ),
          scaffoldBackgroundColor: Colors.transparent,
          fontFamily: defaultTargetPlatform == TargetPlatform.windows
              ? 'Segoe UI'
              : '.AppleSystemUIFont',
        ),
        home: LauncherScreen(
          activate: activate,
          updateInput: updateInput,
          execute: execute,
          deactivate: deactivate,
          window: window,
          startupMessage: startupMessage,
        ),
      );
}

class LauncherScreen extends StatefulWidget {
  const LauncherScreen({
    super.key,
    required this.activate,
    required this.updateInput,
    required this.execute,
    required this.deactivate,
    required this.window,
    required this.startupMessage,
  });
  final Activate activate;
  final UpdateInput updateInput;
  final Execute execute;
  final Future<void> Function(int) deactivate;
  final MethodChannel window;
  final String startupMessage;
  @override
  State<LauncherScreen> createState() => _LauncherScreenState();
}

class _LauncherScreenState extends State<LauncherScreen> {
  static const _rowHeight = 52.0;
  static const _maxRows = 7;
  final _input = TextEditingController();
  final _focus = FocusNode();
  final _scroll = ScrollController();
  LauncherStateDto? _state;
  int _selected = 0;
  int _revision = 0;
  int _generation = 0;
  bool _visible = false;
  bool _activating = false;
  bool _resolving = false;
  bool _executing = false;
  String? _message;
  String? _output;

  // Keep the last committed rows while resolving. Revision checks prevent their
  // execution; only a committed result changes the native window's height.
  List<ActionDto> get _actions => _state?.actions ?? [];
  String? get _notice =>
      _message ??
      ((_state?.diagnostics.isNotEmpty ?? false)
          ? _state!.diagnostics.join('\n')
          : null);
  double get _previewHeight =>
      ((_output?.split('\n').length ?? 0).clamp(3, 14) * 20.0) + 24;
  double get _height {
    final body = _output != null
        ? _previewHeight
        : _actions.length.clamp(0, _maxRows) * _rowHeight + 8;
    return (64.0 +
            (_notice == null ? 0 : 44) +
            (_output != null || _actions.isNotEmpty ? 62 + body : 0))
        .clamp(64.0, 520.0);
  }

  @override
  void initState() {
    super.initState();
    widget.window.setMethodCallHandler((call) async {
      if (call.method == 'toggle') {
        if (_visible || _activating) {
          await _hide();
        } else {
          await _activate();
        }
      } else if (call.method == 'dismiss' &&
          (call.arguments as Map?)?['generation'] == _generation) {
        await _hide(hostAlreadyHidden: true);
      }
    });
    Future<void>.microtask(() async {
      if (mounted) await widget.window.invokeMethod<void>('ready');
    });
  }

  Future<void> _syncWindow({bool show = false}) async {
    if (!mounted || !_visible) return;
    await widget.window.invokeMethod<void>(show ? 'show' : 'resize', {
      'generation': _generation,
      'height': _height,
    });
  }

  Future<void> _activate() async {
    _activating = true;
    final generation = ++_generation;
    try {
      final state = await widget.activate();
      if (!mounted || generation != _generation || state == null) {
        if (state != null) await widget.deactivate(state.sessionId);
        return;
      }
      _input.clear();
      setState(() {
        _state = state;
        _revision = state.revision;
        _selected = 0;
        _message = widget.startupMessage.isEmpty ? null : widget.startupMessage;
        _output = null;
        _visible = true;
        _resolving = false;
        _executing = false;
      });
      // Capture/ranking finish before showing; a hidden window need not render a frame.
      await _syncWindow(show: true);
      if (mounted && _visible && generation == _generation) {
        _focus.requestFocus();
      }
    } catch (error) {
      if (mounted && generation == _generation) {
        setState(() {
          _message = '$error';
          _visible = true;
        });
        await _syncWindow(show: true);
        if (mounted && _visible && generation == _generation) {
          _focus.requestFocus();
        }
      }
    } finally {
      if (generation == _generation) _activating = false;
    }
  }

  Future<void> _hide({bool hostAlreadyHidden = false}) async {
    final generation = _generation;
    final session = _state?.sessionId;
    ++_generation; // Invalidate input, execution and activation responses before any await.
    _focus.unfocus();
    _input.clear();
    setState(() {
      _visible = false;
      _activating = false;
      _resolving = false;
      _executing = false;
      _state = null;
      _output = null;
      _message = null;
    });
    final invalidation =
        session == null ? Future<void>.value() : widget.deactivate(session);
    if (!hostAlreadyHidden) {
      await widget.window
          .invokeMethod<void>('hide', {'generation': generation});
    }
    await invalidation;
  }

  Future<void> _update(String input) async {
    final session = _state?.sessionId;
    if (session == null || !_visible) return;
    final revision = ++_revision;
    final generation = _generation;
    setState(() {
      _resolving = true;
      _executing = false;
      _output = null;
      _message = null;
      _selected = 0;
    });
    try {
      final state = await widget.updateInput(session, input, revision);
      if (!mounted ||
          !_visible ||
          generation != _generation ||
          revision != _revision ||
          state == null ||
          state.sessionId != session ||
          state.revision != revision) {
        return;
      }
      setState(() {
        _state = state;
        _resolving = false;
      });
      if (_scroll.hasClients) _scroll.jumpTo(0);
      await _syncWindow();
    } catch (error) {
      if (mounted &&
          _visible &&
          generation == _generation &&
          revision == _revision) {
        setState(() {
          _message = '$error';
          _resolving = false;
        });
        await _syncWindow();
      }
    }
  }

  Future<void> _execute() async {
    final state = _state;
    if (state == null ||
        _actions.isEmpty ||
        _resolving ||
        _executing ||
        !_visible ||
        state.revision != _revision) {
      return;
    }
    final generation = _generation;
    final revision = _revision;
    setState(() => _executing = true);
    try {
      final result =
          await widget.execute(state.sessionId, _actions[_selected].id);
      if (!mounted ||
          !_visible ||
          generation != _generation ||
          revision != _revision ||
          result.sessionId != state.sessionId ||
          result.revision != revision) {
        return;
      }
      if (result.success && result.closeWindow) {
        await _hide();
      } else {
        setState(() {
          _message = result.message == 'Done' ? null : result.message;
          _output = result.outputText;
        });
        await _syncWindow();
      }
    } catch (error) {
      if (mounted &&
          _visible &&
          generation == _generation &&
          revision == _revision) {
        setState(() => _message = '$error');
        await _syncWindow();
      }
    } finally {
      if (mounted && generation == _generation && revision == _revision) {
        setState(() => _executing = false);
      }
    }
  }

  void _move(int direction) {
    if (_resolving || _actions.isEmpty) return;
    setState(() {
      _selected = (_selected + direction) % _actions.length;
      _output = null;
    });
    _syncWindow();
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!mounted || !_visible || !_scroll.hasClients) return;
      final top = _selected * _rowHeight;
      final bottom = top + _rowHeight;
      if (top < _scroll.offset) _scroll.jumpTo(top);
      if (bottom > _scroll.offset + _scroll.position.viewportDimension) {
        _scroll.jumpTo((bottom - _scroll.position.viewportDimension)
            .clamp(0, _scroll.position.maxScrollExtent));
      }
    });
  }

  KeyEventResult _onKey(FocusNode node, KeyEvent event) {
    if (event is! KeyDownEvent && event is! KeyRepeatEvent) {
      return KeyEventResult.ignored;
    }
    // Leave candidate selection to the input method until composition has finished.
    if (!_input.value.composing.isCollapsed) return KeyEventResult.ignored;
    if (event.logicalKey == LogicalKeyboardKey.escape) {
      _hide();
    } else if (event.logicalKey == LogicalKeyboardKey.arrowDown) {
      _move(1);
    } else if (event.logicalKey == LogicalKeyboardKey.arrowUp) {
      _move(-1);
    } else if (event.logicalKey == LogicalKeyboardKey.enter ||
        event.logicalKey == LogicalKeyboardKey.numpadEnter) {
      _execute();
    } else {
      return KeyEventResult.ignored;
    }
    return KeyEventResult.handled;
  }

  @override
  void dispose() {
    ++_generation;
    widget.window.setMethodCallHandler(null);
    _input.dispose();
    _focus.dispose();
    _scroll.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final actions = _actions;
    final hasPanel = _output != null || actions.isNotEmpty;
    final labels =
        (_state?.contexts ?? []).map((c) => c.label).toSet().join(' · ');
    return Align(
      alignment: Alignment.topCenter,
      child: SizedBox(
        key: const Key('launcher-panel'),
        width: 648,
        height: _height,
        child: Material(
          // AppKit supplies the real behind-window blur; keep this tint translucent.
          color: const Color(0xcc1b1d23),
          shape: RoundedRectangleBorder(
            borderRadius: BorderRadius.circular(12),
            side: const BorderSide(color: Color(0x38ffffff), width: 0.7),
          ),
          clipBehavior: Clip.antiAlias,
          child: Focus(
            onKeyEvent: _onKey,
            child: Column(children: [
              SizedBox(
                height: 64,
                child: Padding(
                  padding: const EdgeInsets.symmetric(horizontal: 20),
                  child: Row(children: [
                    const Icon(Icons.search_rounded,
                        size: 24, color: Color(0xffd0d6e1)),
                    const SizedBox(width: 14),
                    Expanded(
                        child: TextField(
                      controller: _input,
                      focusNode: _focus,
                      autofocus: true,
                      style: const TextStyle(
                          fontSize: 18, color: Color(0xfff7f8fc)),
                      cursorColor: const Color(0xffd5d7df),
                      decoration: const InputDecoration(
                        hintText: 'Search for an action…',
                        hintStyle: TextStyle(
                            color: Color(0xffc2c7d2),
                            fontWeight: FontWeight.w400),
                        border: InputBorder.none,
                        isDense: true,
                      ),
                      onChanged: _update,
                    )),
                    const SizedBox(width: 12),
                    if (_resolving || _executing)
                      const SizedBox(
                          width: 14,
                          height: 14,
                          child: CircularProgressIndicator(
                              strokeWidth: 1.5, color: Color(0xffd0d6e1)))
                    else
                      _keycap('esc'),
                  ]),
                ),
              ),
              if (_notice != null)
                SizedBox(
                    height: 44,
                    child: Padding(
                      padding: const EdgeInsets.symmetric(
                          horizontal: 20, vertical: 5),
                      child: Tooltip(
                          message: _notice!,
                          child: Align(
                            alignment: Alignment.centerLeft,
                            child: Text(_notice!,
                                maxLines: 2,
                                overflow: TextOverflow.ellipsis,
                                style: const TextStyle(
                                    fontSize: 11, color: Color(0xffe3bc83))),
                          )),
                    )),
              if (hasPanel) ...[
                const Divider(
                    height: 1, thickness: 1, color: Color(0x24ffffff)),
                SizedBox(
                    height: 28,
                    child: Padding(
                      padding: const EdgeInsets.symmetric(horizontal: 18),
                      child: Row(children: [
                        Text(_output == null ? 'Suggested actions' : 'Result',
                            style: const TextStyle(
                                fontSize: 11,
                                fontWeight: FontWeight.w500,
                                color: Color(0xffd0d6e1))),
                        const SizedBox(width: 16),
                        Expanded(
                            child: Text(labels,
                                textAlign: TextAlign.right,
                                maxLines: 1,
                                overflow: TextOverflow.ellipsis,
                                style: const TextStyle(
                                    fontSize: 11, color: Color(0xffc2c7d2)))),
                      ]),
                    )),
                Expanded(
                    child: _output != null
                        ? SingleChildScrollView(
                            key: const Key('result-preview'),
                            padding: const EdgeInsets.symmetric(
                                horizontal: 20, vertical: 12),
                            child: SelectableText(_output!,
                                style: const TextStyle(
                                    fontFamily: 'monospace',
                                    fontSize: 12,
                                    height: 1.65,
                                    color: Color(0xffe2e3e8))),
                          )
                        : ListView.builder(
                            key: const Key('action-list'),
                            controller: _scroll,
                            padding: const EdgeInsets.fromLTRB(8, 0, 8, 8),
                            itemExtent: _rowHeight,
                            itemCount: actions.length,
                            itemBuilder: (context, index) =>
                                _actionRow(actions[index], index),
                          )),
                const Divider(
                    height: 1, thickness: 1, color: Color(0x24ffffff)),
                SizedBox(
                    height: 32,
                    child: Padding(
                      padding: const EdgeInsets.symmetric(horizontal: 16),
                      child: Row(children: [
                        const Icon(Icons.bolt_rounded,
                            size: 14, color: Color(0xffd0d6e1)),
                        const SizedBox(width: 6),
                        Text(
                            _output == null
                                ? '${actions.length} actions'
                                : 'JSON',
                            style: const TextStyle(
                                fontSize: 11, color: Color(0xffc2c7d2))),
                        const Spacer(),
                        Text(_output == null ? 'Run action' : 'Back to actions',
                            style: const TextStyle(
                                fontSize: 11, color: Color(0xffd0d6e1))),
                        const SizedBox(width: 8),
                        _keycap(_output == null ? '↵' : '↑ ↓'),
                      ]),
                    )),
              ],
            ]),
          ),
        ),
      ),
    );
  }

  Widget _actionRow(ActionDto action, int index) {
    final selected = index == _selected;
    return Semantics(
      selected: selected,
      button: true,
      child: Material(
        color: selected ? const Color(0x665c6070) : Colors.transparent,
        borderRadius: BorderRadius.circular(7),
        child: InkWell(
          borderRadius: BorderRadius.circular(7),
          onTap: () {
            if (_resolving) return;
            setState(() => _selected = index);
            _execute();
          },
          child: Padding(
            padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 6),
            child: Row(children: [
              Container(
                  width: 30,
                  height: 30,
                  decoration: BoxDecoration(
                      color: const Color(0x4d454955),
                      borderRadius: BorderRadius.circular(7)),
                  child: Icon(_icon(action.iconKey),
                      size: 17, color: const Color(0xffcad0de))),
              const SizedBox(width: 12),
              Expanded(
                  child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                mainAxisAlignment: MainAxisAlignment.center,
                children: [
                  Text(action.title,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: const TextStyle(
                          fontSize: 13,
                          fontWeight: FontWeight.w500,
                          color: Color(0xffebebef))),
                  if (action.subtitle != null)
                    Text(action.subtitle!,
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                        style: const TextStyle(
                            fontSize: 11,
                            height: 1.5,
                            color: Color(0xffd0d6e1))),
                ],
              )),
              if (selected) ...[
                const SizedBox(width: 12),
                const Icon(Icons.keyboard_return_rounded,
                    size: 16, color: Color(0xffa8abb7)),
              ],
            ]),
          ),
        ),
      ),
    );
  }

  Widget _keycap(String label) => Container(
        padding: const EdgeInsets.symmetric(horizontal: 5, vertical: 2),
        decoration: BoxDecoration(
            color: const Color(0x4d454955),
            border: Border.all(color: const Color(0xff484a52), width: 0.5),
            borderRadius: BorderRadius.circular(4)),
        child: Text(label,
            style: const TextStyle(
                fontSize: 10, height: 1.1, color: Color(0xffd0d6e1))),
      );

  IconData _icon(String key) => switch (key) {
        'terminal' => Icons.terminal_rounded,
        'folder' => Icons.folder_open_rounded,
        'file' => Icons.insert_drive_file_outlined,
        'copy' => Icons.content_copy_rounded,
        'globe' => Icons.language_rounded,
        'code' => Icons.data_object_rounded,
        'search' => Icons.search_rounded,
        _ => Icons.bolt_rounded,
      };
}
