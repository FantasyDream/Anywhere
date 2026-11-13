import 'package:cupertino_ui/cupertino_ui.dart';
import 'package:flutter/services.dart';
import 'package:material_ui/material_ui.dart';

import '../infrastructure/desktop_appearance.dart';
import '../infrastructure/desktop_edit_menu.dart';
import '../infrastructure/desktop_environment.dart';
import 'design_tokens.g.dart';

export 'design_tokens.g.dart';

/// One palette and control vocabulary for every window and sheet.
/// Native AppKit supplies the blur; content surfaces stabilize text contrast.
abstract final class AppStyle {
  // A neutral veil over the native blur softens bright backgrounds without
  // adding a second blur layer or replacing glass with a flat opaque panel.
  static DesignPalette get palette {
    final appearance = DesktopAppearance.current.snapshot;
    return DesignTokens.forAppearance(
        brightness: appearance.brightness == AppearanceBrightness.light
            ? 'light'
            : 'dark',
        increasedContrast: appearance.increasedContrast);
  }

  static Color get surface => palette.surface;
  static Color get overlay => palette.overlay;
  static Color get menuSurface => palette.menuSurface;
  static Color get actionSurface => palette.actionSurface;
  static Color get buttonSelection => palette.buttonSelection;
  static Color get keycapSurface => palette.keycapSurface;
  static Color get keycapBorder => palette.keycapBorder;
  static Color get card => palette.card;
  static Color get field => palette.field;
  static Color get elevated => palette.elevated;
  static Color get sidebar => palette.sidebar;
  static Color get window => palette.window;
  static Color get sidebarSelection => palette.sidebarSelection;
  static Color get hover => palette.hover;
  static Color get border => palette.border;
  static Color get divider => palette.divider;
  // Local controls carry their own contrast; the window glass stays untouched.
  static Color get controlSurface => palette.controlSurface;
  static Color get controlSelection => palette.controlSelection;
  static Color get controlBorder => palette.controlBorder;
  static const List<Shadow> textShadows = [];
  static Color get primary => palette.primary;
  static Color get secondary => palette.secondary;
  static Color get muted => palette.muted;
  static Color? get _systemAccent {
    final value = DesktopAppearance.current.snapshot.accentColor;
    if (value == null || value >> 24 == 0) return null;
    return Color(value);
  }

  static Color? _systemColor(String key) {
    final value = DesktopAppearance.current.snapshot.systemColors[key];
    if (value == null || value >> 24 == 0) return null;
    return Color(value);
  }

