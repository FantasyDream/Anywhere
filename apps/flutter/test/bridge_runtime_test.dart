import 'package:anywhere/infrastructure/bridge_api.dart' as api;
import 'package:anywhere/src/rust/frb_generated.dart';
import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated.dart';
import 'package:flutter_test/flutter_test.dart';

/// Explicitly supplied compiled library; ordinary widget suites keep using mocks.
/// This checks actual Dart/runtime/generated-code compatibility without opening
/// windows or initializing storage, permissions or clipboard collection.
void main() {
  const library = String.fromEnvironment('ANYWHERE_BRIDGE_LIBRARY');
  test('compiled bridge loads and answers through the generated Dart API',
      () async {
    await RustLib.init(externalLibrary: ExternalLibrary.open(library));
    try {
      expect(await api.ping(), 'pong');
      // Exercise moved DTOs through the native codec without initializing the
      // application. No captures are read; the uninitialized policy offers none.
      expect(
        await api.chooseActivationInput(
          captures: [
            api.ContextCaptureDto(
              text: 'https://example.org/合成-🧪',
              clipboardAgeMs: BigInt.from(12),
              source: api.ContextSourceDto.clipboard,
              status: api.ContextCaptureStatusDto.captured,
              capturedAtMs: 1000,
            ),
            const api.ContextCaptureDto(
              source: api.ContextSourceDto.selectedText,
              status: api.ContextCaptureStatusDto.permissionDenied,
              capturedAtMs: 1000,
            ),
          ],
          nowMs: 1010,
          selectedEnabled: true,
          clipboardEnabled: true,
          allowPendingSelection: false,
        ),
        isNull,
      );
    } finally {
      RustLib.dispose();
    }
  },
      skip: library.isEmpty
          ? 'Supply ANYWHERE_BRIDGE_LIBRARY for native FFI verification'
          : false);
}
