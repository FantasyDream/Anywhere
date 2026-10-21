import 'dart:async';
import 'dart:convert';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_inappwebview/flutter_inappwebview.dart';
import 'package:flutter_inappwebview_macos/flutter_inappwebview_macos.dart';
import 'probe_bridge.dart';

const _host = MethodChannel('anywhere/web-plugin-probe');

void main() {
  WidgetsFlutterBinding.ensureInitialized();
  // This probe deliberately uses the same manual embedding as the production app.
  MacOSInAppWebViewPlatform.registerWith();
  runApp(MaterialApp(
    debugShowCheckedModeBanner: false,
    theme: ThemeData.dark(useMaterial3: true),
    home: const ProbePage(),
  ));
}

class ProbePage extends StatefulWidget {
  const ProbePage({super.key});
  @override
  State<ProbePage> createState() => _ProbePageState();
}

class _ProbePageState extends State<ProbePage> {
  ProbeBridge _bridge = ProbeBridge(1);
  Timer? _retention;
  bool _present = true;
  bool _visible = true;
  bool _actions = false;
  String _status = 'Opening local WebView…';

  @override
  void initState() {
    super.initState();
    _host.setMethodCallHandler((call) async {
      if (!mounted) return;
      if (call.method == 'visibility') {
        _setVisible(call.arguments == true);
      } else if (call.method == 'actions') {
        setState(() => _actions = !_actions);
      } else if (call.method == 'back') {
        if (_actions) {
          setState(() => _actions = false);
        } else {
          _close();
        }
      }
    });
  }

  void _setVisible(bool visible) {
    _visible = visible;
    _retention?.cancel();
    _bridge.interactive = visible;
    if (visible && !_present) {
      _reload();
    } else if (!visible) {
      _retention = Timer(const Duration(seconds: 15), _close);
    }
  }

  void _close() {
    _retention?.cancel();
    _bridge.dispose();
    if (mounted) {
      setState(() {
        _present = false;
        _actions = false;
      });
    }
  }

  void _reload() {
    _retention?.cancel();
    final next = _bridge.epoch + 1;
    _bridge.dispose();
    setState(() {
      _bridge = ProbeBridge(next)..interactive = _visible;
      _present = true;
      _actions = false;
      _status = 'Opening document $next…';
    });
  }

  Future<void> _report(String value) async {
    await _host.invokeMethod<void>('report', value);
  }

  Widget _surface(ProbeBridge binding) => InAppWebView(
        key: ValueKey(binding.epoch),
        initialSettings: InAppWebViewSettings(
          transparentBackground: true,
          incognito: true,
          cacheEnabled: false,
          resourceCustomSchemes: ['anywhere-plugin'],
          useShouldOverrideUrlLoading: true,
          javaScriptCanOpenWindowsAutomatically: false,
          allowsLinkPreview: false,
          contentBlockers: [
            ContentBlocker(
                trigger: ContentBlockerTrigger(urlFilter: '^https?://'),
                action:
                    ContentBlockerAction(type: ContentBlockerActionType.BLOCK))
          ],
        ),
        onWebViewCreated: (controller) async {
          try {
            await controller.addWebMessageListener(WebMessageListener(
              jsObjectName: 'anywhereProbe',
              // Stable upstream accepts scheme-only rules for custom protocols.
              // ProbeBridge separately verifies the exact native-reported origin.
              allowedOriginRules: {'anywhere-plugin:'},
              onPostMessage: (message, origin, mainFrame, reply) async {
                final data = message?.data;
                final response = binding.handle(data is String ? data : '',
                    sourceOrigin: origin, isMainFrame: mainFrame);
                if (!mounted || !identical(binding, _bridge) || !_present) {
                  return;
                }
                setState(() => _status =
                    'Native origin: $origin · main frame: $mainFrame · document ${binding.epoch}');
                await reply.postMessage(WebMessage(data: jsonEncode(response)));
                final result = response['result'];
                if (result is Map &&
                    result['text'] is String &&
                    (result['text'] as String).startsWith('CHECKS:')) {
                  await _report(result['text'] as String);
                }
              },
            ));
            if (mounted && identical(binding, _bridge) && _present) {
              await controller.loadUrl(
                  urlRequest: URLRequest(
                      url: WebUri('anywhere-plugin://probe.local/index.html')));
            }
          } on PlatformException catch (error) {
            if (mounted && identical(binding, _bridge)) {
              setState(() => _status = 'Bridge setup failed: ${error.code}');
              await _report('FAIL:bridge_setup:${error.code}');
            }
          }
        },
        onLoadResourceWithCustomScheme: (controller, request) async {
          if (!identical(binding, _bridge) || !_present) return null;
          final uri = request.url;
          if (uri.scheme != 'anywhere-plugin' ||
              uri.host != 'probe.local' ||
              uri.hasPort ||
              uri.userInfo.isNotEmpty ||
              uri.hasQuery) {
            return null;
          }
          final (asset, mime) = switch (uri.path) {
            '/index.html' => ('assets/index.html', 'text/html'),
            '/probe.js' => ('assets/probe.js', 'application/javascript'),
            _ => ('', ''),
          };
          if (asset.isEmpty) return null;
          final bytes = await rootBundle.load(asset);
          if (!mounted || !identical(binding, _bridge) || !_present) {
            return null;
          }
          return CustomSchemeResponse(
              data: Uint8List.sublistView(bytes),
              contentType: mime,
              contentEncoding: 'utf-8');
        },
        shouldOverrideUrlLoading: (_, action) async => binding.permitNavigation(
                action.request.url,
                isMainFrame: action.isForMainFrame)
            ? NavigationActionPolicy.ALLOW
            : NavigationActionPolicy.CANCEL,
        onCreateWindow: (controller, action) async => false,
        onPermissionRequest: (_, request) async => PermissionResponse(
            resources: request.resources,
            action: PermissionResponseAction.DENY),
        onReceivedError: (_, request, error) {
          if (mounted && identical(binding, _bridge)) {
            setState(() => _status = 'Load failed: ${error.type}');
          }
        },
      );

  @override
  Widget build(BuildContext context) => Scaffold(
        backgroundColor: const Color(0xDD28282C),
        body: Padding(
          padding: const EdgeInsets.all(18),
          child:
              Column(crossAxisAlignment: CrossAxisAlignment.stretch, children: [
            Row(children: [
              const Expanded(child: Text('Web Plugin · P0 developer probe')),
              TextButton(onPressed: _reload, child: const Text('Reload')),
              TextButton(onPressed: _close, child: const Text('Dispose')),
              TextButton(
                  onPressed: () => setState(() => _actions = !_actions),
                  child: const Text('Actions · ⌘K')),
            ]),
            const Divider(),
            Expanded(
                child: Stack(children: [
              if (_present)
                Offstage(offstage: _actions, child: _surface(_bridge))
              else
                const Center(
                    child: Text(
                        'WebView disposed. Reload to create a fresh document.')),
              if (_actions)
                Center(
                    child: Card(
                        child: Padding(
                  padding: const EdgeInsets.all(20),
                  child: Column(mainAxisSize: MainAxisSize.min, children: [
                    const Text('Host Actions · Esc to return'),
                    TextButton(
                        onPressed: _reload,
                        child: const Text('Reload document')),
                    TextButton(
                        onPressed: _close,
                        child: const Text('Dispose WebView')),
                  ]),
                ))),
            ])),
            const Divider(),
            Text(_status, maxLines: 2),
          ]),
        ),
      );

  @override
  void dispose() {
    _retention?.cancel();
    _bridge.dispose();
    _host.setMethodCallHandler(null);
    super.dispose();
  }
}
