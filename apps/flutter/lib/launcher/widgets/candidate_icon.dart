import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';

import '../../src/rust/api.dart';
import '../../ui/action_icon_appearance.dart';
import '../../ui/file_type_appearance.dart';
import '../../ui/semantic_icon.dart';

/// The same icon vocabulary is used by result rows and the selected-app hint.
class CandidateIcon extends StatelessWidget {
  const CandidateIcon(
      {super.key,
      required this.candidate,
      this.bytes,
      this.size = 36,
      this.fileObject = true});
  final CandidateDto candidate;
  final Uint8List? bytes;
  final double size;
  final bool fileObject;

  @override
  Widget build(BuildContext context) {
    final disabled = candidate.availability != AvailabilityDto.enabled;
    if (fileObject && candidate.kind == CandidateKindDto.file) {
      return FileTypeIcon(
          appearance: FileTypeAppearance.forName(candidate.title,
              isDirectory: candidate.fileKind == FileObjectKindDto.directory),
          size: size,
          disabled: disabled);
    }
    final appearance = ActionIconAppearance.forKey(candidate.iconKey,
        application: candidate.kind == CandidateKindDto.application);
    Widget fallback() => SemanticIcon(
        glyph: appearance.glyph,
        tint: appearance.tint,
        size: size,
        mark: appearance.mark,
        stamp: appearance.stamp,
        disabled: disabled);
    final pixels =
        (size * MediaQuery.devicePixelRatioOf(context)).ceil().clamp(28, 128);
    return SizedBox.square(
        dimension: size,
        child: bytes == null
            ? fallback()
            : ClipRRect(
                borderRadius: BorderRadius.circular(8),
                child: Image.memory(bytes!,
                    width: size,
                    height: size,
                    cacheWidth: pixels,
                    cacheHeight: pixels,
                    fit: BoxFit.contain,
                    gaplessPlayback: true,
                    errorBuilder: (_, __, ___) => fallback())));
  }
}
