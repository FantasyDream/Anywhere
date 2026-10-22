import 'package:anywhere/ui/app_style.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  testWidgets('bare Tab does not traverse fields, buttons or dialog controls',
      (tester) async {
    final input = FocusNode();
    final dialogInput = FocusNode();
    addTearDown(input.dispose);
    addTearDown(dialogInput.dispose);
    await tester.pumpWidget(MaterialApp(
        theme: AppStyle.theme,
        builder: AppStyle.windowBuilder,
        home: Scaffold(
            body: Builder(
                builder: (context) => Column(children: [
                      TextField(focusNode: input, autofocus: true),
                      TextField(),
                      TextButton(
                          onPressed: () => showDialog<void>(
                              context: context,
                              builder: (_) => AlertDialog(
                                      content: Column(
                                          mainAxisSize: MainAxisSize.min,
                                          children: [
                                        TextField(
                                            focusNode: dialogInput,
                                            autofocus: true),
                                        const TextField(),
                                      ]))),
                          child: const Text('Open dialog')),
                    ])))));
    await tester.pumpAndSettle();
    Future<void> tabs() async {
      await tester.sendKeyEvent(LogicalKeyboardKey.tab);
      await tester.pumpAndSettle();
      await tester.sendKeyDownEvent(LogicalKeyboardKey.shiftLeft);
      await tester.sendKeyEvent(LogicalKeyboardKey.tab);
      await tester.sendKeyUpEvent(LogicalKeyboardKey.shiftLeft);
      await tester.pumpAndSettle();
    }

    await tabs();
    expect(input.hasFocus, isTrue);
    await tester.tap(find.text('Open dialog'));
    await tester.pumpAndSettle();
    expect(dialogInput.hasFocus, isTrue);
    await tabs();
    expect(dialogInput.hasFocus, isTrue);
    expect(find.byType(AlertDialog), findsOneWidget);
    expect(tester.takeException(), isNull);
    await tester.pumpWidget(const SizedBox.shrink());
  });
}
