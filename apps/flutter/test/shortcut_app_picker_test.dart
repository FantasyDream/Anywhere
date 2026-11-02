import 'package:anywhere/settings/shortcut_app_picker.dart';
import 'package:anywhere/src/rust/api.dart';
import 'package:anywhere/ui/app_style.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

class CountedApplication extends ShortcutApplicationDto {
  CountedApplication(int index)
      : super(bundleId: 'example.$index', title: 'App $index');
  static int titleReads = 0;
  @override
  String get title {
    ++titleReads;
    return super.title;
  }
}

void main() {
  final apps = [
    for (var i = 0; i < 120; i++)
      ShortcutApplicationDto(bundleId: 'example.app.$i', title: 'App $i'),
    const ShortcutApplicationDto(bundleId: 'com.apple.finder', title: 'Finder'),
    const ShortcutApplicationDto(bundleId: 'missing.app', title: 'Saved app'),
  ];
  for (final query in ['App', '']) {
    testWidgets('searching "$query" after scrolling shows the selected app',
        (tester) async {
      ShortcutApplicationChoice? result;
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
      if (query.isEmpty) {
        await tester.enterText(find.byType(TextField), 'App');
        await tester.pumpAndSettle();
      }
      await tester.drag(find.byType(ListView), const Offset(0, -3500));
      await tester.pumpAndSettle();
      await tester.enterText(find.byType(TextField), query);
      await tester.pumpAndSettle();
      final chosen = query.isEmpty ? 'any-app' : 'example.app.0';
      final selected = find.byKey(ValueKey(chosen));
      expect(selected.hitTestable(), findsOneWidget);
      expect(tester.widget<ListTile>(selected).selected, isTrue);
      await tester.testTextInput.receiveAction(TextInputAction.done);
      await tester.pumpAndSettle();
      expect(result, isNotNull);
      expect(result!.app?.bundleId, query.isEmpty ? null : 'example.app.0');
    });
  }

  testWidgets('submitting before the next frame uses the latest query matches',
      (tester) async {
    ShortcutApplicationChoice? result;
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
    tester.testTextInput.updateEditingValue(const TextEditingValue(
        text: 'Finder', selection: TextSelection.collapsed(offset: 6)));
    // The input and submit events can arrive before Flutter paints a new frame.
    await tester.testTextInput.receiveAction(TextInputAction.done);
    await tester.pumpAndSettle();
    expect(result?.app?.bundleId, 'com.apple.finder');
  });
  testWidgets('app picker navigation reuses search results', (tester) async {
    await tester.pumpWidget(MaterialApp(
        theme: AppStyle.theme,
        home: ShortcutAppPicker(
            applications: List.generate(2000, CountedApplication.new))));
    await tester.pumpAndSettle();
    CountedApplication.titleReads = 0;
    for (var i = 0; i < 2; i++) {
      await tester.sendKeyEvent(LogicalKeyboardKey.arrowDown);
      await tester.pump();
    }
    debugPrint(
        'app_titles_read_for_two_arrows=${CountedApplication.titleReads}');
    expect(CountedApplication.titleReads, lessThan(200));
    expect(
        tester
            .widget<ListTile>(find.byKey(const ValueKey('example.1')))
            .selected,
        isTrue);
    await tester.pumpWidget(const SizedBox());
  });

  testWidgets(
      'catalog replacement preserves the chosen app or falls back safely',
      (tester) async {
    final first = apps[0], second = apps[1];
    var catalog = [first, second];
    late StateSetter updateCatalog;
    await tester.pumpWidget(MaterialApp(
        theme: AppStyle.theme,
        home: StatefulBuilder(builder: (context, setState) {
          updateCatalog = setState;
          return ShortcutAppPicker(applications: catalog);
        })));
    await tester.pumpAndSettle();
    await tester.sendKeyEvent(LogicalKeyboardKey.arrowDown);
    await tester.sendKeyEvent(LogicalKeyboardKey.arrowDown);
    await tester.pump();
    ListTile selected() => tester.widget<ListTile>(find
        .byWidgetPredicate((widget) => widget is ListTile && widget.selected));
    expect(selected().key, ValueKey(second.bundleId));
    updateCatalog(() => catalog = [second, first]);
    await tester.pump();
    expect(selected().key, ValueKey(second.bundleId));
    updateCatalog(() => catalog = [first]);
    await tester.pump();
    expect(selected().key, const ValueKey('any-app'));
    expect(tester.takeException(), isNull);
    await tester.pumpWidget(const SizedBox());
  });

  testWidgets('app picker leaves candidate arrows to the input method',
      (tester) async {
    await tester.pumpWidget(MaterialApp(
        theme: AppStyle.theme, home: ShortcutAppPicker(applications: apps)));
    await tester.pumpAndSettle();
    tester.testTextInput.updateEditingValue(const TextEditingValue(
        text: 'App',
        selection: TextSelection.collapsed(offset: 3),
        composing: TextRange(start: 0, end: 3)));
    await tester.pump();
    ListTile selected() => tester.widget<ListTile>(find
        .byWidgetPredicate((widget) => widget is ListTile && widget.selected));
    expect(selected().key, const ValueKey('example.app.0'));
    await tester.sendKeyEvent(LogicalKeyboardKey.arrowDown);
    await tester.pump();
    expect(selected().key, const ValueKey('example.app.0'));
    await tester.pumpWidget(const SizedBox());
  });
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
