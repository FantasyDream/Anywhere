import 'dart:async';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:anywhere/infrastructure/window_host.dart';
import 'package:anywhere/launcher/launcher_controller.dart';
import 'package:anywhere/launcher/launcher_view_state.dart';
import 'package:anywhere/launcher/launcher_screen.dart';
import 'package:anywhere/launcher/launcher_style.dart';
import 'package:anywhere/settings/host_settings.dart';

class SettingsHarness {
  final channel = const MethodChannel('test/settings-window');
  final calls = <MethodCall>[];
  HostSettings settings = HostSettings.defaults;
  Future<void> Function()? saveGate;
  bool failSave = false;
  late final controller = LauncherController(
    backend: LauncherBackend(
      beginActivation: () => 1,
      activate: (_, {promptAccessibility = false}) async => null,
      updateInput: (_, __, ___, {scopeId}) async => null,
      execute: (_, __, ___) async => throw StateError('not invoked'),
      deactivate: (_) async {},
    ),
    window: WindowHost(channel),
    applyExcludedApps: (_) async {},
    applyContextSources: (_) async {},
    readPlugins: () => [],
  );

  Future<void> mount(WidgetTester tester) async {
    tester.view.physicalSize = const Size(648, 508);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    final messenger =
        TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger;
    messenger.setMockMethodCallHandler(channel, (call) async {
      calls.add(call);
      if (call.method == 'settings') return settings.toMap();
      if (call.method == 'settingsStatus') return <String, Object?>{};
      if (call.method == 'saveSettings') {
        await saveGate?.call();
        if (failSave) {
          throw PlatformException(
              code: 'save', message: 'Could not save settings');
        }
        settings = HostSettings.fromMap(call.arguments as Map);
      }
      return null;
    });
    addTearDown(() => messenger.setMockMethodCallHandler(channel, null));
    controller.attach();
    await tester.pumpWidget(MaterialApp(
      theme: LauncherStyle.theme,
      home: LauncherScreen(controller: controller),
    ));
    await tester.pumpAndSettle();
    await controller.openSettings();
    await tester.pumpAndSettle();
    addTearDown(controller.dispose);
  }

  Future<bool?> native(String method) async {
    final reply = Completer<bool?>();
    unawaited(TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .handlePlatformMessage(
      channel.name,
      const StandardMethodCodec().encodeMethodCall(MethodCall(method)),
      (data) =>
          reply.complete(const StandardMethodCodec().decodeEnvelope(data!)),
    ));
    return reply.future;
  }

  Future<void> draft(WidgetTester tester) async {
    final field = find.byType(TextField);
    await tester.scrollUntilVisible(field, 220);
    await tester.pumpAndSettle();
    await tester.enterText(field, 'test.unsaved.app');
    await tester.pump();
  }
}

void main() {
  testWidgets(
      'native close guards drafts, restore preserves them, save closes after applying',
      (tester) async {
    final h = SettingsHarness();
    await h.mount(tester);
    await h.draft(tester);
    final readCount = h.calls.where((c) => c.method == 'settings').length;
    await h.controller.openSettings();
    await tester.pumpAndSettle();
    expect(h.calls.where((c) => c.method == 'settings').length, readCount);
    expect(tester.widget<TextField>(find.byType(TextField)).controller!.text,
        'test.unsaved.app');
    final closing = h.native('closeSettings');
    await tester.pumpAndSettle();
    expect(find.text('Save changes before closing?'), findsOneWidget);
    await tester.tap(find.text('Keep editing'));
    await tester.pumpAndSettle();
    expect(await closing, false);
    expect(h.controller.view.phase, LauncherPhase.settings);
    expect(h.settings.excludedApps, isEmpty);
    final savingClose = h.native('closeSettings');
    await tester.pumpAndSettle();
    await tester.tap(find.text('Save changes'));
    await tester.pumpAndSettle();
    expect(await savingClose, true);
    expect(h.settings.excludedApps, ['test.unsaved.app']);
    expect(h.controller.view.phase, LauncherPhase.hidden);
    expect(h.calls.last.method, 'hide');
    await tester.pumpWidget(const SizedBox());
  });

  testWidgets(
      'native quit and launcher activation honor cancellation; discard does not save',
      (tester) async {
    final h = SettingsHarness();
    await h.mount(tester);
    await h.draft(tester);
    final quitting = h.native('prepareToQuit');
    await tester.pumpAndSettle();
    await tester.tap(find.text('Keep editing'));
    await tester.pumpAndSettle();
    expect(await quitting,
        false); // Rust shutdown is never reached after cancellation.
    final toggling = h.controller.toggle('universal');
    await tester.pumpAndSettle();
    await tester.tap(find.text('Keep editing'));
    await tester.pumpAndSettle();
    await toggling;
    expect(h.controller.view.phase, LauncherPhase.settings);
    final closing = h.native('closeSettings');
    await tester.pumpAndSettle();
    await tester.tap(find.text('Close without saving'));
    await tester.pumpAndSettle();
    expect(await closing, true);
    expect(h.settings.excludedApps, isEmpty);
    expect(h.calls.where((c) => c.method == 'saveSettings'), isEmpty);
    await tester.pumpWidget(const SizedBox());
  });

  testWidgets(
      'close waits for saving and a failed save keeps the page with explicit feedback',
      (tester) async {
    final h = SettingsHarness();
    await h.mount(tester);
    final gate = Completer<void>();
    h.saveGate = () => gate.future;
    h.failSave = true;
    final loginSwitch = find.widgetWithText(SwitchListTile, 'Open at login');
    await tester.scrollUntilVisible(loginSwitch, 160);
    await tester.tap(loginSwitch);
    await tester.pump();
    expect(find.text('Saving…'), findsOneWidget);
    final closing = h.native('closeSettings');
    await tester.pumpAndSettle();
    expect(h.controller.view.phase, LauncherPhase.settings);
    expect(h.calls.where((c) => c.method == 'hide'), isEmpty);
    gate.complete();
    await tester.pumpAndSettle();
    expect(find.text('Some changes were not saved'), findsOneWidget);
    await tester.tap(find.text('Keep editing'));
    await tester.pumpAndSettle();
    expect(await closing, false);
    expect(h.controller.view.phase, LauncherPhase.settings);
    expect(h.settings.launchAtLogin, false);
    expect(find.text('Could not save settings'), findsOneWidget);
    await tester.pumpWidget(const SizedBox());
  });

  testWidgets('Escape cancels recording first, then closes clean settings',
      (tester) async {
    final h = SettingsHarness();
    await h.mount(tester);
    await tester.tap(find.byType(OutlinedButton).first);
    await tester.pump();
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pump();
    expect(find.text('Press a shortcut'), findsNothing);
    expect(h.controller.view.phase, LauncherPhase.settings);
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pumpAndSettle();
    expect(h.controller.view.phase, LauncherPhase.hidden);
    await tester.pumpWidget(const SizedBox());
  });
}
