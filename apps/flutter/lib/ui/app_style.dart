import 'package:flutter/material.dart';
import 'package:flutter/cupertino.dart';
import 'package:flutter/services.dart';
import '../infrastructure/desktop_environment.dart';
import '../infrastructure/desktop_edit_menu.dart';

/// One palette and control vocabulary for every window and sheet.
/// Native AppKit supplies the blur; content surfaces stabilize text contrast.
abstract final class AppStyle {
  // A neutral veil over the native blur softens bright backgrounds without
  // adding a second blur layer or replacing glass with a flat opaque panel.
  static const surface = Color(0xa0303438);
  static const overlay = Color(0xf22b2e35);
  static const menuSurface = Color(0xff2b2e35);
  static const actionSurface = Color(0x0cffffff);
  static const primaryButtonSurface = Color(0x242869bf);
  static const buttonSelection = Color(0x10ffffff);
  static const keycapSurface = Color(0x06ffffff);
  static const keycapBorder = Color(0x1affffff);
  static const card = Color(0x782a2e36);
  static const field = Color(0x8020232a);
  static const elevated = Color(0x584b505a);
  static const sidebar = Color(0x10101218);
  static const window = Color(0x1a12161e);
  static const sidebarSelection = Color(0x24ffffff);
  static const hover = Color(0x0cffffff);
  static const border = Color(0x38ffffff);
  static const divider = Color(0x1fffffff);
  // Local controls carry their own contrast; the window glass stays untouched.
  static const controlSurface = Color(0xff3d4046);
  static const controlSelection = Color(0xff56595f);
  static const controlBorder = Color(0x30ffffff);
  static const List<Shadow> textShadows = [];
  static const primary = Color(0xfff7f8fa);
  static const secondary = Color(0xffe0e3e9);
  static const muted = Color(0xffd6dbe2);
  static const accent = Color(0xffa7caff);
  static const accentFill = Color(0xff2869bf);
  static const warning = Color(0xffffc66d);
  static const danger = Color(0xffff8078);
  static const selectedSurface = Color(0x0cffffff);
  static const selectionOutline = Color(0x18ffffff);
  static const selectedBorder = Color(0xffb6c2d7);
  static const success = Color(0xff73cc91);
  static const radius = 18.0;
  static const resultRadius = 10.0;
  static const controlRadius = 8.0;
  static const cardRadius = 12.0;
  static const body = TextStyle(
      fontSize: 13, height: 1.25, color: primary, shadows: textShadows);
  static const caption =
      TextStyle(fontSize: 11, height: 1.3, color: muted, shadows: textShadows);
  static const heading = TextStyle(
      fontSize: 15,
      fontWeight: FontWeight.w600,
      color: primary,
      letterSpacing: 0,
      shadows: textShadows);

  static ShapeBorder get panelShape => RoundedRectangleBorder(
      borderRadius: BorderRadius.circular(radius),
      side: const BorderSide(color: divider, width: .5));

  /// Native glass remains fully visible. Only unsupported hosts and reduced
  /// transparency use the original opaque fallback.
  static Widget windowBuilder(BuildContext context, Widget? child) =>
      DesktopEditMenu(
          child: Shortcuts(
              shortcuts: const {
            SingleActivator(LogicalKeyboardKey.tab):
                DoNothingAndStopPropagationIntent(),
            SingleActivator(LogicalKeyboardKey.tab, shift: true):
                DoNothingAndStopPropagationIntent(),
          },
              child: ColoredBox(
                  color: DesktopEnvironment.current.hasBackdrop
                      ? Colors.transparent
                      : const Color(0xff24262b),
                  child: child ?? const SizedBox.shrink())));

