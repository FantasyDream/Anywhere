import 'package:anywhere/infrastructure/desktop_environment.dart';
import 'package:anywhere/launcher/widgets/candidate_row.dart';
import 'package:anywhere/src/rust/api.dart';
import 'package:anywhere/ui/app_components.dart';
import 'package:anywhere/ui/app_icons.dart';
import 'package:anywhere/ui/app_style.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  setUp(() => DesktopEnvironment.configure(
      const DesktopEnvironment(platform: TargetPlatform.macOS)));
  tearDown(DesktopEnvironment.reset);

  CandidateDto menu({bool enabled = true}) => CandidateDto(
      id: 'menu:copy',
      title: 'Copy',
      kind: CandidateKindDto.action,
      iconKey: 'menu',
      menuPath: const ['Edit'],
      isMenu: true,
      shortcut: '⌘C',
      score: 1,
      groupId: 'app',
      contextKeys: const [],
      homeEligible: true,
      primaryAction: false,
      availability:
          enabled ? AvailabilityDto.enabled : AvailabilityDto.disabled);

  Future<void> mount(WidgetTester tester,
      {bool selected = true,
      bool enabled = true,
      String? shortcut,
      VoidCallback? select,
      VoidCallback? execute,
      VoidCallback? actions}) async {
    await tester.pumpWidget(MaterialApp(
        theme: AppStyle.theme,
        home: Material(
            child: Center(
                child: SizedBox(
                    width: 400,
                    child: CandidateRow(
                        candidate: menu(enabled: enabled),
                        selected: selected,
                        actionShortcut: shortcut,
                        primaryLabel: 'Run',
                        onTap: select ?? () {},
                        onPrimary: execute ?? () {},
                        onActions: actions))))));
    await tester.pumpAndSettle();
  }

  testWidgets('native menu shortcuts are hidden; user bindings remain',
      (tester) async {
    await mount(tester);
    expect(find.text('⌘C'), findsNothing);
    await mount(tester, shortcut: '⌥T');
    expect(find.text('⌘C'), findsNothing);
    expect(find.text('⌥T'), findsOneWidget);
    expect(find.byTooltip('Anywhere action shortcut'), findsOneWidget);
  });

  testWidgets('row commands appear only for the selected available object',
      (tester) async {
    await mount(tester, selected: false);
    expect(find.byType(AppRowActions), findsNothing);
    await mount(tester, enabled: false);
    expect(find.byType(AppRowActions), findsNothing);
    await mount(tester);
    expect(find.byType(AppRowActions), findsOneWidget);
    expect(tester.takeException(), isNull);
  });

  testWidgets('selection, execution and more actions are separate targets',
      (tester) async {
    var selected = 0, executed = 0, actions = 0;
    await mount(tester,
        select: () => selected++,
        execute: () => executed++,
        actions: () => actions++);
    await tester.tap(find.text('Copy'));
    expect(selected, 1);
    expect(executed, 0);
    await tester.tap(find.byKey(const Key('selected-row-actions')));
    expect(actions, 1);
    expect(executed, 0);
    await tester.tap(find.byKey(const Key('selected-row-primary')));
    expect(executed, 1);
    expect(selected, 3, reason: 'pointer commands select before activation');
    expect(tester.takeException(), isNull);
  });

  testWidgets('visible more shortcut replaces Return only when available',
      (tester) async {
    await mount(tester, actions: () {});
    expect(find.text('⌘K'), findsOneWidget);
    expect(find.byTooltip('Actions · ⌘K'), findsOneWidget);
    expect(find.text('Run'), findsOneWidget);
    expect(find.byIcon(AppIcons.enter), findsNothing);
    await mount(tester);
    expect(find.text('⌘K'), findsNothing);
    expect(find.byIcon(AppIcons.enter), findsOneWidget);
    DesktopEnvironment.configure(
        const DesktopEnvironment(platform: TargetPlatform.windows));
    await mount(tester, actions: () {});
    expect(find.text('Ctrl+K'), findsOneWidget);
    expect(find.text('⌘K'), findsNothing);
    expect(find.byIcon(AppIcons.enter), findsNothing);
    expect(tester.takeException(), isNull);
  });
}
