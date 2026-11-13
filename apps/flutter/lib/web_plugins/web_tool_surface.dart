import 'dart:async';
import 'dart:convert';

import 'package:flutter/services.dart';
import 'package:flutter_inappwebview/flutter_inappwebview.dart';
import 'package:material_ui/material_ui.dart';

import '../infrastructure/bridge_api.dart' as api;
import '../infrastructure/desktop_appearance.dart';
import '../infrastructure/presentation_metrics.dart';
import '../l10n/app_language.dart';
import '../tools/tool_page_shell.dart';
import '../ui/app_components.dart';
import 'web_bootstrap.dart';
import 'web_document_lifetime.dart';
import 'web_document_policy.dart';
import 'web_document_theme.dart';
import 'web_host_panels.dart';
import 'web_locale.dart';
import 'web_message_transport.dart';
import 'web_resource_pool.dart';
import 'web_view_appearance.dart';
import 'web_view_events.dart';

/// One platform view in the existing Launcher frame. Native callback metadata is
/// forwarded separately from untrusted JS. Hidden views cannot complete requests.
class WebToolSurface extends StatefulWidget {
  const WebToolSurface(
      {super.key,
      required this.invocation,
      required this.onBack,
      required this.visible,
      required this.presentation,
      this.notice});
  final String invocation;
  final VoidCallback onBack;
  final bool visible;
  final int presentation;
  final String? notice;
  @override
  State<WebToolSurface> createState() => _WebToolSurfaceState();
}

enum _WebLoadError { notReady, unavailable, loadFailed, unsupported, stopped }

