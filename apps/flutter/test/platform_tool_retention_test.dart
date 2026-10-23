import 'package:anywhere/infrastructure/window_host.dart';
import 'package:anywhere/launcher/launcher_controller.dart';
import 'package:anywhere/launcher/launcher_screen.dart';
import 'package:anywhere/settings/settings_screen.dart';
import 'package:anywhere/src/rust/api.dart';
import 'package:anywhere/tools/tool_page.dart';
import 'package:anywhere/ui/app_style.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'launcher_fixtures.dart';

class _Editor extends StatefulWidget {
  const _Editor(this.session);
  final _Page session;
  @override
  State<_Editor> createState() => _EditorState();
}

class _EditorState extends State<_Editor> {
  final input = TextEditingController();
  @override
  void initState() {
    super.initState();
    widget.session.created++;
  }

  @override
  void dispose() {
    widget.session.destroyed++;
    input.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) =>
      TextField(key: const Key('platform-editor'), controller: input);
}

class _Page extends ToolPageSession {
  int created = 0;
  int destroyed = 0;
  bool visible = false;
  @override
  String get id => 'test.platform';
  @override
  String get title => 'Retained editor';
  @override
  bool get embedsPlatformView => true;
  @override
  Widget build(BuildContext context,
      {required VoidCallback onBack,
      required int presentation,
      required bool visible,
      String? notice,
      Future<void> Function()? onCopy}) {
    this.visible = visible;
    return _Editor(this);
  }
}

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  const channel = MethodChannel('test/platform-retention');
  final messenger =
      TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger;
  setUp(() => messenger.setMockMethodCallHandler(channel, (_) async => null));
  tearDown(() => messenger.setMockMethodCallHandler(channel, null));

  Future<LauncherController> open(WidgetTester tester, _Page page) async {
    var session = 0;
    LauncherStateDto? held;
    const candidate = CandidateDto(
        id: 'web:test:open',
        commandId: 'web:test:open',
        kind: CandidateKindDto.action,
        title: 'Open tool',
        menuPath: [],
        contextKeys: [],
        homeEligible: true,
        primaryAction: true,
        iconKey: 'file',
        score: 100,
        groupId: 'test',
        availability: AvailabilityDto.enabled);
    LauncherStateDto snapshot(int id, int revision, String text) =>
        launcherSnapshot(
            sessionId: id,
            revision: revision,
            snapshotSeq: 1,
            input: text,
            contextComplete: true,
            candidates: [candidate],
            diagnostics: []);
    final host = LauncherController(
        window: WindowHost(channel),
        readExtensions: () => [],
        toolPages: ToolPageRegistry([
          ToolPageRegistration(
              id: page.id, view: HostViewDto.webPlugin, open: (_) => page)
        ]),
        backend: LauncherBackend(
            beginActivation: () => ++session,
            activate: (id,
                    {promptAccessibility = false,
                    capturedEnvironment = false}) async =>
                held = snapshot(id, 0, ''),
            updateInput: (id, text, revision,
                    {scopeId, inputMode = InputModeDto.environment}) async =>
                held = snapshot(id, revision, text),
            execute: (id, revision, _) async => ActionResultDto(
                sessionId: id,
                revision: revision,
                status: ActionStatusDto.succeeded,
                view: HostViewDto.webPlugin,
                closeWindow: false),
            suspend: (_) async {},
            resume: (_) async => held,
            deactivate: (_) async {}));
    addTearDown(host.dispose);
    await tester.pumpWidget(MaterialApp(
        theme: AppStyle.theme, home: LauncherScreen(controller: host)));
    await host.activate(mode: WakeMode.universal);
    await host.update('tool query');
    await host.executeSelected();
    await tester.pump();
    await tester.enterText(find.byKey(const Key('platform-editor')), 'draft');
    return host;
  }

  testWidgets('Settings keeps the platform editor mounted until resumed',
      (tester) async {
    final page = _Page();
    final host = await open(tester, page);
    await host.openSettings();
    await tester.pump();
    expect(find.byType(SettingsScreen), findsOneWidget);
    expect(page.visible, isFalse);
    expect(page.created, 1);
    expect(page.destroyed, 0);
    await tester.pump(const Duration(seconds: 14));
    await host.closeSettings();
    await host.activate();
    await tester.pump();
    expect(page.visible, isTrue);
    expect(page.created, 1);
    expect(page.destroyed, 0);
    final editor =
        tester.widget<TextField>(find.byKey(const Key('platform-editor')));
    expect(editor.controller!.text, 'draft');
    expect(host.view.input, 'tool query');
    expect(tester.takeException(), isNull);
    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets(
      'retention expiry destroys the editor without remounting Settings',
      (tester) async {
    final page = _Page();
    final host = await open(tester, page);
    await host.openSettings();
    await tester.pump();
    final settings = tester.element(find.byType(SettingsScreen));
    await tester.pump(const Duration(seconds: 16));
    await tester.pump();
    expect(page.destroyed, 1);
    expect(page.disposed, isTrue);
    expect(tester.element(find.byType(SettingsScreen)), same(settings));
    expect(tester.takeException(), isNull);
    await tester.pumpWidget(const SizedBox.shrink());
  });
}
