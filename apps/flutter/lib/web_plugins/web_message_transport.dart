import 'dart:convert';
import 'dart:typed_data';
import 'package:flutter_inappwebview/flutter_inappwebview.dart';

const webFrameBytes = 64 * 1024;

/// Presentation only: dismiss loading after the Rust broker accepted ready.
/// A page's request alone never proves that its document is authorized / ready.
bool webReadyAcknowledged(String request, String response) {
  try {
    final sent = jsonDecode(request);
    final received = jsonDecode(response);
    return sent is Map &&
        sent['method'] == 'lifecycle.ready' &&
        sent['requestId'] is String &&
        received is Map &&
        received['requestId'] == sent['requestId'] &&
        !received.containsKey('error') &&
        received['result'] is Map &&
        (received['result'] as Map).length == 1 &&
        (received['result'] as Map).containsKey('value') &&
        received['result']['value'] == null;
  } on FormatException {
    return false;
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
  return WebMessage(
      data: Uint8List.fromList(bytes), type: WebMessageType.ARRAY_BUFFER);
}

/// Start the native state wait only after a qualified SDK subscription was accepted.
bool webStateSubscribed(String request, String response) {
  try {
    final sent = jsonDecode(request), received = jsonDecode(response);
    return sent is Map &&
        sent['method'] == 'state.subscribe' &&
        sent['requestId'] is String &&
        received is Map &&
        received['requestId'] == sent['requestId'] &&
        !received.containsKey('error') &&
        received['result'] is Map;
  } on FormatException {
    return false;
  }
}
