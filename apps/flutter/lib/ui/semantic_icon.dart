import 'package:flutter/material.dart';
import 'app_style.dart';

/// Transparent object glyph plus a small operation/type mark. Marks convey
/// meaning without relying on tint, files, native APIs or a bitmap cache.
class SemanticIcon extends StatelessWidget {
  const SemanticIcon(
      {super.key,
      required this.glyph,
      required this.tint,
      this.size = 32,
      this.mark,
      this.stamp,
      this.disabled = false});
  final IconData glyph;
  final Color tint;
  final double size;
  final IconData? mark;
  final String? stamp;
  final bool disabled;

  @override
  Widget build(BuildContext context) {
    final color = disabled ? AppStyle.muted : tint;
    final marked = mark != null || stamp != null;
    return ExcludeSemantics(
        child: SizedBox.square(
            dimension: size,
            child: Stack(clipBehavior: Clip.none, children: [
              Align(
                  alignment: marked ? Alignment.topLeft : Alignment.center,
                  child: Icon(glyph,
                      size: size * (marked ? .84 : .82), color: color)),
              if (marked)
                Positioned(
                    right: 0,
                    bottom: size * .02,
                    child: DecoratedBox(
                        decoration: BoxDecoration(
                            color: color,
                            borderRadius: BorderRadius.circular(3)),
                        child: Padding(
                            padding: EdgeInsets.symmetric(
                                horizontal: stamp == null ? 2 : 3, vertical: 1),
                            child: stamp != null
                                ? Text(stamp!,
                                    textScaler: TextScaler.noScaling,
                                    style: TextStyle(
                                        fontSize: (size * .25).clamp(7.5, 11),
                                        height: 1.05,
                                        fontWeight: FontWeight.w800,
                                        color: DesignTokens.iconBadgeInk))
                                : Icon(mark,
                                    size: (size * .3).clamp(8, 14),
                                    color: DesignTokens.iconBadgeInk)))),
            ])));
  }
}
