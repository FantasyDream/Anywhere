import 'dart:async';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:anywhere/infrastructure/clipboard_preferences_backend.dart';
import 'package:anywhere/infrastructure/window_host.dart';
import 'package:anywhere/l10n/app_language.dart';
import 'package:anywhere/settings/clipboard_settings_controller.dart';
import 'package:anywhere/settings/clipboard_settings_section.dart';
import 'package:anywhere/settings/settings_controller.dart';
import 'package:anywhere/settings/settings_components.dart';
import 'package:anywhere/src/rust/api.dart';
import 'package:anywhere/ui/app_style.dart';

class Preferences implements ClipboardPreferencesBackend {
  bool recording = false, available = true, fail = false;
  int cleared = 0, writes = 0;
  Completer<void>? gate;
  @override
  Future<ClipboardHistoryStatusDto> read() async {
    if (fail) throw StateError('unavailable');
    return ClipboardHistoryStatusDto(
        recording: recording, available: available);
  }

  @override
  Future<void> setRecording(bool enabled) async {
    ++writes;
    await gate?.future;
    if (fail) throw StateError('denied');
    recording = enabled;
  }

  @override
  Future<void> clear() async {
    ++cleared;
  }
}

void main() {
  test(
      'preferences readback, failures, and feature disable never acknowledge a stale setting',
      () async {
    final backend = Preferences();
    final controller = ClipboardSettingsController(backend);
    await controller.load();
    expect(controller.loaded, isTrue);
    expect(backend.writes, 0);
    await controller.setRecording(true);
    expect(controller.recording, isTrue);
    backend.fail = true;
    expect(await controller.setRecording(false), isFalse);
    expect(controller.loaded, isFalse);
    expect(controller.notice, isNotNull);
    expect(backend.recording, isTrue);
    backend.fail = false;
    backend.available = false;
    await controller.load();
    expect(controller.available, isFalse);
    controller.dispose();
  });

  test('closing settings waits for a recording update and its readback',
      () async {
    final backend = Preferences()..gate = Completer<void>();
    final settings = SettingsController(
        window: WindowHost(const MethodChannel('unused/settings')),
        clipboardBackend: backend,
        readPlugins: () => []);
    var prepared = false;
    settings.beforeClose = () async {
      prepared = true;
      return true;
    };
    final change = settings.clipboard.setRecording(true);
    final close = settings.prepareClose();
    await Future<void>.delayed(Duration.zero);
    expect(prepared, isFalse);
    backend.gate!.complete();
    expect(await change, isTrue);
    expect(await close, isTrue);
    expect(settings.clipboard.recording, isTrue);
    settings.dispose();
  });

  test('listeners observe a pending write before busy notification', () async {
    final backend = Preferences()..gate = Completer<void>();
    final controller = ClipboardSettingsController(backend);
    Future<bool>? observed;
    controller.addListener(() {
      if (controller.busy) observed = controller.waitForPending();
    });
    final write = controller.setRecording(true);
    var completed = false;
    unawaited(observed!.then((_) => completed = true));
    await Future<void>.delayed(Duration.zero);
    expect(completed, isFalse);
    backend.gate!.complete();
    expect(await write, isTrue);
    expect(await observed, isTrue);
    expect(backend.writes, 1);
    controller.dispose();
  });

  test('a failed pending clipboard save blocks close with a translated message',
      () async {
    final backend = Preferences()
      ..gate = Completer<void>()
      ..fail = true;
    final settings = SettingsController(
        window: WindowHost(const MethodChannel('unused/settings')),
        clipboardBackend: backend,
        readPlugins: () => []);
    addTearDown(settings.dispose);
    final change = settings.clipboard.setRecording(true);
    final close = settings.prepareClose();
    backend.gate!.complete();
    expect(await change, isFalse);
    expect(await close, isFalse);
    final zh = lookupAppLocalizations(const Locale('zh'));
    final en = lookupAppLocalizations(const Locale('en'));
    expect(settings.statusMessageFor(zh), zh.clipboardSettingsFailed);
    expect(settings.statusMessageFor(en), en.clipboardSettingsFailed);
    expect(settings.statusMessageFor(zh), isNot(settings.statusMessageFor(en)));
  });

  testWidgets(
      'recording and clear live in settings; clear requires confirmation and preserves recording',
      (tester) async {
    final backend = Preferences();
    final controller = ClipboardSettingsController(backend);
    await controller.load();
    await tester.pumpWidget(MaterialApp(
        theme: SettingsStyle.theme(AppStyle.theme),
        home: Scaffold(
            body: SingleChildScrollView(
                child: ClipboardSettingsSection(
                    controller: controller,
                    autoFill: true,
                    onAutoFill: (_) {})))));
    await tester.tap(find.text('Record clipboard history'));
    await tester.pumpAndSettle();
    expect(backend.recording, isTrue);
    await tester.ensureVisible(find.text('History storage'));
    await tester.tap(find.text('History storage'));
    await tester.pumpAndSettle();
    await tester.ensureVisible(find.text('Clear History…'));
    await tester.tap(find.text('Clear History…'));
    await tester.pumpAndSettle();
    expect(backend.cleared, 0);
    await tester.tap(find.text('Cancel'));
    await tester.pumpAndSettle();
    expect(backend.cleared, 0);
    await tester.tap(find.text('Clear History…'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Clear History'));
    await tester.pumpAndSettle();
    expect(backend.cleared, 1);
    expect(backend.recording, isTrue);
    await tester.pumpWidget(const SizedBox.shrink());
    controller.dispose();
  });
}
