import 'package:flutter/cupertino.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import '../infrastructure/desktop_edit_menu.dart';
import '../infrastructure/desktop_environment.dart';
import 'design_tokens.g.dart';

export 'design_tokens.g.dart';

/// One palette and control vocabulary for every window and sheet.
/// Native AppKit supplies the blur; content surfaces stabilize text contrast.
abstract final class AppStyle {
  // A neutral veil over the native blur softens bright backgrounds without
  // adding a second blur layer or replacing glass with a flat opaque panel.
  static const surface = DesignTokens.surface;
  static const overlay = DesignTokens.overlay;
  static const menuSurface = DesignTokens.menuSurface;
  static const actionSurface = DesignTokens.actionSurface;
  static const primaryButtonSurface = DesignTokens.primaryButtonSurface;
  static const buttonSelection = DesignTokens.buttonSelection;
  static const keycapSurface = DesignTokens.keycapSurface;
  static const keycapBorder = DesignTokens.keycapBorder;
  static const card = DesignTokens.card;
  static const field = DesignTokens.field;
  static const elevated = DesignTokens.elevated;
  static const sidebar = DesignTokens.sidebar;
  static const window = DesignTokens.window;
  static const sidebarSelection = DesignTokens.sidebarSelection;
  static const hover = DesignTokens.hover;
  static const border = DesignTokens.border;
  static const divider = DesignTokens.divider;
  // Local controls carry their own contrast; the window glass stays untouched.
  static const controlSurface = DesignTokens.controlSurface;
  static const controlSelection = DesignTokens.controlSelection;
  static const controlBorder = DesignTokens.controlBorder;
  static const List<Shadow> textShadows = [];
  static const primary = DesignTokens.primary;
  static const secondary = DesignTokens.secondary;
  static const muted = DesignTokens.muted;
  static const accent = DesignTokens.accent;
  static const accentFill = DesignTokens.accentFill;
  static const warning = DesignTokens.warning;
  static const danger = DesignTokens.danger;
  static const selectedSurface = DesignTokens.selectedSurface;
  static const selectionOutline = DesignTokens.selectionOutline;
  static const selectedBorder = DesignTokens.selectedBorder;
  static const success = DesignTokens.success;
  static const radius = DesignTokens.radius;
  static const resultRadius = DesignTokens.resultRadius;
  static const controlRadius = DesignTokens.controlRadius;
  static const cardRadius = DesignTokens.cardRadius;
  static const body = TextStyle(
      fontSize: DesignTokens.bodySize,
      height: DesignTokens.bodyLineHeight,
      fontWeight: DesignTokens.bodyWeight,
      color: primary,
      shadows: textShadows);
  static const caption = TextStyle(
      fontSize: DesignTokens.captionSize,
      height: DesignTokens.captionLineHeight,
      color: muted,
      shadows: textShadows);
  static const heading = TextStyle(
      fontSize: DesignTokens.headingSize,
      fontWeight: DesignTokens.headingWeight,
      color: primary,
      letterSpacing: 0,
      shadows: textShadows);
  static const paneTitle = TextStyle(
      fontSize: DesignTokens.labelSize,
      height: DesignTokens.bodyLineHeight,
      fontWeight: DesignTokens.primaryControlWeight,
      color: primary);

  static final pageHeading =
      body.copyWith(fontSize: DesignTokens.pageHeadingSize);
  static final pageMode = caption.copyWith(
      fontSize: DesignTokens.pageModeSize,
      fontWeight: DesignTokens.pageModeWeight);
  static final contentInput =
      body.copyWith(fontSize: DesignTokens.contentInputSize);
  static TextStyle get code => body.copyWith(
      fontFamily: DesktopEnvironment.current.monoFontFamily,
      fontFamilyFallback: const ['monospace'],
      fontSize: DesignTokens.previewCodeSize,
      height: DesignTokens.previewCodeLineHeight);

  /// Search headers are the deliberate borderless variant of default form fields.
  static InputDecoration searchDecoration(String hint,
          {TextStyle? style, bool collapsed = true}) =>
      InputDecoration(
          hintText: hint,
          hintStyle: (style ?? pageHeading).copyWith(color: muted),
          filled: false,
          isDense: true,
          isCollapsed: collapsed,
          contentPadding: EdgeInsets.zero,
          border: InputBorder.none,
          enabledBorder: InputBorder.none,
          focusedBorder: InputBorder.none);

