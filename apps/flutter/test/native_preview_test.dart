import 'dart:async';

import 'package:anywhere/infrastructure/desktop_environment.dart';
import 'package:anywhere/infrastructure/native_preview.dart';
import 'package:anywhere/src/rust/api.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  final messenger =
      TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger;
  final calls = <MethodCall>[], released = <String>[];
  var failures = 0;
  NativePreviewDto resource(String id) =>
      NativePreviewDto(id: id, paths: ['/test/$id.txt'], title: id);
  NativePreviewController controller() => NativePreviewController(
      onMove: (_) {},
      onActivate: () {},
      onSearch: () {},
      onFailure: () => failures++,
      release: (id) async {
        released.add(id);
      });
  setUp(() {
    calls.clear();
    released.clear();
    failures = 0;
    DesktopEnvironment.configure(
        const DesktopEnvironment(platform: TargetPlatform.macOS));
    messenger.setMockMethodCallHandler(NativePreviewController.channel,
        (call) async {
      calls.add(call);
      return call.method == 'capabilities' ? {'supported': true} : null;
    });
  });
  tearDown(DesktopEnvironment.reset);
  Future<void> event(String method, String id,
      {bool restoreFocus = false}) async {
    final done = Completer<void>();
    await messenger.handlePlatformMessage(
        NativePreviewController.channel.name,
        const StandardMethodCodec().encodeMethodCall(
            MethodCall(method, {'id': id, 'restoreFocus': restoreFocus})),
        (_) => done.complete());
    await done.future;
  }

  testWidgets(
      'native events are bound to the current resource and close releases it',
      (tester) async {
    final retained = <String>[], moved = <int>[];
    var focused = 0;
    final c = NativePreviewController(
        onMove: moved.add,
        onActivate: () {},
        onSearch: () => focused++,
        onFailure: () => failures++,
        release: (id) async => released.add(id),
        retain: (id) async => retained.add(id));
    c.sync('one', () async => resource('one'));
    c.toggle();
    await tester.pumpAndSettle();
    await event('shown', 'stale');
    await event('closed', 'stale');
    expect(c.isOpen, true);
    expect(retained, isEmpty);
    await event('shown', 'one');
    await event('next', 'one');
    await event('previous', 'one');
    expect(retained, ['one']);
    expect(moved, [1, -1]);
    await event('closed', 'one', restoreFocus: true);
    await tester.pumpAndSettle();
    expect(c.isOpen, false);
    expect(released, ['one']);
    expect(focused, 1);
    expect(c.browsing, true);
    c.toggle();
    await tester.pumpAndSettle();
    await event('search', 'one');
    await tester.pumpAndSettle();
    expect(c.isOpen, false);
    expect(c.browsing, false);
    expect(focused, 2);
    c.dispose();
  });
  testWidgets(
      'selection is lazy, updates only the latest target and closing releases resources',
      (tester) async {
    final c = controller();
    var prepared = 0;
    c.sync('one', () async {
      prepared++;
      return resource('one');
    });
    await tester.pumpAndSettle();
    expect(prepared, 0);
    c.toggle();
    await tester.pumpAndSettle();
    expect(prepared, 1);
    expect(calls.last.arguments['id'], 'one');
    c.sync('two', () async => resource('two'));
    c.sync('three', () async => resource('three'));
    await tester.pump(const Duration(milliseconds: 80));
    await tester.pumpAndSettle();
    expect(calls.last.arguments['id'], 'three');
    expect(released, ['one']);
    c.close();
    await tester.pumpAndSettle();
    expect(released, ['one', 'three']);
    expect(c.isOpen, false);
    c.dispose();
  });
  testWidgets('native disappearance lets one Space press reopen the selection',
      (tester) async {
    final c = controller();
    var issued = 0;
    c.sync('file', () async => resource('preview-${++issued}'));
    c.toggle();
    await tester.pumpAndSettle();
    await event('closed', 'preview-1');
    await tester.pumpAndSettle();
    expect(c.isOpen, false);
    expect(released, ['preview-1']);
    expect(
        c.handleKey(
            KeyDownEvent(
                physicalKey: PhysicalKeyboardKey.space,
                logicalKey: LogicalKeyboardKey.space,
                timeStamp: Duration.zero),
            query: 'settings.xml'),
        true);
    await tester.pumpAndSettle();
    expect(c.isOpen, true);
    expect(
        calls
            .where((call) => call.method == 'show')
            .map((call) => call.arguments['id']),
        ['preview-1', 'preview-2']);
    c.dispose();
    await tester.pumpAndSettle();
  });
  testWidgets('late preparation cannot reopen a closed or replacement preview',
      (tester) async {
    final c = controller();
    final pending = Completer<NativePreviewDto>();
    c.sync('old', () => pending.future);
    c.toggle();
    await tester.pump();
    c.close();
    final next = controller();
    next.sync('new', () async => resource('new'));
    next.toggle();
    await tester.pumpAndSettle();
    pending.complete(resource('old'));
    await tester.pumpAndSettle();
    expect(
        calls
            .where((call) => call.method == 'show')
            .map((call) => call.arguments['id']),
        ['new']);
    expect(released, contains('old'));
    expect(next.isOpen, true);
    c.dispose();
    next.dispose();
    await tester.pumpAndSettle();
  });
  testWidgets(
      'failed replacement and unavailable host never leak resources or leave an open mode',
      (tester) async {
    final c = controller();
    c.sync('one', () async => resource('one'));
    c.toggle();
    await tester.pumpAndSettle();
    messenger.setMockMethodCallHandler(NativePreviewController.channel,
        (call) async {
      if (call.method == 'show') throw PlatformException(code: 'failed');
      return call.method == 'capabilities' ? {'supported': true} : null;
    });
    c.sync('two', () async => resource('two'));
    await tester.pump(const Duration(milliseconds: 80));
    await tester.pumpAndSettle();
    expect(c.isOpen, false);
    expect(released.toSet(), {'one', 'two'});
    expect(failures, 1);
    c.dispose();
    messenger.setMockMethodCallHandler(
        NativePreviewController.channel, (_) async => {'supported': false});
    final unsupported = controller();
    var loaded = false;
    unsupported.sync('one', () async {
      loaded = true;
      return resource('never');
    });
    unsupported.toggle();
    await tester.pumpAndSettle();
    expect(loaded, false);
    expect(unsupported.isOpen, false);
    unsupported.dispose();
  });
  testWidgets(
      'old panel cannot activate a changed selection, even after returning',
      (tester) async {
    var activated = 0;
    final c = NativePreviewController(
        onMove: (_) {},
        onActivate: () => activated++,
        onSearch: () {},
        onFailure: () => failures++,
        release: (id) async => released.add(id),
        retain: (_) async {});
    c.sync('one', () async => resource('one'));
    c.toggle();
    await tester.pumpAndSettle();
    c.sync('two', () async => resource('two'));
    await event('activate', 'one');
    expect(activated, 0);
    expect(c.isOpen, true);
    c.sync('one', () async => resource('one-new'));
    await event('activate', 'one');
    expect(activated, 0);
    await tester.pump(const Duration(milliseconds: 80));
    await tester.pumpAndSettle();
    await event('activate', 'one');
    expect(activated, 0);
    await event('activate', 'one-new');
    expect(activated, 1);
    expect(c.isOpen, false);
    c.dispose();
    await tester.pumpAndSettle();
  });

  testWidgets('slow preparation admits only the latest debounced replacement',
      (tester) async {
    final first = Completer<NativePreviewDto>();
    final prepared = <String>[];
    final c = controller();
    c.sync('one', () {
      prepared.add('one');
      return first.future;
    });
    c.toggle();
    await tester.pump();
    for (final id in ['two', 'three', 'four']) {
      c.sync(id, () async {
        prepared.add(id);
        return resource(id);
      });
      await tester.pump(const Duration(milliseconds: 81));
    }
    expect(prepared, ['one']);
    first.complete(resource('one'));
    await tester.pumpAndSettle();
    expect(prepared, ['one', 'four']);
    expect(released, ['one']);
    expect(calls.where((call) => call.method == 'show').single.arguments['id'],
        'four');
    c.dispose();
    await tester.pumpAndSettle();
    expect(released, ['one', 'four']);
  });

  testWidgets(
      'closing discards queued work and reopening waits for old preparation',
      (tester) async {
    final first = Completer<NativePreviewDto>();
    final prepared = <String>[];
    final c = controller();
    c.sync('one', () => first.future);
    c.toggle();
    await tester.pump();
    c.sync('two', () async {
      prepared.add('two');
      return resource('two');
    });
    await tester.pump(const Duration(milliseconds: 81));
    c.close();
    c.sync('three', () async {
      prepared.add('three');
      return resource('three');
    });
    c.toggle();
    await tester.pump();
    expect(prepared, isEmpty);
    first.complete(resource('one'));
    await tester.pumpAndSettle();
    expect(prepared, ['three']);
    expect(released, ['one']);
    c.dispose();
    await tester.pumpAndSettle();
  });

  testWidgets(
      'Space belongs to text and IME until results are explicitly browsed',
      (tester) async {
    final c = controller();
    c.sync('one', () async => resource('one'));
    final key = KeyDownEvent(
        physicalKey: PhysicalKeyboardKey.space,
        logicalKey: LogicalKeyboardKey.space,
        timeStamp: Duration.zero);
    expect(c.handleKey(key, query: 'two words'), false);
    c.browsing = true;
    expect(c.handleKey(key, query: 'zhong', composing: true), false);
    expect(c.handleKey(key, query: 'two words'), true);
    await tester.pumpAndSettle();
    expect(c.isOpen, true);
    expect(c.handleKey(key, query: 'two words'), true);
    await tester.pumpAndSettle();
    expect(c.isOpen, false);
    expect(c.handleKey(key, query: 'two words'), true);
    await tester.pumpAndSettle();
    expect(c.isOpen, true);
    c.edited();
    expect(c.browsing, false);
    c.dispose();
  });
}
