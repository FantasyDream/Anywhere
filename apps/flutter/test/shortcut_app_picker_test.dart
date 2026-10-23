import 'package:anywhere/settings/shortcut_app_picker.dart';
import 'package:anywhere/src/rust/api.dart';
import 'package:anywhere/ui/app_style.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  final apps = [
    for (var i = 0; i < 120; i++)
      ShortcutApplicationDto(bundleId: 'example.app.$i', title: 'App $i'),
    const ShortcutApplicationDto(bundleId: 'com.apple.finder', title: 'Finder'),
    const ShortcutApplicationDto(bundleId: 'missing.app', title: 'Saved app'),
  ];
  testWidgets(
      'app picker searches names and IDs and supports keyboard selection',
      (tester) async {
    ShortcutApplicationChoice? result;
    tester.view.physicalSize = const Size(648, 508);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    await tester.pumpWidget(MaterialApp(
        theme: AppStyle.theme,
        home: Builder(
            builder: (context) => TextButton(
                onPressed: () async {
                  result = await showDialog<ShortcutApplicationChoice>(
                      context: context,
                      builder: (_) => ShortcutAppPicker(applications: apps));
                },
                child: const Text('Choose')))));
    await tester.tap(find.text('Choose'));
    await tester.pumpAndSettle();
    await tester.enterText(find.byType(TextField), ' APPLE.FINDER ');
    await tester.pumpAndSettle();
    expect(find.text('Finder'), findsOneWidget);
    expect(find.text('App 1'), findsNothing);
    await tester.testTextInput.receiveAction(TextInputAction.done);
    await tester.pumpAndSettle();
    expect(result?.app?.bundleId, 'com.apple.finder');
    result = null;
    await tester.tap(find.text('Choose'));
    await tester.pumpAndSettle();
    await tester.enterText(find.byType(TextField), 'App 119');
    await tester.pumpAndSettle();
    await tester.sendKeyEvent(LogicalKeyboardKey.arrowUp);
    await tester.testTextInput.receiveAction(TextInputAction.done);
    await tester.pumpAndSettle();
    expect(result, isNotNull);
    expect(result!.app, isNull); // Explicit Any app differs from Cancel.
    expect(tester.takeException(), isNull);
  });

  testWidgets('no matches and cancelling preserve the prior app choice',
      (tester) async {
    ShortcutApplicationChoice? result = const ShortcutApplicationChoice(
        ShortcutApplicationDto(bundleId: 'missing.app', title: 'Saved app'));
    await tester.pumpWidget(MaterialApp(
        theme: AppStyle.theme,
        home: Builder(
            builder: (context) => TextButton(
                onPressed: () async {
                  final selected = await showDialog<ShortcutApplicationChoice>(
                      context: context,
                      builder: (_) => ShortcutAppPicker(
                          applications: apps,
                          selectedId: result?.app?.bundleId));
                  if (selected != null) result = selected;
                },
                child: const Text('Choose')))));
    await tester.tap(find.text('Choose'));
    await tester.pumpAndSettle();
    await tester.enterText(find.byType(TextField), 'nothing-matches');
    await tester.pumpAndSettle();
    expect(find.text('No matching apps'), findsOneWidget);
    expect(find.text('Any app'), findsOneWidget);
    await tester.tap(find.text('Cancel'));
    await tester.pumpAndSettle();
    expect(result?.app?.bundleId, 'missing.app');
    await tester.tap(find.text('Choose'));
    await tester.pumpAndSettle();
    await tester.enterText(find.byType(TextField), 'Saved app');
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('missing.app')));
    await tester.pumpAndSettle();
    expect(result?.app?.bundleId, 'missing.app');
  });

  testWidgets(
      'larger text keeps a keyboard-selected app visible in a long list',
      (tester) async {
    ShortcutApplicationChoice? result;
    tester.view.physicalSize = const Size(648, 508);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    await tester.pumpWidget(MaterialApp(
        theme: AppStyle.theme,
        builder: (context, child) => MediaQuery(
            data: MediaQuery.of(context)
                .copyWith(textScaler: TextScaler.linear(1.4)),
            child: child!),
        home: Builder(
            builder: (context) => TextButton(
                onPressed: () async {
                  result = await showDialog<ShortcutApplicationChoice>(
                      context: context,
                      builder: (_) => ShortcutAppPicker(applications: apps));
                },
                child: const Text('Choose')))));
    await tester.tap(find.text('Choose'));
    await tester.pumpAndSettle();
    for (var i = 0; i < 25; i++) {
      await tester.sendKeyEvent(LogicalKeyboardKey.arrowDown);
      await tester.pump();
    }
    final selected = find.byKey(const ValueKey('example.app.24'));
    expect(selected.hitTestable(), findsOneWidget);
    expect(tester.widget<ListTile>(selected).selected, isTrue);
    expect(tester.takeException(), isNull);
    await tester.testTextInput.receiveAction(TextInputAction.done);
    await tester.pumpAndSettle();
    expect(result?.app?.bundleId, 'example.app.24');
  });
}