  static ShapeBorder get panelShape => RoundedRectangleBorder(
      borderRadius: BorderRadius.circular(radius),
      side: const BorderSide(color: divider, width: DesignTokens.dividerWidth));

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
                      : DesignTokens.fallbackSurface,
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
        return const BorderSide(
            color: selectedBorder, width: DesignTokens.focusBorderWidth);
      }
      return BorderSide(
          color:
              states.contains(WidgetState.disabled) ? divider : controlBorder,
          width: DesignTokens.controlBorderWidth);
    });
    final controlText = TextStyle(
        fontSize: DesignTokens.controlFontSize,
        fontWeight: DesignTokens.controlWeight,
        fontFamily: DesktopEnvironment.current.fontFamily);
    final control = TextButton.styleFrom(
            foregroundColor: primary,
            disabledForegroundColor:
                primary.withValues(alpha: DesignTokens.disabledOpacity),
            minimumSize: const Size(0, DesignTokens.controlHeight),
            padding: const EdgeInsets.symmetric(
                horizontal: DesignTokens.controlPaddingX,
                vertical: DesignTokens.controlPaddingY),
            textStyle: controlText,
            shape: shape)
        .copyWith(overlayColor: buttonOverlay);
    final outlinedControl = control.copyWith(
        backgroundColor: const WidgetStatePropertyAll(Colors.transparent),
        side: buttonSide);
    final primaryControl = outlinedControl.copyWith(
        backgroundColor: WidgetStateProperty.resolveWith((states) =>
            states.contains(WidgetState.disabled)
                ? Colors.transparent
                : primaryButtonSurface),
        foregroundColor: WidgetStateProperty.resolveWith((states) =>
            states.contains(WidgetState.disabled)
                ? primary.withValues(alpha: DesignTokens.disabledOpacity)
                : Colors.white),
        textStyle: WidgetStatePropertyAll(controlText.copyWith(
            fontWeight: DesignTokens.primaryControlWeight)));
    final menu = MenuStyle(
        backgroundColor: const WidgetStatePropertyAll(menuSurface),
        surfaceTintColor: const WidgetStatePropertyAll(Colors.transparent),
        shape: WidgetStatePropertyAll(shape),
        side: const WidgetStatePropertyAll(BorderSide(
            color: controlBorder, width: DesignTokens.dividerWidth)));
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
      iconTheme:
          const IconThemeData(color: secondary, size: DesignTokens.iconSize),
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
    final textTheme = base.textTheme
        .copyWith(
            bodyLarge: body,
            bodyMedium: body,
            bodySmall: caption,
            titleLarge: heading,
            headlineSmall: pageHeading,
            titleMedium: body.copyWith(fontWeight: DesignTokens.headingWeight),
            titleSmall: caption.copyWith(
                fontWeight: DesignTokens.headingWeight, color: primary),
            labelLarge: body.copyWith(fontSize: DesignTokens.labelSize),
            labelMedium: caption.copyWith(color: secondary),
            labelSmall: caption.copyWith(fontSize: 10))
        .apply(fontFamily: DesktopEnvironment.current.fontFamily);
    return base.copyWith(
      textTheme: textTheme,
      materialTapTargetSize: MaterialTapTargetSize.shrinkWrap,
      visualDensity: VisualDensity.standard,
      dividerTheme: const DividerThemeData(
          color: divider, thickness: DesignTokens.dividerWidth, space: 1),
      inputDecorationTheme: InputDecorationTheme(
        filled: true,
        fillColor: field,
        isDense: true,
        hoverColor: Colors.transparent,
        floatingLabelBehavior: FloatingLabelBehavior.never,
        prefixIconColor: secondary,
        prefixIconConstraints:
            const BoxConstraints(minWidth: 28, minHeight: 28),
        hintStyle: caption.copyWith(fontSize: DesignTokens.labelSize),
        labelStyle:
            const TextStyle(fontSize: DesignTokens.labelSize, color: secondary),
        floatingLabelStyle:
            const TextStyle(fontSize: DesignTokens.labelSize, color: accent),
        contentPadding: const EdgeInsets.symmetric(
            horizontal: DesignTokens.fieldPaddingX,
            vertical: DesignTokens.fieldPaddingY),
        border: OutlineInputBorder(
            borderRadius: BorderRadius.circular(controlRadius)),
        enabledBorder: OutlineInputBorder(
            borderRadius: BorderRadius.circular(controlRadius),
            borderSide: const BorderSide(
                color: border, width: DesignTokens.fieldBorderWidth)),
        focusedBorder: OutlineInputBorder(
            borderRadius: BorderRadius.circular(controlRadius),
            borderSide: const BorderSide(
                color: selectedBorder,
                width: DesignTokens.fieldFocusBorderWidth)),
      ),
      textSelectionTheme: const TextSelectionThemeData(
          cursorColor: accent,
          selectionColor: DesignTokens.textSelection,
          selectionHandleColor: accent),
      textButtonTheme: TextButtonThemeData(
          style: control.copyWith(
              side: WidgetStateProperty.resolveWith((states) =>
                  states.contains(WidgetState.focused)
                      ? const BorderSide(
                          color: selectedBorder,
                          width: DesignTokens.focusBorderWidth)
                      : BorderSide.none))),
      outlinedButtonTheme: OutlinedButtonThemeData(style: outlinedControl),
      filledButtonTheme: FilledButtonThemeData(style: primaryControl),
      elevatedButtonTheme: ElevatedButtonThemeData(
          style: primaryControl.copyWith(
              elevation: const WidgetStatePropertyAll(0),
              surfaceTintColor:
                  const WidgetStatePropertyAll(Colors.transparent),
              shadowColor: const WidgetStatePropertyAll(Colors.transparent))),
      cardTheme: CardThemeData(
          color: card,
          elevation: 0,
          surfaceTintColor: Colors.transparent,
          margin: EdgeInsets.zero,
          clipBehavior: Clip.antiAlias,
          shape: RoundedRectangleBorder(
              borderRadius: BorderRadius.circular(cardRadius),
              side: const BorderSide(
                  color: divider, width: DesignTokens.dividerWidth))),
      appBarTheme: AppBarThemeData(
          backgroundColor: Colors.transparent,
          foregroundColor: primary,
          elevation: 0,
          scrolledUnderElevation: 0,
          surfaceTintColor: Colors.transparent,
          centerTitle: false,
          toolbarHeight: DesignTokens.pageHeaderHeight,
          titleTextStyle: textTheme.headlineSmall),
      menuTheme: MenuThemeData(style: menu),
      dropdownMenuTheme: DropdownMenuThemeData(
          textStyle: textTheme.bodyMedium, menuStyle: menu),
      iconButtonTheme: IconButtonThemeData(
          style: IconButton.styleFrom(
                  foregroundColor: secondary,
                  disabledForegroundColor:
                      secondary.withValues(alpha: DesignTokens.disabledOpacity),
                  iconSize: DesignTokens.iconSize,
                  padding: const EdgeInsets.all(5),
                  minimumSize: const Size(
                      DesignTokens.iconButtonSize, DesignTokens.iconButtonSize),
                  shape: shape)
              .copyWith(
                  overlayColor: buttonOverlay,
                  side: WidgetStateProperty.resolveWith((states) =>
                      states.contains(WidgetState.focused)
                          ? const BorderSide(
                              color: selectedBorder,
                              width: DesignTokens.focusBorderWidth)
                          : BorderSide.none))),
      dialogTheme: DialogThemeData(
          backgroundColor: overlay,
          surfaceTintColor: Colors.transparent,
          barrierColor: const Color(0x66000000),
          shape: RoundedRectangleBorder(
              borderRadius: BorderRadius.circular(DesignTokens.cardRadius),
              side: const BorderSide(
                  color: divider, width: DesignTokens.dividerWidth)),
          titleTextStyle: textTheme.titleLarge,
          contentTextStyle: textTheme.bodyMedium!
              .copyWith(fontSize: DesignTokens.labelSize, color: secondary)),
      listTileTheme: const ListTileThemeData(
          textColor: primary,
          iconColor: secondary,
          selectedColor: primary,
          selectedTileColor: selectedSurface,
          titleTextStyle: TextStyle(
              fontSize: DesignTokens.bodySize,
              fontWeight: DesignTokens.controlWeight),
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
              fontSize: DesignTokens.labelSize,
              fontFamily: DesktopEnvironment.current.fontFamily),
          labelTextStyle: WidgetStatePropertyAll(body.copyWith(
              fontSize: DesignTokens.labelSize,
              color: primary,
              fontFamily: DesktopEnvironment.current.fontFamily)),
          shape: RoundedRectangleBorder(
              borderRadius: BorderRadius.circular(controlRadius),
              side: const BorderSide(
                  color: controlBorder, width: DesignTokens.dividerWidth))),
      tooltipTheme: TooltipThemeData(
          decoration: BoxDecoration(
              color: overlay,
              borderRadius: BorderRadius.circular(DesignTokens.tooltipRadius),
              border: Border.all(color: border)),
          textStyle: caption.copyWith(color: primary),
          waitDuration: const Duration(milliseconds: 450)),
      snackBarTheme: SnackBarThemeData(
          backgroundColor: overlay,
          contentTextStyle: body.copyWith(fontSize: DesignTokens.labelSize),
          behavior: SnackBarBehavior.floating,
          shape: shape),
      scrollbarTheme: ScrollbarThemeData(
          thickness: WidgetStateProperty.all(DesignTokens.scrollbarSize),
          radius: const Radius.circular(DesignTokens.scrollbarSize)),
      expansionTileTheme: const ExpansionTileThemeData(
          iconColor: secondary,
          collapsedIconColor: muted,
          backgroundColor: Colors.transparent,
          collapsedBackgroundColor: Colors.transparent),
    );
  }
}