  static Color get accent => _systemAccent ?? palette.accent;
  static Color get accentFill => _systemAccent ?? palette.accentFill;
  static Color get onAccent =>
      accentFill.computeLuminance() > 0.179 ? Colors.black : Colors.white;
  static Color get primaryButtonSurface =>
      _systemAccent?.withValues(alpha: palette.primaryButtonSurface.a) ??
      palette.primaryButtonSurface;
  static Color get warning => palette.warning;
  static Color get danger => palette.danger;
  static Color get selectedSurface =>
      _systemAccent?.withValues(alpha: palette.selectedSurface.a) ??
      palette.selectedSurface;
  static Color get selectionOutline =>
      _systemColor('keyboardFocusIndicator') ??
      _systemAccent ??
      palette.selectionOutline;
  static Color get selectedBorder =>
      _systemColor('keyboardFocusIndicator') ??
      _systemAccent ??
      palette.selectedBorder;
  static Color get success => palette.success;
  static Color get fallbackSurface => palette.fallbackSurface;
  static Color get textSelection => palette.textSelection;
  static Color get iconBlue => palette.iconBlue;
  static Color get iconGreen => palette.iconGreen;
  static Color get iconCoral => palette.iconCoral;
  static Color get iconPurple => palette.iconPurple;
  static Color get iconAmber => palette.iconAmber;
  static Color get iconPink => palette.iconPink;
  static Color get iconNeutral => palette.iconNeutral;
  static Color get iconBadgeInk => palette.iconBadgeInk;
  static Duration motionDuration(Duration duration) =>
      DesktopAppearance.current.snapshot.reduceMotion
          ? Duration.zero
          : duration;
  static const radius = DesignTokens.radius;
  static const resultRadius = DesignTokens.resultRadius;
  static const controlRadius = DesignTokens.controlRadius;
  static const cardRadius = DesignTokens.cardRadius;
  static TextStyle get body => TextStyle(
      fontSize: DesignTokens.bodySize,
      height: DesignTokens.bodyLineHeight,
      fontWeight: DesignTokens.bodyWeight,
      color: primary,
      shadows: textShadows);
  static TextStyle get caption => TextStyle(
      fontSize: DesignTokens.captionSize,
      height: DesignTokens.captionLineHeight,
      color: muted,
      shadows: textShadows);
  static TextStyle get heading => TextStyle(
      fontSize: DesignTokens.headingSize,
      fontWeight: DesignTokens.headingWeight,
      color: primary,
      letterSpacing: 0,
      shadows: textShadows);
  static TextStyle get paneTitle => TextStyle(
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
      side: BorderSide(color: divider, width: DesignTokens.dividerWidth));

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
                  color: DesktopAppearance.current.snapshot.usesSystemSurface &&
                          !DesktopAppearance.current.snapshot.reduceTransparency
                      ? Colors.transparent
                      : fallbackSurface,
                  child: child ?? const SizedBox.shrink())));

  static ThemeData get theme {
    final appearance = DesktopAppearance.current.snapshot;
    final brightness = appearance.brightness == AppearanceBrightness.light
        ? Brightness.light
        : Brightness.dark;
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
        return BorderSide(
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
                : onAccent),
        textStyle: WidgetStatePropertyAll(controlText.copyWith(
            fontWeight: DesignTokens.primaryControlWeight)));
    final menu = MenuStyle(
        backgroundColor: WidgetStatePropertyAll(menuSurface),
        surfaceTintColor: const WidgetStatePropertyAll(Colors.transparent),
        shape: WidgetStatePropertyAll(shape),
        side: WidgetStatePropertyAll(BorderSide(
            color: controlBorder, width: DesignTokens.dividerWidth)));
    final base = ThemeData(
      fontFamily: DesktopEnvironment.current.fontFamily,
      brightness: brightness,
      useMaterial3: true,
      cupertinoOverrideTheme: CupertinoThemeData(
          brightness: brightness,
          primaryColor: accentFill,
          primaryContrastingColor: onAccent),
      scaffoldBackgroundColor: Colors.transparent,
      canvasColor: overlay,
      iconTheme: IconThemeData(color: secondary, size: DesignTokens.iconSize),
      splashFactory: NoSplash.splashFactory,
      hoverColor: hover,
      focusColor: buttonSelection,
      highlightColor: buttonSelection,
      colorScheme: ColorScheme(
          brightness: brightness,
          primary: accent,
          onPrimary: onAccent,
          secondary: accent,
          onSecondary: onAccent,
          surface: card,
          onSurface: primary,
          onSurfaceVariant: secondary,
          outline: border,
          outlineVariant: divider,
          error: danger,
          onError: Colors.white),
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
      dividerTheme: DividerThemeData(
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
            TextStyle(fontSize: DesignTokens.labelSize, color: secondary),
        floatingLabelStyle:
            TextStyle(fontSize: DesignTokens.labelSize, color: accent),
        contentPadding: const EdgeInsets.symmetric(
            horizontal: DesignTokens.fieldPaddingX,
            vertical: DesignTokens.fieldPaddingY),
        border: OutlineInputBorder(
            borderRadius: BorderRadius.circular(controlRadius)),
        enabledBorder: OutlineInputBorder(
            borderRadius: BorderRadius.circular(controlRadius),
            borderSide: BorderSide(
                color: border, width: DesignTokens.fieldBorderWidth)),
        focusedBorder: OutlineInputBorder(
            borderRadius: BorderRadius.circular(controlRadius),
            borderSide: BorderSide(
                color: selectedBorder,
                width: DesignTokens.fieldFocusBorderWidth)),
      ),
      textSelectionTheme: TextSelectionThemeData(
          cursorColor: accent,
          selectionColor: palette.textSelection,
          selectionHandleColor: accent),
      textButtonTheme: TextButtonThemeData(
          style: control.copyWith(
              side: WidgetStateProperty.resolveWith((states) =>
                  states.contains(WidgetState.focused)
                      ? BorderSide(
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
              side: BorderSide(
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
                          ? BorderSide(
                              color: selectedBorder,
                              width: DesignTokens.focusBorderWidth)
                          : BorderSide.none))),
      dialogTheme: DialogThemeData(
          backgroundColor: overlay,
          surfaceTintColor: Colors.transparent,
          barrierColor: const Color(0x66000000),
          shape: RoundedRectangleBorder(
              borderRadius: BorderRadius.circular(DesignTokens.cardRadius),
              side:
                  BorderSide(color: divider, width: DesignTokens.dividerWidth)),
          titleTextStyle: textTheme.titleLarge,
          contentTextStyle: textTheme.bodyMedium!
              .copyWith(fontSize: DesignTokens.labelSize, color: secondary)),
      listTileTheme: ListTileThemeData(
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
              states.contains(WidgetState.selected) ? onAccent : secondary),
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
              side: BorderSide(
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
      expansionTileTheme: ExpansionTileThemeData(
          iconColor: secondary,
          collapsedIconColor: muted,
          backgroundColor: Colors.transparent,
          collapsedBackgroundColor: Colors.transparent),
    );
  }
}
