import 'package:anywhere/src/rust/api.dart';
import 'package:anywhere/web_plugins/web_document_lifetime.dart';
import 'package:flutter_test/flutter_test.dart';

WebDocumentDto document(String id) => WebDocumentDto(
    handle: id,
    origin: 'anywhere-plugin://$id.plugin.local',
    entry: 'index.html',
    title: 'Test');
void main() {
  test('late open after hide is closed without adopting its binding', () async {
    final closed = <String>[];
    final lifetime = WebDocumentLifetime(close: (id) async => closed.add(id));
    final generation = lifetime.invalidate();
    lifetime.invalidate();
    expect(
        await lifetime.accept(document('late'), generation, interactive: false),
        isFalse);
    expect(lifetime.document, isNull);
    expect(closed, ['late']);
  });
  test(
      'obsolete opens cannot overwrite a replacement; dispose closes only current',
      () async {
    final closed = <String>[];
    final lifetime = WebDocumentLifetime(close: (id) async => closed.add(id));
    final old = lifetime.invalidate(), next = lifetime.invalidate();
    final current = document('new');
    expect(await lifetime.accept(current, next, interactive: true), isTrue);
    expect(await lifetime.accept(document('old'), old, interactive: true),
        isFalse);
    expect(lifetime.owns(current), isTrue);
    await lifetime.dispose();
    expect(lifetime.current(next), isFalse);
    expect(lifetime.document, isNull);
    expect(closed, ['old', 'new']);
    expect(
        await lifetime.accept(document('after-dispose'), next,
            interactive: true),
        isFalse);
    expect(closed.last, 'after-dispose');
  });
  test('revoked binding cleanup failure does not escape lifecycle cleanup',
      () async {
    final lifetime =
        WebDocumentLifetime(close: (_) async => throw StateError('revoked'));
    final ticket = lifetime.invalidate();
    await lifetime.accept(document('one'), ticket, interactive: true);
    await lifetime.dispose();
    expect(lifetime.document, isNull);
  });
}
