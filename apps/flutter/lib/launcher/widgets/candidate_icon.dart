import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import '../../src/rust/api.dart';
import '../../ui/app_icons.dart';
import '../../ui/app_style.dart';

/// The same icon vocabulary is used by result rows and the selected-app hint.
class CandidateIcon extends StatelessWidget {
  const CandidateIcon(
      {super.key, required this.candidate, this.bytes, this.size = 36});
  final CandidateDto candidate;
  final Uint8List? bytes;
  final double size;

  @override
  Widget build(BuildContext context) {
    final disabled = candidate.availability != AvailabilityDto.enabled;
    final (glyph, tint) = switch (candidate.iconKey) {
      'terminal' => (AppIcons.terminal, const Color(0xffa0ceb9)),
      'folder' => (AppIcons.folder, const Color(0xff92c9f6)),
      'file' => (AppIcons.file, const Color(0xffd9e1ec)),
      'copy' => (AppIcons.copy, const Color(0xffd9e1ec)),
      'globe' => (AppIcons.globe, const Color(0xffa1c9e8)),
      'code' => (AppIcons.code, const Color(0xffc3b4e4)),
      'git' => (AppIcons.git, const Color(0xffe9bd9c)),
      'search' => (AppIcons.search, AppStyle.secondary),
      'app' => (AppIcons.external, AppStyle.secondary),
      _ => (
          candidate.kind == CandidateKindDto.application
              ? AppIcons.apps
              : AppIcons.bolt,
          AppStyle.secondary
        ),
    };
    Widget fallback() =>
        Icon(glyph, size: size * .66, color: disabled ? AppStyle.muted : tint);
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
