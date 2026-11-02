import 'dart:async';

import 'package:anywhere/infrastructure/web_package_backend.dart';
import 'package:anywhere/src/rust/api.dart';
import 'package:anywhere/ui/app_components.dart';
import 'package:anywhere/ui/app_style.dart';
import 'package:anywhere/web_plugins/web_packages_section.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

WebPackageDto package(String name,
        {bool enabled = true, bool available = true}) =>
    WebPackageDto(
        available: available,
        id: 'dev.example.tool',
        name: name,
        version: '1.0.0',
        digest: 'digest',
        permissions: [],
        enabled: enabled,
        rollback: false);

class Backend extends WebPackageBackend {
  final initial = Completer<List<WebPackageDto>>();
  final picker = Completer<String?>();
  int picks = 0;
  @override
  Future<List<WebPackageDto>> list() => initial.future;
  @override
  Future<String?> choose() {
    picks++;
    return picker.future;
  }

  @override
  Future<WebReviewDto> inspect(String path) async => const WebReviewDto(
      token: 'review',
      id: 'dev.example.tool',
      name: 'Tool',
      version: '1.0.0',
      digest: 'digest',
      permissions: [],
      updating: false);
  @override
  Future<List<WebPackageDto>> approve(String token) async =>
      [package('Installed')];
  @override
  Future<void> cancelReview(String token) async =>
      throw StateError('already expired');
}

class PartialRemovalBackend extends Backend {
  bool removed = false;
  @override
  Future<List<WebPackageDto>> list() async => removed ? [] : [package('Tool')];
  @override
  Future<List<WebPackageDto>> change(String id, String operation) async {
    removed = true;
    throw StateError('private data cleanup failed');
  }
}

class FailedApprovalBackend extends Backend {
  bool attempted = false;
  @override
  Future<List<WebPackageDto>> approve(String token) async {
    attempted = true;
    throw StateError('approval response unavailable');
  }

  @override
  Future<List<WebPackageDto>> list() =>
      attempted ? Future.value([package('Authoritative tool')]) : super.list();
}

class ReviewBackend extends WebPackageBackend {
  ReviewBackend(this.token, {this.held});
  final String token;
  final Completer<WebReviewDto>? held;
  final cancelled = <String>[];
  final approved = <String>[];
  bool inspecting = false;
  WebReviewDto get review => WebReviewDto(
      token: token,
      id: 'dev.example.tool',
      name: 'Tool',
      version: '1.0.0',
      digest: 'digest',
      permissions: [],
      updating: false);
  @override
  Future<List<WebPackageDto>> list() async => [];
  @override
  Future<String?> choose() async => '/test/tool.awplugin';
  @override
  Future<WebReviewDto> inspect(String path) {
    inspecting = true;
    return held?.future ?? Future.value(review);
  }

  @override
  Future<void> cancelReview(String token) async {
    cancelled.add(token);
  }

  @override
  Future<List<WebPackageDto>> approve(String token) async {
    approved.add(token);
    return [package('Installed')];
  }
}

Widget page(WebPackageBackend backend, {Key? key}) => MaterialApp(
    theme: AppStyle.theme,
    home: Scaffold(
        body: SingleChildScrollView(
            child: WebPackagesSection(key: key, backend: backend))));
