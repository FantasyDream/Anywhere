import 'package:flutter/material.dart';

/// Colors shared by the compact launcher and its settings sheet. The window
/// underneath is an AppKit material, so keep this surface translucent.
abstract final class LauncherStyle {
  static const surface = Color(0x7810141d);
  static const border = Color(0x46ffffff);
  static const divider = Color(0x20ffffff);
  static const primary = Color(0xfff5f7fc);
  static const secondary = Color(0xffbdc7d8);
  static const muted = Color(0xff929eb1);
  static const accent = Color(0xffb5c5ff);
  static const warning = Color(0xfff1c995);
  static const radius = 18.0;

  static ShapeBorder get panelShape => RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(radius),
        side: const BorderSide(color: border, width: 0.8),
      );
}
