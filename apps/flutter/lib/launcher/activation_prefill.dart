import '../src/rust/api.dart';

class ActivationPrefill {
  const ActivationPrefill(this.source, this.text);
  final ContextSourceDto source;
  final String text;
  static const clipboardLifetime = Duration(seconds: 15);

  static ActivationPrefill? choose(List<ContextCaptureDto> captures, int nowMs,
      {required bool selectedEnabled,
      required bool clipboardEnabled,
      bool allowPendingSelection = false}) {
    final selected = captures
        .where((c) => c.source == ContextSourceDto.selectedText)
        .firstOrNull;
    if (selectedEnabled) {
      if (selected == null && !allowPendingSelection) {
        return null;
      }
      if (selected?.status == ContextCaptureStatusDto.captured &&
          (selected?.text?.trim().isNotEmpty ?? false)) {
        return ActivationPrefill(
            ContextSourceDto.selectedText, selected!.text!);
      }
    }
    final clipboard = captures
        .where((c) => c.source == ContextSourceDto.clipboard)
        .firstOrNull;
    if (!clipboardEnabled ||
        clipboard?.status != ContextCaptureStatusDto.captured ||
        clipboard?.clipboardAgeMs == null ||
        !(clipboard?.text?.trim().isNotEmpty ?? false)) {
      return null;
    }
    final age =
        nowMs - clipboard!.capturedAtMs + clipboard.clipboardAgeMs!.toInt();
    if (age < 0 || age > clipboardLifetime.inMilliseconds) return null;
    return ActivationPrefill(ContextSourceDto.clipboard, clipboard.text!);
  }
}