  static ThemeData get theme {
    final shape = RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(controlRadius));
    final buttonOverlay = WidgetStateProperty.resolveWith<Color?>((states) {
      if (states.contains(WidgetState.disabled)) return Colors.transparent;
      if (states.contains(WidgetState.pressed)) return buttonSelection;
      if (states.contains(WidgetState.focused)) return buttonSelection;
      if (states.contains(WidgetState.hovered)) return hover;
      return Colors.transparent;
    });
    final buttonSide = WidgetStateProperty.resolveWith<BorderSide>((states) {
      if (states.contains(WidgetState.focused)) {
        return const BorderSide(color: selectedBorder, width: 1.2);
      }
      return BorderSide(
          color:
              states.contains(WidgetState.disabled) ? divider : controlBorder,
          width: .6);
    });
    final base = ThemeData(
      fontFamily: DesktopEnvironment.current.fontFamily,
      brightness: Brightness.dark,
      useMaterial3: true,
      cupertinoOverrideTheme: const CupertinoThemeData(
          brightness: Brightness.dark,
          primaryColor: accentFill,
          primaryContrastingColor: Colors.white),
      scaffoldBackgroundColor: Colors.transparent,
      canvasColor: overlay,
      splashFactory: NoSplash.splashFactory,
      hoverColor: hover,
      focusColor: buttonSelection,
      highlightColor: buttonSelection,
      colorScheme: const ColorScheme.dark(
          primary: accent,
          onPrimary: Colors.white,
          secondary: accent,
          surface: card,
          onSurface: primary,
          onSurfaceVariant: secondary,
          outline: border,
          outlineVariant: divider,
          error: danger),
    );
    return base.copyWith(
      textTheme: base.textTheme
          .copyWith(
              bodyLarge: body,
              bodyMedium: body,
              bodySmall: caption,
              titleLarge: heading,
              titleMedium: body.copyWith(fontWeight: FontWeight.w600),
              titleSmall:
                  caption.copyWith(fontWeight: FontWeight.w600, color: primary),
              labelLarge: body.copyWith(fontSize: 12),
              labelMedium: caption.copyWith(color: secondary),
              labelSmall: caption.copyWith(fontSize: 10))
          .apply(fontFamily: DesktopEnvironment.current.fontFamily),
      materialTapTargetSize: MaterialTapTargetSize.shrinkWrap,
      visualDensity: VisualDensity.standard,
      dividerTheme:
          const DividerThemeData(color: divider, thickness: .5, space: 1),
      inputDecorationTheme: InputDecorationTheme(
        filled: true,
        fillColor: field,
        isDense: true,
        hoverColor: Colors.transparent,
        floatingLabelBehavior: FloatingLabelBehavior.never,
        prefixIconColor: secondary,
        prefixIconConstraints:
            const BoxConstraints(minWidth: 28, minHeight: 28),
        hintStyle: caption.copyWith(fontSize: 12),
        labelStyle: const TextStyle(fontSize: 12, color: secondary),
        floatingLabelStyle: const TextStyle(fontSize: 12, color: accent),
        contentPadding: const EdgeInsets.symmetric(horizontal: 9, vertical: 7),
        border: OutlineInputBorder(
            borderRadius: BorderRadius.circular(controlRadius)),
        enabledBorder: OutlineInputBorder(
            borderRadius: BorderRadius.circular(controlRadius),
            borderSide: const BorderSide(color: border, width: .8)),
        focusedBorder: OutlineInputBorder(
            borderRadius: BorderRadius.circular(controlRadius),
            borderSide: const BorderSide(color: selectedBorder, width: 1.5)),
      ),
      textButtonTheme: TextButtonThemeData(
          style: TextButton.styleFrom(
                  foregroundColor: primary,
                  minimumSize: const Size(0, 24),
                  padding:
                      const EdgeInsets.symmetric(horizontal: 10, vertical: 3),
                  textStyle: TextStyle(
                      fontSize: 11,
                      fontWeight: FontWeight.w400,
                      fontFamily: DesktopEnvironment.current.fontFamily),
                  shape: shape)
              .copyWith(
                  overlayColor: buttonOverlay,
                  side: WidgetStateProperty.resolveWith((states) =>
                      states.contains(WidgetState.focused)
                          ? const BorderSide(color: selectedBorder, width: 1.2)
                          : BorderSide.none))),
      outlinedButtonTheme: OutlinedButtonThemeData(
          style: OutlinedButton.styleFrom(
                  foregroundColor: primary,
                  backgroundColor: Colors.transparent,
                  minimumSize: const Size(0, 24),
                  padding:
                      const EdgeInsets.symmetric(horizontal: 10, vertical: 3),
                  side: const BorderSide(color: border, width: .8),
                  textStyle: TextStyle(
                      fontSize: 11,
                      fontWeight: FontWeight.w400,
                      fontFamily: DesktopEnvironment.current.fontFamily),
                  shape: shape)
              .copyWith(overlayColor: buttonOverlay, side: buttonSide)),
      filledButtonTheme: FilledButtonThemeData(
          style: FilledButton.styleFrom(
                  backgroundColor: primaryButtonSurface,
                  disabledBackgroundColor: Colors.transparent,
                  foregroundColor: Colors.white,
                  minimumSize: const Size(0, 24),
                  padding:
                      const EdgeInsets.symmetric(horizontal: 10, vertical: 3),
                  textStyle: TextStyle(
                      fontSize: 11,
                      fontWeight: FontWeight.w500,
                      fontFamily: DesktopEnvironment.current.fontFamily),
                  shape: shape)
              .copyWith(overlayColor: buttonOverlay, side: buttonSide)),
      iconButtonTheme: IconButtonThemeData(
          style: IconButton.styleFrom(
                  foregroundColor: secondary,
                  iconSize: 16,
                  minimumSize: const Size(26, 26),
                  shape: shape)
              .copyWith(
                  overlayColor: buttonOverlay,
                  side: WidgetStateProperty.resolveWith((states) =>
                      states.contains(WidgetState.focused)
                          ? const BorderSide(color: selectedBorder, width: 1.2)
                          : BorderSide.none))),
      dialogTheme: DialogThemeData(
          backgroundColor: overlay,
          surfaceTintColor: Colors.transparent,
          barrierColor: const Color(0x66000000),
          shape: RoundedRectangleBorder(
              borderRadius: BorderRadius.circular(12),
              side: const BorderSide(color: divider, width: .5)),
          titleTextStyle: heading,
          contentTextStyle: body.copyWith(fontSize: 12, color: secondary)),
      listTileTheme: const ListTileThemeData(
          textColor: primary,
          iconColor: secondary,
          selectedColor: primary,
          selectedTileColor: selectedSurface,
          titleTextStyle: TextStyle(fontSize: 13, fontWeight: FontWeight.w400),
          subtitleTextStyle: caption),
      switchTheme: SwitchThemeData(
          thumbColor: WidgetStateProperty.resolveWith((states) =>
              states.contains(WidgetState.selected) ? Colors.white : secondary),
          trackColor: WidgetStateProperty.resolveWith((states) =>
              states.contains(WidgetState.selected) ? accentFill : elevated)),
      popupMenuTheme: PopupMenuThemeData(
          color: menuSurface,
          elevation: 3,
          shadowColor: const Color(0x40000000),
          surfaceTintColor: Colors.transparent,
          textStyle: body.copyWith(
              fontSize: 12, fontFamily: DesktopEnvironment.current.fontFamily),
          labelTextStyle: WidgetStatePropertyAll(body.copyWith(
              fontSize: 12,
              color: primary,
              fontFamily: DesktopEnvironment.current.fontFamily)),
          shape: RoundedRectangleBorder(
              borderRadius: BorderRadius.circular(controlRadius),
              side: const BorderSide(color: controlBorder, width: .5))),
      tooltipTheme: TooltipThemeData(
          decoration: BoxDecoration(
              color: overlay,
              borderRadius: BorderRadius.circular(6),
              border: Border.all(color: border)),
          textStyle: caption.copyWith(color: primary),
          waitDuration: const Duration(milliseconds: 450)),
      snackBarTheme: SnackBarThemeData(
          backgroundColor: overlay,
          contentTextStyle: body.copyWith(fontSize: 12),
          behavior: SnackBarBehavior.floating,
          shape: shape),
      scrollbarTheme: ScrollbarThemeData(
          thickness: WidgetStateProperty.all(5),
          radius: const Radius.circular(5)),
      expansionTileTheme: const ExpansionTileThemeData(
          iconColor: secondary,
          collapsedIconColor: muted,
          backgroundColor: Colors.transparent,
          collapsedBackgroundColor: Colors.transparent),
    );
  }
}
