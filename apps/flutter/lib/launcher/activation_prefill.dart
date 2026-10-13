import '../src/rust/api.dart';

/// Rust supplies eligibility, priority and expiry. The Host only decides whether
/// user interaction still permits inserting the recommendation.
typedef ChooseActivationInput = ActivationInputDto? Function(
    List<ContextCaptureDto> captures, int nowMs,
    {required bool selectedEnabled,
    required bool clipboardEnabled,
    required bool allowPendingSelection});

class ActivationPrefill {
  const ActivationPrefill(this.source, this.text);
  final ContextSourceDto source;
  final String text;
  static ActivationPrefill? choose(List<ContextCaptureDto> captures, int nowMs,
      {required ChooseActivationInput choose,
      required bool selectedEnabled,
      required bool clipboardEnabled,
      bool allowPendingSelection = false}) {
    final input = choose(captures, nowMs,
        selectedEnabled: selectedEnabled,
        clipboardEnabled: clipboardEnabled,
        allowPendingSelection: allowPendingSelection);
    return input == null ? null : ActivationPrefill(input.source, input.text);
  }
}
