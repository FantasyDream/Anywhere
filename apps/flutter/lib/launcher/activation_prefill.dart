import 'dart:async';
import '../src/rust/api.dart';

/// Rust supplies eligibility, priority and expiry. The Host only decides whether
/// user interaction still permits inserting the recommendation.
typedef ChooseActivationInput = FutureOr<ActivationInputDto?> Function(
    List<ContextCaptureDto> captures, int nowMs,
    {required bool selectedEnabled,
    required bool clipboardEnabled,
    required bool allowPendingSelection});

class ActivationPrefill {
  const ActivationPrefill(this.source, this.text, this.validUntilMs);
  final ContextSourceDto source;
  final String text;
  final int? validUntilMs;
  static Future<ActivationPrefill?> choose(
      List<ContextCaptureDto> captures, int nowMs,
      {required ChooseActivationInput choose,
      required bool selectedEnabled,
      required bool clipboardEnabled,
      bool allowPendingSelection = false}) async {
    final input = await choose(captures, nowMs,
        selectedEnabled: selectedEnabled,
        clipboardEnabled: clipboardEnabled,
        allowPendingSelection: allowPendingSelection);
    return input == null
        ? null
        : ActivationPrefill(input.source, input.text, input.validUntilMs);
  }
}
