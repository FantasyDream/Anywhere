import 'dart:async';

import 'package:anywhere/infrastructure/bridge_api.dart';
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
  test('replacing a binding closes the old document once', () async {
    final closed = <String>[];
    final lifetime = WebDocumentLifetime(close: (id) async => closed.add(id));
    final ticket = lifetime.invalidate();
    expect(await lifetime.accept(document('old'), ticket, interactive: true),
        isTrue);
    lifetime.invalidate();
    await lifetime.closeCurrent();
    expect(lifetime.document, isNull);
    expect(closed, ['old']);
    await lifetime.dispose();
    expect(closed, ['old']);
  });
  test('replacement binds before it closes the prior document', () async {
    final closed = <String>[];
    final lifetime = WebDocumentLifetime(close: (id) async => closed.add(id));
    final ticket = lifetime.invalidate();
    await lifetime.accept(document('old'), ticket, interactive: true);
    final replacement = await lifetime.openAndAccept(ticket, () async {
      expect(lifetime.document?.handle, 'old');
      return document('new');
    }, interactive: true);
    expect(replacement?.handle, 'new');
    expect(closed, ['old']);
    await lifetime.dispose();
  });
  test('late replacement is closed when its generation became stale', () async {
    final openStarted = Completer<void>();
    final allowOpen = Completer<void>();
    final closed = <String>[];
    final lifetime = WebDocumentLifetime(close: (id) async => closed.add(id));
    final ticket = lifetime.invalidate();
    await lifetime.accept(document('old'), ticket, interactive: true);
    final replacement = lifetime.openAndAccept(ticket, () async {
      openStarted.complete();
      await allowOpen.future;
      return document('new');
    }, interactive: true);
    await openStarted.future;
    lifetime.invalidate();
    allowOpen.complete();
    expect(await replacement, isNull);
    expect(lifetime.document?.handle, 'old');
    expect(closed, ['new']);
    await lifetime.dispose();
  });
  test('accepting a new runtime closes a replaced binding', () async {
    final closed = <String>[];
    final lifetime = WebDocumentLifetime(close: (id) async => closed.add(id));
    final first = lifetime.invalidate();
    expect(await lifetime.accept(document('old'), first, interactive: true),
        isTrue);
    final second = lifetime.invalidate();
    expect(await lifetime.accept(document('new'), second, interactive: true),
        isTrue);
    expect(lifetime.document?.handle, 'new');
    expect(closed, ['old']);
    await lifetime.dispose();
    expect(closed, ['old', 'new']);
  });
}
