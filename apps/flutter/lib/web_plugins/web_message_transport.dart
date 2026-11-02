import 'dart:convert';
import 'package:flutter_inappwebview/flutter_inappwebview.dart';

const webFrameBytes = 64 * 1024;

enum WebAcknowledgement { ready, stateSubscribed }

/// Presentation changes require a matching accepted broker reply. Ordinary RPC
/// replies are forwarded without decoding their potentially large result here.
WebAcknowledgement? webAcknowledgement(String request, String response) {
  try {
    final sent = jsonDecode(request);
    if (sent is! Map || sent['requestId'] is! String) return null;
    final acknowledgement = switch (sent['method']) {
      'lifecycle.ready' => WebAcknowledgement.ready,
      'state.subscribe' => WebAcknowledgement.stateSubscribed,
      _ => null,
    };
    if (acknowledgement == null) return null;
    final received = jsonDecode(response);
    if (received is! Map ||
        received['requestId'] != sent['requestId'] ||
        received.containsKey('error') ||
        received['result'] is! Map) {
      return null;
    }
    final result = received['result'] as Map;
    if (acknowledgement == WebAcknowledgement.ready &&
        (result.length != 1 ||
            !result.containsKey('value') ||
            result['value'] != null)) {
      return null;
    }
    return acknowledgement;
  } on FormatException {
    return null;
  }
}

/// The pinned native string reply embeds its data in a JavaScript literal without
/// escaping backslashes. Binary replies use numeric bytes and preserve the exact
/// UTF-8 frame; the document transport decodes it before delivering it to the SDK.
WebMessage webReplyMessage(String frame) {
  if (frame.length > webFrameBytes) {
    throw ArgumentError('Web reply exceeds the frame limit');
  }
  final bytes = utf8.encode(frame);
  if (bytes.length > webFrameBytes) {
    throw ArgumentError('Web reply exceeds the frame limit');
  }
  return WebMessage(data: bytes, type: WebMessageType.ARRAY_BUFFER);
}
