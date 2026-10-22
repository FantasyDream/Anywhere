import 'package:anywhere/execution/execution_command.dart';
import 'package:anywhere/settings/action_shortcut.dart';
import 'package:anywhere/infrastructure/desktop_environment.dart';
import 'package:anywhere/infrastructure/runtime_paths.dart';
import 'package:anywhere/settings/host_settings.dart';
import 'package:anywhere/ui/app_style.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  const channel = MethodChannel('test/portable-host');
  tearDown(() {
    DesktopEnvironment.reset();
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(channel, null);
  });

  test(
      'legacy 1–3 settings migrate physical keys without changing action identity',
      () {
    for (final version in [1, 2, 3]) {
      final settings = HostSettings.fromMap({
        'actionShortcutVersion': version,
        'currentAppKeyCode': 49,
        'currentAppModifiers': 2304,
        'universalKeyCode': 49,
        'universalModifiers': 2816,
        'actionShortcuts': [
          {
            'actionId': 'core.directory.terminal',
            'title': 'Terminal',
            'keyCode': 17,
            'modifiers': 2048,
            'global': true,
          }
        ],
      });
      expect(settings.actionShortcutVersion, 4);
      expect(settings.currentAppKeyCode, 44);
      expect(settings.currentAppModifiers,
          HostSettings.metaKey | HostSettings.altKey);
      final binding = settings.actionShortcuts.single;
      expect(binding.actionId, 'core.directory.terminal');
      expect(binding.keyCode, 23);
      expect(binding.modifiers, HostSettings.altKey);
      expect(binding.global, version != 1);
      expect(HostSettings.fromMap(settings.toMap()).toMap(), settings.toMap());
    }
  });

  test('an unknown legacy key cannot become a different action shortcut', () {
    final settings = HostSettings.fromMap({
      'actionShortcutVersion': 3,
      'actionShortcuts': [
        {'actionId': 'copy', 'title': 'Copy', 'keyCode': 999, 'modifiers': 2048}
      ],
    });
    expect(settings.actionShortcuts, isEmpty);
  });

  test('physical key identity is the same on macOS and Windows', () {
    for (final platform in [TargetPlatform.macOS, TargetPlatform.windows]) {
      DesktopEnvironment.configure(DesktopEnvironment(platform: platform));
      expect(
          shortcutEventKeyCode(const KeyDownEvent(
              physicalKey: PhysicalKeyboardKey.keyZ,
              logicalKey: LogicalKeyboardKey.keyY,
              timeStamp: Duration.zero)),
          29);
    }
    expect(formatHotkey(23, HostSettings.controlKey | HostSettings.altKey),
        'Ctrl+Alt+T');
    expect(AppStyle.theme.textTheme.bodyMedium!.fontFamily, 'Segoe UI');
  });

  testWidgets('Windows uses Ctrl for shared page commands', (tester) async {
    DesktopEnvironment.configure(
        const DesktopEnvironment(platform: TargetPlatform.windows));
    var calls = 0;
    await tester.pumpWidget(MaterialApp(
        home: CallbackShortcuts(bindings: {
      DesktopEnvironment.current.command(LogicalKeyboardKey.keyF): () => calls++
    }, child: const Focus(autofocus: true, child: Text('Find')))));
    await tester.pump();
    await tester.sendKeyDownEvent(LogicalKeyboardKey.controlLeft);
    await tester.sendKeyEvent(LogicalKeyboardKey.keyF);
    await tester.sendKeyUpEvent(LogicalKeyboardKey.controlLeft);
    expect(calls, 1);
  });

  testWidgets(
      'glass backdrop stays transparent and unsupported hosts use the original opaque root',
      (tester) async {
    Future<Color> surface(bool backdrop) async {
      DesktopEnvironment.configure(DesktopEnvironment(
          platform: TargetPlatform.windows, hasBackdrop: backdrop));
      await tester.pumpWidget(
          MaterialApp(builder: AppStyle.windowBuilder, home: const SizedBox()));
      return tester.widget<ColoredBox>(find.byType(ColoredBox).first).color;
    }

    expect(await surface(false), const Color(0xff24262b));
    expect(await surface(true), Colors.transparent);
  });

  test(
      'capabilities default to opaque and Flutter recording on unsupported hosts',
      () async {
    final environment = await DesktopEnvironment.read(channel);
    expect(environment.hasBackdrop, isFalse);
    expect(environment.nativeShortcutRecording, isFalse);
  });

  test(
      'Windows reserves Ctrl editing chords and formats copied commands as PowerShell',
      () {
    DesktopEnvironment.configure(
        const DesktopEnvironment(platform: TargetPlatform.windows));
    const copy = ActionShortcut(
        actionId: 'copy',
        title: 'Copy',
        keyCode: 6,
        modifiers: HostSettings.controlKey);
    expect(copy.chordError, contains('reserved'));
    expect(
        formatExecutionCommand(
            [r"C:\tool path\tool.exe", r"it's a $(literal)"]),
        r"& 'C:\tool path\tool.exe' 'it''s a $(literal)'");
  });

  test('host supplies library paths without assuming a macOS bundle', () async {
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(
            channel,
            (call) async => {
                  'library': r'C:\Anywhere\anywhere_bridge_flutter.dll',
                  'data': r'C:\User\Anywhere',
                  'plugins': r'C:\Anywhere\plugins',
                });
    final paths = await RuntimePaths.read(channel);
    expect(paths.library, endsWith('.dll'));
    expect(paths.requiredPath('data'), r'C:\User\Anywhere');
    expect(() => paths.requiredPath('missing'), throwsStateError);
  });
}
