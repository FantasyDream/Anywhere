import 'package:anywhere/l10n/app_language.dart';
import 'package:anywhere/launcher/widgets/candidate_row.dart';
import 'package:anywhere/settings/shortcut_app_picker.dart';
import 'package:anywhere/src/rust/api.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  const application = CandidateDto(
    id: 'app:com.apple.reminders:/System/Applications/Reminders.app',
    kind: CandidateKindDto.application,
    title: 'Reminders',
    localizedTitles: {'en': 'Reminders', 'zh': '提醒事项'},
    menuPath: [],
    contextKeys: [],
    homeEligible: true,
    primaryAction: true,
    subtitle: '/System/Applications/Reminders.app',
    iconKey: 'application',
    score: 100,
    groupId: 'app',
    availability: AvailabilityDto.enabled,
    bundleId: 'com.apple.reminders',
  );

  testWidgets('language changes update application label and retain launch row',
      (tester) async {
    final language = AppLanguage.current;
    final previous = language.preference;
    addTearDown(() => language.apply(previous));
    var opened = 0;
    language.apply('en');
    await tester.pumpWidget(LocalizedApp(
        home: Scaffold(
            body: CandidateRow(
                candidate: application,
                selected: true,
                onTap: () => opened++))));
    await tester.pumpAndSettle();
    expect(find.text('Reminders'), findsOneWidget);
    language.apply('zh');
    await tester.pumpAndSettle();
    expect(find.text('提醒事项'), findsOneWidget);
    expect(find.text('Reminders'), findsNothing);
    final row = tester.widget<CandidateRow>(find.byType(CandidateRow));
    expect(identical(row.candidate, application), isTrue);
    expect(row.selected, isTrue);
    await tester.tap(find.text('提醒事项'));
    expect(opened, 1);
    language.apply('en');
    await tester.pumpAndSettle();
    expect(find.text('Reminders'), findsOneWidget);
  });

  testWidgets('applications without a supplied translation keep their own name',
      (tester) async {
    await tester.pumpWidget(MaterialApp(
        locale: const Locale('zh'),
        localizationsDelegates: AppLocalizations.localizationsDelegates,
        supportedLocales: AppLocalizations.supportedLocales,
        home: Scaffold(
            body: CandidateRow(
                candidate: CandidateDto(
                    id: 'app:vendor',
                    kind: CandidateKindDto.application,
                    title: 'Vendor Application',
                    menuPath: const [],
                    contextKeys: const [],
                    homeEligible: true,
                    primaryAction: true,
                    iconKey: 'application',
                    score: 100,
                    groupId: 'app',
                    availability: AvailabilityDto.enabled),
                selected: false,
                onTap: () {}))));
    await tester.pumpAndSettle();
    expect(find.text('Vendor Application'), findsOneWidget);
  });

  testWidgets(
      'shortcut app picker shows and searches bundle-supplied Chinese names',
      (tester) async {
    await tester.pumpWidget(MaterialApp(
        locale: const Locale('zh'),
        localizationsDelegates: AppLocalizations.localizationsDelegates,
        supportedLocales: AppLocalizations.supportedLocales,
        home: const Scaffold(
            body: ShortcutAppPicker(applications: [
          ShortcutApplicationDto(
              bundleId: 'com.apple.reminders',
              title: 'Reminders',
              localizedTitles: {'en': 'Reminders', 'zh': '提醒事项'}),
        ]))));
    await tester.pumpAndSettle();
    expect(find.text('提醒事项'), findsOneWidget);
    await tester.enterText(find.byType(TextField), '提醒');
    await tester.pump();
    expect(find.text('提醒事项'), findsOneWidget);
    await tester.enterText(find.byType(TextField), 'Reminders');
    await tester.pump();
    expect(find.text('提醒事项'), findsOneWidget);
  });
}
