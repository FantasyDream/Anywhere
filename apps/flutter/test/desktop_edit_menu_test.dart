import 'dart:async';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:anywhere/infrastructure/desktop_edit_menu.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  const channel = MethodChannel('anywhere/editing');
  final messenger =
      TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger;
  Future<void> command(Object value) async {
    final done = Completer<void>();
    await messenger.handlePlatformMessage(
        channel.name,
        const StandardMethodCodec()
            .encodeMethodCall(MethodCall('perform', value)), (data) {
      try {
        const StandardMethodCodec().decodeEnvelope(data!);
        done.complete();
      } catch (error, stack) {
        done.completeError(error, stack);
      }
    });
    await done.future;
  }

  testWidgets(
      'native edit commands update the focused Flutter model and history',
      (tester) async {
    const pasted = '{"message":"你好 👋"}';
    var clipboard = pasted;
    messenger.setMockMethodCallHandler(SystemChannels.platform, (call) async {
      if (call.method == 'Clipboard.getData') return {'text': clipboard};
      if (call.method == 'Clipboard.hasStrings') return {'value': true};
      if (call.method == 'Clipboard.setData') {
        clipboard = (call.arguments as Map)['text'] as String;
      }
      return null;
    });
    addTearDown(() =>
        messenger.setMockMethodCallHandler(SystemChannels.platform, null));
    final input = TextEditingController(text: 'original');
    final focus = FocusNode();
    var changes = 0;
    await tester.pumpWidget(MaterialApp(
        builder: (_, child) => DesktopEditMenu(child: child!),
        home: Material(
            child: TextField(
                controller: input,
                focusNode: focus,
                onChanged: (_) => changes++))));
    focus.requestFocus();
    await tester.pumpAndSettle();
    await tester.pump(const Duration(milliseconds: 600));
    await command('selectAll');
    expect(input.selection.textInside(input.text), 'original');
    await command('paste');
    await tester.pumpAndSettle();
    expect(input.text, pasted);
    expect(changes, 1);
    await tester.pump(const Duration(milliseconds: 600));
    await command('undo');
    await tester.pumpAndSettle();
    expect(input.text, 'original');
    await command('redo');
    await tester.pumpAndSettle();
    expect(input.text, pasted);
    await command('selectAll');
    await command('copy');
    expect(clipboard, pasted);
    await command('cut');
    await tester.pumpAndSettle();
    expect(input.text, isEmpty);
    expect(clipboard, pasted);
    await command('paste');
    await tester.pumpAndSettle();
    expect(input.text, pasted);
    focus.unfocus();
    await tester.pumpAndSettle();
    await command('paste');
    await command('unsupported');
    await command({'command': 'paste'});
    expect(input.text, pasted);
    await tester.pumpWidget(const SizedBox());
    focus.dispose();
    input.dispose();
    expect(tester.takeException(), isNull);
  }, variant: TargetPlatformVariant.desktop());
}
