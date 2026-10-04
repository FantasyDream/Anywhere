import 'package:flutter/material.dart';

/// Colors shared by the launcher, settings and task window. The window
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
  static const selectedSurface = Color(0x44778fe0);
  static const selectedBorder = Color(0x779eb4ff);
  static const success = Color(0xffa2d6bb);
  static const radius = 18.0;

  static ThemeData get theme => ThemeData(
        brightness: Brightness.dark,
        useMaterial3: true,
        colorScheme: ColorScheme.fromSeed(
          seedColor: const Color(0xff8ea2ff),
          brightness: Brightness.dark,
        ),
        scaffoldBackgroundColor: Colors.transparent,
        fontFamily: '.AppleSystemUIFont',
      );

  static ShapeBorder get panelShape => RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(radius),
        side: const BorderSide(color: border, width: 0.8),
      );
}