class _WebToolSurfaceState extends State<WebToolSurface> {
  static final _resources = WebResourcePool();
  static _WebToolSurfaceState? _owner;
  static const _native = MethodChannel('anywhere/web-tools');
  static const _appearance = WebViewAppearance();
  final _lifetime = WebDocumentLifetime(
      close: (handle) => api.closeWebDocument(handle: handle));
  api.WebDocumentDto? get _document => _lifetime.document;
  InAppWebViewController? _web;
  late final DesktopAppearanceController _desktopAppearance;
  _WebLoadError? _error;
  String _errorLabel(AppLocalizations l10n) => switch (_error!) {
        _WebLoadError.notReady => l10n.theToolDidNotBecomeReadyGo,
        _WebLoadError.unavailable => l10n.thisToolIsNoLongerAvailableReturn,
        _WebLoadError.loadFailed => l10n.theToolCouldNotLoadReloadIt,
        _WebLoadError.unsupported => l10n.webToolsAreUnavailableOnThisPlatform,
        _WebLoadError.stopped => l10n.theToolStoppedReloadItOrReturn,
      };
  int _viewKey = 0;
  String _language = 'en';
  bool _documentRegistered = false;
  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    final language = Localizations.localeOf(context).languageCode;
    if (_language == language) return;
    _language = language;
    unawaited(notifyWebLanguage(_web, _language));
  }

  bool _actions = false, _navigated = false;
  bool _loading = true;
  bool _preparingDisplay = false;
  int _actionIndex = 0;
  Timer? _readyTimer;
  String? _followingState;
  @override
  void initState() {
    super.initState();
    _owner = this;
    _desktopAppearance = DesktopAppearance.current
      ..addListener(_desktopAppearanceChanged);
    _native.setMethodCallHandler((call) async {
      if (!mounted || !widget.visible) return;
      if (call.method == 'back') {
        _back();
      } else if (call.method == 'actions') {
        _toggleActions();
      }
    });
    unawaited(_open());
  }

  void _desktopAppearanceChanged() {
    final controller = _web;
    if (controller == null ||
        !mounted ||
        !widget.visible ||
        _document == null) {
      return;
    }
    unawaited(_applyWebAppearance(controller));
  }

  void _back() {
    if (_actions) {
      _toggleActions();
    } else {
      widget.onBack();
    }
  }

  void _hostPanel(bool showing) {
    if (!identical(_owner, this)) return;
    unawaited(notifyWebPresentation(_web, WebPresentationEvent.panel, showing));
    if (showing) {
      unawaited(
          _native.invokeMethod<void>('focusHost').catchError((Object _) {}));
    } else {
      unawaited(
          _native.invokeMethod<void>('focusWeb').catchError((Object _) {}));
    }
  }

  Future<void> _applyWebAppearance(InAppWebViewController controller) async {
    final document = _document;
    await notifyWebLanguage(controller, _language);
    return _appearance.apply(controller,
        isCurrent: () =>
            mounted &&
            identical(_owner, this) &&
            identical(_document, document) &&
            sharesWebView(_web, controller) &&
            widget.visible);
  }

  void _restoreEditorFocus() {
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!mounted ||
          !identical(_owner, this) ||
          !widget.visible ||
          _document == null ||
          _loading ||
          _actions) {
        return;
      }
      _hostPanel(false);
    });
    // A WebKit load callback can arrive while Flutter is idle. Post-frame
    // callbacks do not themselves request a frame; otherwise restoration can
    // remain queued until an unrelated Flutter interaction happens.
    WidgetsBinding.instance.ensureVisualUpdate();
  }

  void _toggleActions() {
    setState(() {
      _actions = !_actions;
      _actionIndex = 0;
    });
    _hostPanel(_actions);
  }

  void _runAction(int index) {
    _hostPanel(false);
    setState(() => _actions = false);
    if (index == 0) {
      unawaited(_open());
    } else {
      widget.onBack();
    }
  }

  KeyEventResult _key(KeyEvent event) {
    if (!widget.visible || event is! KeyDownEvent) {
      return KeyEventResult.ignored;
    }
    if (_actions) {
      if (event.logicalKey == LogicalKeyboardKey.arrowUp ||
          event.logicalKey == LogicalKeyboardKey.arrowDown) {
        setState(() => _actionIndex = 1 - _actionIndex);
        return KeyEventResult.handled;
      }
      if (event.logicalKey == LogicalKeyboardKey.enter) {
        _runAction(_actionIndex);
        return KeyEventResult.handled;
      }
    }
    return KeyEventResult.ignored;
  }

  void _active(bool active) {
    if (!identical(_owner, this)) return;
    unawaited(
        _native.invokeMethod<void>('active', active).catchError((Object _) {}));
  }

  Future<void> _open({bool resume = false}) async {
    final generation = _lifetime.invalidate();
    _resources.cancel(this);
    final previous = resume ? _document : null;
    _readyTimer?.cancel();
    try {
      final document = await _lifetime.openAndAccept(
        generation,
        () => api.openWebDocument(
            invocationId: widget.invocation,
            resumeHandle: resume ? previous?.handle : null),
        interactive: mounted && identical(_owner, this) && widget.visible,
      );
      if (document == null) return;
      _registerDocument();
      setState(() {
        _error = null;
        if (!resume) {
          _loading = true;
          _preparingDisplay = false;
          ++_viewKey;
          _web = null;
          _navigated = false;
        }
      });
      _active(true);
      if (resume) {
        await notifyWebPresentation(
            _web, WebPresentationEvent.visibility, true);
        final controller = _web;
        if (controller != null) await _applyWebAppearance(controller);
      }
      if (resume) _restoreEditorFocus();
      if (!_lifetime.current(generation) || !_current(document)) return;
      // Core also enforces this deadline; this timer only gives the user recovery UI.
      _readyTimer = Timer(const Duration(seconds: 10), () async {
        if (!_lifetime.current(generation) || !_current(document)) return;
        try {
          await api.webResource(handle: document.handle, path: document.entry);
        } catch (_) {
          if (_lifetime.current(generation) && _current(document)) {
            setState(() => _error = _WebLoadError.notReady);
          }
        }
      });
    } catch (_) {
      if (mounted && _lifetime.current(generation) && widget.visible) {
        setState(() => _error = _WebLoadError.unavailable);
      }
    }
  }

  @override
  void didUpdateWidget(WebToolSurface old) {
    super.didUpdateWidget(old);
    if (widget.visible == old.visible) {
      if (widget.visible && widget.presentation != old.presentation) {
        _restoreEditorFocus();
      }
      return;
    }
    if (!widget.visible) {
      _lifetime.invalidate();
      _resources.cancel(this);
      _readyTimer?.cancel();
      _active(false);
      _actions = false;
      unawaited(
          notifyWebPresentation(_web, WebPresentationEvent.visibility, false));
    } else {
      unawaited(_open(resume: true));
    }
  }

  bool _current(api.WebDocumentDto d) =>
      mounted && identical(_owner, this) && widget.visible && _lifetime.owns(d);
  Future<void> _message(
      InAppWebViewController controller,
      WebMessage? message,
      WebUri? origin,
      bool? mainFrame,
      PlatformJavaScriptReplyProxy reply) async {
    final document = _document;
    final frame = message?.data;
    if (document == null ||
        !sharesWebView(_web, controller) ||
        !_current(document) ||
        mainFrame != true ||
        origin?.toString() != document.origin ||
        frame is! String ||
        frame.length > 65536) {
      return;
    }
    try {
      final response = await api.dispatchWebFrame(
          handle: document.handle,
          origin: origin!.toString(),
          mainFrame: true,
          frame: frame);
      if (!_current(document)) return;
      await reply.postMessage(webReplyMessage(response));
      if (!_current(document)) return;
      final acknowledgement = webAcknowledgement(frame, response);
      if (acknowledgement == WebAcknowledgement.stateSubscribed &&
          _followingState != document.handle) {
        _followingState = document.handle;
        unawaited(_followState(controller, document));
      }
      if (acknowledgement == WebAcknowledgement.ready) {
        _readyTimer?.cancel();
        if (_loading && !_preparingDisplay) {
          _preparingDisplay = true;
          await _applyWebAppearance(controller);
          if (_current(document) && sharesWebView(_web, controller)) {
            // Keep the live WebKit surface. Taking a snapshot here can leave
            // transparent remote layers blank until a later Host interaction.
            setState(() => _loading = false);
            _restoreEditorFocus();
          }
        }
      }
    } catch (_) {/* No unqualified reply or internal error reaches the page. */}
  }

  Future<void> _followState(
      InAppWebViewController controller, api.WebDocumentDto document) async {
    var version = BigInt.zero;
    try {
      while (_current(document) && sharesWebView(_web, controller)) {
        final frame =
            await api.waitWebState(handle: document.handle, after: version);
        if (!_current(document) || !sharesWebView(_web, controller)) return;
        if (frame == null) continue;
        final event = jsonDecode(frame) as Map<String, dynamic>;
        version = BigInt.from(event['version'] as num);
        await controller.evaluateJavascript(
            source:
                "window.dispatchEvent(new CustomEvent('anywhere:state', {detail: ${jsonEncode(frame)}}));");
      }
    } catch (_) {/* Disposal/revocation ends the subscription. */}
  }

  Future<void> _attach(InAppWebViewController controller,
      api.WebDocumentDto document, int viewKey) async {
    if (!_current(document) || viewKey != _viewKey) return;
    _web = controller;
    try {
      await controller.addWebMessageListener(WebMessageListener(
          jsObjectName: 'anywhereNative',
          allowedOriginRules: {'anywhere-plugin:'},
          onPostMessage: (message, origin, main, reply) =>
              _message(controller, message, origin, main, reply)));
      if (!_current(document) || !sharesWebView(_web, controller)) return;
      // Listener bootstrap must precede the transport at document start.
      await controller.addUserScript(
          userScript: UserScript(
              source: webLocaleScript(_language) +
                  webBootstrap +
                  webCanvasScript(
                      transparent: DesktopAppearance
                              .current.snapshot.usesSystemSurface &&
                          !DesktopAppearance
                              .current.snapshot.reduceTransparency),
              injectionTime: UserScriptInjectionTime.AT_DOCUMENT_START,
              forMainFrameOnly: true));
      if (_current(document) && sharesWebView(_web, controller)) {
        await controller.loadUrl(
            urlRequest: URLRequest(
                url: WebUri(
                    WebDocumentPolicy(document.origin, document.entry).url)));
      }
    } on PlatformException {
      if (_current(document) && sharesWebView(_web, controller)) {
        setState(() => _error = _WebLoadError.loadFailed);
      }
    } on MissingPluginException {
      if (_current(document) && sharesWebView(_web, controller)) {
        setState(() => _error = _WebLoadError.unsupported);
      }
    }
  }

  Future<CustomSchemeResponse?> _resource(
      InAppWebViewController controller, WebResourceRequest request) async {
    final document = _document;
    if (document == null ||
        !sharesWebView(_web, controller) ||
        !_current(document)) {
      return null;
    }
    final policy = WebDocumentPolicy(document.origin, document.entry);
    final path = policy.resource(request.url.toString());
    final type = path == null ? null : policy.contentType(path);
    if (path == null || type == null) return null;
    if (!await _resources.acquire(this)) return null;
    try {
      if (!_current(document)) return null;
      var bytes = await api.webResource(handle: document.handle, path: path);
      if (!_current(document)) return null;
      if (type == 'text/html') {
        bytes = policy.protectHtml(bytes);
      }
      return CustomSchemeResponse(
          data: bytes, contentType: type, contentEncoding: 'utf-8');
    } catch (_) {
      return null;
    } finally {
      _resources.release();
    }
  }

  @override
  void dispose() {
    _unregisterDocument();
    _desktopAppearance.removeListener(_desktopAppearanceChanged);
    _lifetime.invalidate();
    _resources.cancel(this);
    _readyTimer?.cancel();
    // Flutter may initialize the replacement before finalizing the old element.
    // The departing page must not clear the new page's native shortcut channel.
    if (identical(_owner, this)) {
      _active(false);
      _owner = null;
      _native.setMethodCallHandler(null);
    }
    _web = null;
    unawaited(_lifetime.dispose());
    super.dispose();
  }

  void _registerDocument() {
    if (_documentRegistered) return;
    _documentRegistered = true;
    PresentationDocumentStats.registerWebDocument();
  }

  void _unregisterDocument() {
    if (!_documentRegistered) return;
    _documentRegistered = false;
    PresentationDocumentStats.releaseWebDocument();
  }

  @override
  Widget build(BuildContext context) {
    final document = _document;
    final viewKey = _viewKey;
    return ToolPageShell(
        title: document?.title ?? context.l10n.openingTool,
        onBack: _back,
        onKeyEvent: _key,
        visible: widget.visible,
        presentation: widget.presentation,
        notice: widget.notice,
        trailing: Row(mainAxisSize: MainAxisSize.min, children: [
          if (document != null && _loading && _error == null)
            Padding(
                padding: const EdgeInsets.only(right: 8),
                child: SizedBox(
                    width: 14,
                    height: 14,
                    child: CircularProgressIndicator(
                        strokeWidth: 1.5,
                        semanticsLabel: context.l10n.loadingTool))),
          AppMoreAction(onPressed: widget.visible ? _toggleActions : null),
        ]),
        child: Stack(fit: StackFit.expand, children: [
          if (_error != null)
            Center(
                child: Padding(
                    padding: const EdgeInsets.all(24),
                    child: Text(_errorLabel(context.l10n))))
          else if (document == null)
            const Center(child: CircularProgressIndicator(strokeWidth: 2))
          else
            InAppWebView(
                key: ValueKey(_viewKey),
                initialSettings: InAppWebViewSettings(
                    transparentBackground: true,
                    incognito: true,
                    cacheEnabled: false,
                    resourceCustomSchemes: ['anywhere-plugin'],
                    useShouldOverrideUrlLoading: true,
                    javaScriptCanOpenWindowsAutomatically: false,
                    allowsLinkPreview: false,
                    disableContextMenu: true,
                    contentBlockers: [
                      ContentBlocker(
                          trigger:
                              ContentBlockerTrigger(urlFilter: '^https?://'),
                          action: ContentBlockerAction(
                              type: ContentBlockerActionType.BLOCK))
                    ]),
                onWebViewCreated: (controller) =>
                    _attach(controller, document, viewKey),
                onLoadStop: (controller, _) async {
                  // Configure the loaded document directly. Appearance must
                  // never wait for a future Flutter frame or editor focus.
                  if (!sharesWebView(_web, controller) || !widget.visible) {
                    return;
                  }
                  await _applyWebAppearance(controller);
                  if (!_loading && sharesWebView(_web, controller)) {
                    _restoreEditorFocus();
                  }
                },
                onLoadResourceWithCustomScheme: _resource,
                shouldOverrideUrlLoading: (_, navigation) async {
                  final initial = !_navigated &&
                      navigation.isForMainFrame == true &&
                      navigation.request.url?.toString() ==
                          WebDocumentPolicy(document.origin, document.entry)
                              .url;
                  if (initial) _navigated = true;
                  return initial
                      ? NavigationActionPolicy.ALLOW
                      : NavigationActionPolicy.CANCEL;
                },
                onJsAlert: (_, __) async => JsAlertResponse(
                    handledByClient: true,
                    action: JsAlertResponseAction.CONFIRM),
                onJsConfirm: (_, __) async => JsConfirmResponse(
                    handledByClient: true,
                    action: JsConfirmResponseAction.CANCEL),
                onJsPrompt: (_, __) async => JsPromptResponse(
                    handledByClient: true,
                    action: JsPromptResponseAction.CANCEL),
                onDownloadStartRequest: (_, __) {},
                onCreateWindow: (_, __) async => false,
                onPermissionRequest: (_, request) async => PermissionResponse(
                    resources: request.resources,
                    action: PermissionResponseAction.DENY),
                onWebContentProcessDidTerminate: (controller) {
                  if (mounted && sharesWebView(_web, controller)) {
                    _lifetime.invalidate();
                    _readyTimer?.cancel();
                    _web = null;
                    _active(false);
                    unawaited(_lifetime.closeCurrent());
                    setState(() => _error = _WebLoadError.stopped);
                  }
                }),
          if (_actions)
            WebActionsPanel(selected: _actionIndex, onSelect: _runAction),
        ]));
  }
}
