import 'dart:ui' show Tristate;
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:anywhere/ui/app_style.dart';
import 'package:anywhere/settings/settings_components.dart';
import 'package:anywhere/settings/intent_shortcut_dialog.dart';
import 'package:anywhere/settings/host_settings.dart';

void main() {
  testWidgets('context sheet remains usable with larger desktop text',
      (tester) async {
    tester.view.physicalSize = const Size(648, 508);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    await tester.pumpWidget(MaterialApp(
        theme: SettingsStyle.theme(AppStyle.theme),
        builder: (context, child) => MediaQuery(
            data: MediaQuery.of(context)
                .copyWith(textScaler: TextScaler.linear(1.4)),
            child: child!),
        home: Scaffold(
            body: Builder(
                builder: (context) => TextButton(
                    onPressed: () => showDialog<void>(
                        context: context,
                        builder: (_) => IntentShortcutDialog(
                            actions: const [],
                            applications: const [],
                            settings: HostSettings.defaults)),
                    child: const Text('Open'))))));
    await tester.tap(find.text('Open'));
    await tester.pumpAndSettle();
    expect(tester.takeException(), isNull);
    expect(find.text('Save context shortcut').hitTestable(), findsOneWidget);
    final ruleList = find.descendant(
        of: find.byType(IntentShortcutDialog), matching: find.byType(ListView));
    await tester.scrollUntilVisible(find.text('Add rule'), 80,
        scrollable:
            find.descendant(of: ruleList, matching: find.byType(Scrollable)));
    await tester.pumpAndSettle();
    expect(find.text('Add rule').hitTestable(), findsOneWidget);
    await tester.tap(find.text('Cancel'));
    await tester.pumpAndSettle();
    expect(find.byType(IntentShortcutDialog), findsNothing);
  });

  testWidgets('compact toggle supports pointer, keyboard and state semantics',
      (tester) async {
    final semantics = tester.ensureSemantics();
    try {
      var value = false;
      await tester.pumpWidget(MaterialApp(
          theme: SettingsStyle.theme(AppStyle.theme),
          home: Scaffold(
              body: StatefulBuilder(
                  builder: (context, setState) => AppToggle(
                      label: 'Capture source',
                      value: value,
                      onChanged: (v) => setState(() => value = v))))));
      final toggle = find.byType(AppToggle);
      expect(tester.getSize(toggle).width, lessThanOrEqualTo(40));
      expect(tester.getSize(toggle).height, lessThanOrEqualTo(30));
      expect(tester.getSemantics(toggle).flagsCollection.isToggled,
          Tristate.isFalse);
      await tester.tap(toggle);
      await tester.pump();
      expect(value, isTrue);
      expect(tester.getSemantics(toggle).flagsCollection.isToggled,
          Tristate.isTrue);
      FocusScope.of(tester.element(toggle)).nextFocus();
      await tester.pump();
      await tester.sendKeyEvent(LogicalKeyboardKey.space);
      await tester.pump();
      expect(value, isFalse);
    } finally {
      semantics.dispose();
    }
  });

  testWidgets('disabled toggle keeps state and cannot be activated',
      (tester) async {
    await tester.pumpWidget(MaterialApp(
        theme: SettingsStyle.theme(AppStyle.theme),
        home: const Scaffold(
            body:
                AppToggle(label: 'Recording', value: true, onChanged: null))));
    await tester.tap(find.byType(AppToggle));
    await tester.pump();
    expect(tester.widget<AppToggle>(find.byType(AppToggle)).value, isTrue);
    expect(
        tester.widget<TextButton>(find.byType(TextButton)).onPressed, isNull);
  });
}
