import 'package:anywhere/infrastructure/bridge_api.dart';
import 'package:anywhere/l10n/app_language.dart';
import 'package:anywhere/settings/action_shortcut.dart';
import 'package:anywhere/settings/shortcut_action_picker.dart';
import 'package:anywhere/ui/app_style.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

class CountedAction extends ShortcutActionDto {
  CountedAction(int index)
      : super(
            actionId: '$index', title: 'Action $index', contextTypes: const []);
  static int titleReads = 0;
  @override
  String get title {
    ++titleReads;
    return super.title;
  }
}

void main() {
  testWidgets('editing or clearing a search reveals the first matching action',
      (tester) async {
    final query = TextEditingController();
    addTearDown(query.dispose);
    final actions = List.generate(
        120,
        (index) => ShortcutActionDto(
            actionId: '$index',
            title: '${index < 60 ? 'First' : 'Second'} action $index',
            contextTypes: const []));
    await tester.pumpWidget(MaterialApp(
        theme: AppStyle.theme,
        home: Scaffold(
            body: SizedBox(
                height: 300,
                child: ShortcutActionPicker(
                    actions: actions, query: query, onSelected: (_) {})))));
    await tester.pumpAndSettle();
    await tester.drag(find.byType(ListView), const Offset(0, -1500));
    await tester.pumpAndSettle();
    expect(find.text('First action 0').hitTestable(), findsNothing);
    query.text = 'Second';
    await tester.pumpAndSettle();
    expect(find.text('Second action 60').hitTestable(), findsOneWidget);
    await tester.drag(find.byType(ListView), const Offset(0, -1000));
    await tester.pumpAndSettle();
    query.clear();
    await tester.pumpAndSettle();
    expect(find.text('First action 0').hitTestable(), findsOneWidget);
  });

  testWidgets(
      'caret and parent rebuilds reuse matches; catalog replacement does not',
      (tester) async {
    final query = TextEditingController(text: 'Action 1999');
    addTearDown(query.dispose);
    final actions = List.generate(2000, CountedAction.new);
    ShortcutActionDto? selected;
    Widget frame(List<ShortcutActionDto> catalog) => MaterialApp(
        theme: AppStyle.theme,
        home: Scaffold(
            body: SizedBox(
                height: 300,
                child: ShortcutActionPicker(
                    actions: catalog,
                    query: query,
                    onSelected: (action) => selected = action))));
    await tester.pumpWidget(frame(actions));
    await tester.pumpAndSettle();
    expect(find.widgetWithText(ListTile, 'Action 1999'), findsOneWidget);
    CountedAction.titleReads = 0;
    query.selection = const TextSelection.collapsed(offset: 0);
    await tester.pumpWidget(frame(actions));
    await tester.pumpAndSettle();
    expect(CountedAction.titleReads, lessThan(10),
        reason:
            'caret changes and rebuilding the parent must not rescan 2000 actions');

    const replacement = ShortcutActionDto(
        actionId: 'replacement',
        title: 'Action 1999 updated',
        contextTypes: []);
    await tester.pumpWidget(frame([replacement]));
    await tester.pumpAndSettle();
    expect(find.widgetWithText(ListTile, 'Action 1999'), findsNothing);
    await tester.tap(find.widgetWithText(ListTile, replacement.title));
    expect(identical(selected, replacement), isTrue);
    await tester.pumpWidget(frame(const []));
    await tester.pumpAndSettle();
    expect(find.byType(ListTile), findsNothing);
  });

  testWidgets('locale changes refresh context matching and displayed titles',
      (tester) async {
    const action = ShortcutActionDto(
        actionId: 'inspect',
        title: 'Inspect',
        localizedTitles: {'zh': '查看'},
        contextTypes: ['core.directory']);
    final query = TextEditingController(
        text: contextTypeLabel(
            'core.directory', lookupAppLocalizations(const Locale('en'))));
    addTearDown(query.dispose);
    Widget frame(String language) => MaterialApp(
        theme: AppStyle.theme,
        locale: Locale(language),
        localizationsDelegates: AppLocalizations.localizationsDelegates,
        supportedLocales: AppLocalizations.supportedLocales,
        home: Scaffold(
            body: SizedBox(
                height: 300,
                child: ShortcutActionPicker(
                    actions: const [action],
                    query: query,
                    onSelected: (_) {}))));
    await tester.pumpWidget(frame('en'));
    await tester.pumpAndSettle();
    expect(find.widgetWithText(ListTile, 'Inspect'), findsOneWidget);
    await tester.pumpWidget(frame('zh'));
    await tester.pumpAndSettle();
    expect(find.byType(ListTile), findsNothing);
    query.text = contextTypeLabel(
        'core.directory', lookupAppLocalizations(const Locale('zh')));
    await tester.pumpAndSettle();
    expect(find.widgetWithText(ListTile, '查看'), findsOneWidget);
    query.text = 'core.directory';
    await tester.pumpAndSettle();
    expect(find.widgetWithText(ListTile, '查看'), findsOneWidget);
  });
}
