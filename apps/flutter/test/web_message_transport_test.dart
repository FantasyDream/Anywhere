import 'dart:convert';
import 'dart:typed_data';

import 'package:anywhere/web_plugins/web_message_transport.dart';
import 'package:flutter_inappwebview/flutter_inappwebview.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  test('loading ends only on the matching broker ready acknowledgement', () {
    const request = '{"requestId":"r1","method":"lifecycle.ready"}';
    expect(
        webReadyAcknowledged(
            request, '{"requestId":"r1","result":{"value":null}}'),
        isTrue);
    for (final response in [
      '{"requestId":"r2","result":{"value":null}}',
      '{"requestId":"r1","error":{"code":"INACTIVE"}}',
      '{"requestId":"r1","result":{"value":null},"error":null}',
      '{"requestId":"r1","result":{"transfer":{}}}',
      '{"requestId":"r1","result":{}}',
      'invalid',
    ]) {
      expect(webReadyAcknowledged(request, response), isFalse);
    }
    expect(
        webReadyAcknowledged('{"requestId":"r1","method":"storage.get"}',
            '{"requestId":"r1","result":{"value":null}}'),
        isFalse);
  });
  test('native reply uses binary data and preserves escaped JSON and Unicode',
      () {
    final input = r'''{"path":"C:\\work\\project","quote":"\"","line":"\n"}'''
        '\n中文🚀\u2028\u2029';
    final frame = jsonEncode({
      'requestId': 'test',
      'result': {
        'value': {
          'input': {'kind': 'text', 'text': input}
        }
      }
    });
    final message = webReplyMessage(frame);
    expect(message.type, WebMessageType.ARRAY_BUFFER);
    expect(message.data, isA<Uint8List>());
    final roundTrip = utf8.decode(message.data as Uint8List);
    expect(roundTrip, frame);
    expect(jsonDecode(roundTrip)['result']['value']['input']['text'], input);
    expect(
        message.toMap()['type'], WebMessageType.ARRAY_BUFFER.toNativeValue());
  });

  test('reply budget applies to UTF-8 bytes, not only string length', () {
    expect((webReplyMessage('a' * webFrameBytes).data as Uint8List).length,
        webFrameBytes);
    expect(
        () => webReplyMessage('a' * (webFrameBytes + 1)), throwsArgumentError);
    expect(() => webReplyMessage('中' * (webFrameBytes ~/ 3 + 1)),
        throwsArgumentError);
  });
}
