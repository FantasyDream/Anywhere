import 'package:anywhere/infrastructure/presentation_metrics.dart';
import 'package:anywhere/infrastructure/presentation_metrics_transport.dart';
import 'package:anywhere/infrastructure/runtime_paths.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  const channel = MethodChannel('test/performance-transport');
  final messenger =
      TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger;
  tearDown(() {
    PresentationMetrics.reporter = null;
    messenger.setMockMethodCallHandler(channel, null);
  });

  testWidgets('each engine opts in explicitly and missing hosts are harmless',
      (tester) async {
    final calls = <MethodCall>[];
    messenger.setMockMethodCallHandler(channel, (call) async {
      calls.add(call);
      throw PlatformException(code: 'older-host');
    });
    configurePresentationMetrics(
        const RuntimePaths('library', {'profileUi': '1'}), channel);
    PresentationMetrics.reporter!({'operation': 'shelf_query', 'rows': 0});
    await tester.pump();
    expect(calls.single.method, 'performanceSample');
    expect(tester.takeException(), isNull);
    configurePresentationMetrics(const RuntimePaths('library', {}), channel);
    expect(PresentationMetrics.reporter, isNull);
    configurePresentationMetrics(
        const RuntimePaths('library', {'profileUi': '0'}), channel);
    expect(PresentationMetrics.reporter, isNull);
  });
}
