import 'dart:ui' show PointerDeviceKind, Tristate;
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:anywhere/ui/app_components.dart';
import 'package:anywhere/ui/app_style.dart';
import 'package:anywhere/settings/settings_components.dart';
import 'package:anywhere/web_plugins/web_host_panels.dart';

void main() {
  testWidgets('selected row stays distinct while pointer moves over it',
      (tester) async {
    var calls = 0;
    await tester.pumpWidget(MaterialApp(
        theme: AppStyle.theme,
        home: Scaffold(
            body: Center(
                child: AppSelectionSurface(
                    selected: true,
                    onTap: () => calls++,
                    child: const SizedBox(width: 200, height: 44))))));
    final row = find.byType(AppSelectionSurface);
    final material = find.descendant(of: row, matching: find.byType(Material));
    final ink = find.descendant(of: row, matching: find.byType(InkWell));
    expect(tester.widget<Material>(material).color, AppStyle.selectedSurface);
    expect(tester.widget<InkWell>(ink).hoverColor, Colors.transparent);
    final pointer = await tester.createGesture(kind: PointerDeviceKind.mouse);
    await pointer.addPointer(location: Offset.zero);
    await pointer.moveTo(tester.getCenter(row));
    await tester.pumpAndSettle();
    expect(tester.widget<Material>(material).color, AppStyle.selectedSurface);
    await tester.tap(row);
    expect(calls, 1);
    await pointer.removePointer();
  });

  testWidgets(
      'unavailable row cannot activate and hover is lighter than selection',
      (tester) async {
    await tester.pumpWidget(MaterialApp(
        theme: AppStyle.theme,
        home: const Scaffold(
            body: AppSelectionSurface(
                selected: false, child: SizedBox(width: 200, height: 44)))));
    final row = find.byType(AppSelectionSurface);
    final ink = tester.widget<InkWell>(
        find.descendant(of: row, matching: find.byType(InkWell)));
    expect(ink.onTap, isNull);
    expect(ink.hoverColor!.a, lessThan(AppStyle.selectedSurface.a));
    await tester.tap(row);
    expect(tester.takeException(), isNull);
  });

  test('settings inherits shared button sizes, weights and focus affordance',
      () {
    final shared = AppStyle.theme;
    final settings = SettingsStyle.theme(shared);
    for (final states in [
      <WidgetState>{},
      {WidgetState.focused},
      {WidgetState.disabled},
    ]) {
      expect(settings.filledButtonTheme.style!.textStyle!.resolve(states),
          shared.filledButtonTheme.style!.textStyle!.resolve(states));
      expect(settings.outlinedButtonTheme.style!.side!.resolve(states),
          shared.outlinedButtonTheme.style!.side!.resolve(states));
      expect(settings.iconButtonTheme.style!.minimumSize!.resolve(states),
          shared.iconButtonTheme.style!.minimumSize!.resolve(states));
    }
  });

  testWidgets('compact notices fit launcher height with long text and action',
      (tester) async {
    var opened = false;
    await tester.pumpWidget(MaterialApp(
        theme: AppStyle.theme,
        home: Scaffold(
            body: Center(
                child: SizedBox(
                    width: 608,
                    height: 36,
                    child: AppNotice(
                        'Some locations could not be searched. Available results are still usable. ' *
                            5,
                        compact: true,
                        trailing: TextButton(
                            onPressed: () => opened = true,
                            child: const Text('Open Settings'))))))));
    expect(tester.takeException(), isNull);
    await tester.tap(find.text('Open Settings'));
    expect(opened, isTrue);
  });

  testWidgets('web permission actions wrap in a narrow window at large text',
      (tester) async {
    tester.view.physicalSize = const Size(320, 300);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    bool? allowed;
    await tester.pumpWidget(MaterialApp(
        theme: AppStyle.theme,
        builder: (context, child) => MediaQuery(
            data: MediaQuery.of(context)
                .copyWith(textScaler: TextScaler.linear(1.5)),
            child: child!),
        home: Scaffold(
            body: Stack(children: [
          WebConsentPanel(
              title: 'Copy text',
              toolTitle: 'JSON Workspace',
              allowSelected: true,
              onAnswer: (value) => allowed = value)
        ]))));
    expect(tester.takeException(), isNull);
    await tester.ensureVisible(find.text('✓ Allow once'));
    await tester.tap(find.text('✓ Allow once'));
    expect(allowed, isTrue);
  });

  testWidgets(
      'web action menu keeps selection semantics and commands at large text',
      (tester) async {
    tester.view.physicalSize = const Size(400, 400);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    final semantics = tester.ensureSemantics();
    try {
      int? chosen;
      await tester.pumpWidget(MaterialApp(
          theme: AppStyle.theme,
          builder: (context, child) => MediaQuery(
              data: MediaQuery.of(context)
                  .copyWith(textScaler: TextScaler.linear(1.4)),
              child: child!),
          home: Scaffold(
              body:
                  WebActionsPanel(selected: 1, onSelect: (i) => chosen = i))));
      expect(tester.takeException(), isNull);
      expect(
          tester
              .getSemantics(find.text('Back to search · Esc'))
              .flagsCollection
              .isSelected,
          Tristate.isTrue);
      await tester.tap(find.text('Reload tool'));
      expect(chosen, 0);
      await tester.tap(find.text('Back to search · Esc'));
      expect(chosen, 1);
    } finally {
      semantics.dispose();
    }
  });
}