void main() {
  testWidgets('an unmounted importer cannot cancel a newer page review',
      (tester) async {
    final held = Completer<WebReviewDto>();
    final old = ReviewBackend('old', held: held);
    final next = ReviewBackend('new');
    await tester.pumpWidget(page(old, key: const ValueKey('old')));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Import…'));
    await tester.pump();
    expect(old.inspecting, isTrue);
    await tester.pumpWidget(page(next, key: const ValueKey('new')));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Import…'));
    await tester.pumpAndSettle();
    held.complete(old.review);
    await tester.pumpAndSettle();
    expect(old.cancelled, ['old']);
    expect(next.cancelled, isEmpty);
    await tester.tap(find.text('Approve and install'));
    await tester.pumpAndSettle();
    expect(next.approved, ['new']);
    expect(next.cancelled, ['new']);
    expect(find.text('Installed'), findsOneWidget);
    expect(tester.takeException(), isNull);
  });

  testWidgets('declining approval cancels the current review', (tester) async {
    final backend = ReviewBackend('current');
    await tester.pumpWidget(page(backend));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Import…'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Cancel'));
    await tester.pumpAndSettle();
    expect(backend.cancelled, ['current']);
    expect(backend.approved, isEmpty);
  });

  for (final review in [false, true]) {
    testWidgets(
        'canceling ${review ? 'approval' : 'the picker'} preserves a late initial package list',
        (tester) async {
      final backend = Backend();
      await tester.pumpWidget(page(backend));
      await tester.tap(find.text('Import…'));
      await tester.pump();
      backend.picker.complete(review ? '/test/tool.awplugin' : null);
      await tester.pumpAndSettle();
      if (review) {
        await tester.tap(find.text('Cancel'));
        await tester.pumpAndSettle();
      }
      backend.initial.complete([package('Existing tool')]);
      await tester.pumpAndSettle();
      expect(find.text('Existing tool'), findsOneWidget);
      expect(tester.takeException(), isNull);
    });
  }

  testWidgets(
      'failed approval reloads status and still rejects the stale initial list',
      (tester) async {
    final backend = FailedApprovalBackend();
    await tester.pumpWidget(page(backend));
    await tester.tap(find.text('Import…'));
    await tester.pump();
    backend.picker.complete('/test/tool.awplugin');
    await tester.pumpAndSettle();
    await tester.tap(find.text('Approve and install'));
    await tester.pumpAndSettle();
    expect(find.text('Authoritative tool'), findsOneWidget);
    backend.initial.complete([package('Old snapshot')]);
    await tester.pumpAndSettle();
    expect(find.text('Old snapshot'), findsNothing);
    expect(find.text('Authoritative tool'), findsOneWidget);
    expect(tester.takeException(), isNull);
  });

  testWidgets('damaged package exposes recovery and cannot be enabled',
      (tester) async {
    final backend = Backend();
    backend.initial
        .complete([package('Damaged', enabled: false, available: false)]);
    await tester.pumpWidget(page(backend));
    await tester.pumpAndSettle();
    expect(
        find.textContaining('Package files are unavailable'), findsOneWidget);
    expect(find.text('Uninstall'), findsOneWidget);
    expect(tester.widget<AppToggle>(find.byType(AppToggle)).onChanged, isNull);
  });

  testWidgets(
      'long plugin names keep enable and uninstall reachable in narrow settings',
      (tester) async {
    tester.view.physicalSize = const Size(320, 500);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    final backend = Backend();
    backend.initial.complete([
      package('A very long workspace tool name that wraps across several lines')
    ]);
    await tester.pumpWidget(page(backend));
    await tester.pumpAndSettle();
    expect(tester.takeException(), isNull);
    expect(find.text('Uninstall'), findsNothing);
    await tester.tap(find.text(
        'A very long workspace tool name that wraps across several lines'));
    await tester.pumpAndSettle();
    expect(find.text('Uninstall'), findsOneWidget);
    expect(find.byType(Switch), findsNothing);
  });

  testWidgets('partially completed removal reloads authoritative status',
      (tester) async {
    final backend = PartialRemovalBackend();
    await tester.pumpWidget(page(backend));
    await tester.pumpAndSettle();
    expect(find.text('Tool'), findsOneWidget);
    await tester.tap(find.text('Tool'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Uninstall'));
    await tester.pumpAndSettle();
    expect(find.text('Tool'), findsNothing);
    expect(find.textContaining('operation could not finish'), findsOneWidget);
    expect(tester.takeException(), isNull);
  });

  testWidgets('late initial refresh cannot replace a completed installation',
      (tester) async {
    final backend = Backend();
    await tester.pumpWidget(page(backend));
    await tester.tap(find.text('Import…'));
    await tester.pump();
    expect(
        tester
            .widget<TextButton>(find.widgetWithText(TextButton, 'Import…'))
            .onPressed,
        isNull);
    backend.picker.complete('/test/tool.awplugin');
    await tester.pumpAndSettle();
    await tester.tap(find.text('Approve and install'));
    await tester.pumpAndSettle();
    expect(find.text('Installed'), findsOneWidget);
    expect(tester.takeException(), isNull); // Cleanup failure is contained.
    backend.initial.complete([package('Old snapshot')]);
    await tester.pumpAndSettle();
    expect(find.text('Installed'), findsOneWidget);
    expect(find.text('Old snapshot'), findsNothing);
    expect(
        tester
            .widget<TextButton>(find.widgetWithText(TextButton, 'Import…'))
            .onPressed,
        isNotNull);
  });
  testWidgets(
      'canceling the picker restores controls without inspecting a package',
      (tester) async {
    final backend = Backend();
    backend.initial.complete([]);
    await tester.pumpWidget(page(backend));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Import…'));
    await tester.pump();
    expect(backend.picks, 1);
    backend.picker.complete(null);
    await tester.pumpAndSettle();
    expect(
        tester
            .widget<TextButton>(find.widgetWithText(TextButton, 'Import…'))
            .onPressed,
        isNotNull);
    expect(tester.takeException(), isNull);
  });
}
