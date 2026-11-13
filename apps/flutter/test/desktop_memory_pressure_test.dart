import 'package:anywhere/infrastructure/desktop_memory_pressure.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  const channel = MethodChannel('test/memory-pressure');
  final messenger =
      TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger;

  test('bounded native pressure events notify and malformed data is ignored',
      () async {
    final pressure = DesktopMemoryPressure(channel: channel);
    addTearDown(pressure.dispose);
    var notifications = 0;
    pressure
      ..attach()
      ..addListener(() => notifications++);

    await messenger.handlePlatformMessage(
        channel.name,
        const StandardMethodCodec()
            .encodeMethodCall(const MethodCall('memoryPressure', 'warning')),
        (_) {});
    expect(pressure.level, DesktopMemoryPressureLevel.warning);
    expect(notifications, 1);

    await messenger.handlePlatformMessage(
        channel.name,
        const StandardMethodCodec()
            .encodeMethodCall(const MethodCall('memoryPressure', 'critical')),
        (_) {});
    expect(pressure.level, DesktopMemoryPressureLevel.critical);
    expect(notifications, 2);

    for (final arguments in <Object?>['unknown', 1, null]) {
      await messenger.handlePlatformMessage(
          channel.name,
          StandardMethodCodec()
              .encodeMethodCall(MethodCall('memoryPressure', arguments)),
          (_) {});
    }
    expect(pressure.level, DesktopMemoryPressureLevel.critical);
    expect(notifications, 2);
  });

  test('detaching removes the native handler', () async {
    final pressure = DesktopMemoryPressure(channel: channel);
    pressure.attach();
    pressure.dispose();
    await messenger.handlePlatformMessage(
        channel.name,
        const StandardMethodCodec()
            .encodeMethodCall(const MethodCall('memoryPressure', 'warning')),
        (_) {});
    expect(pressure.level, isNull);
  });
}
