import 'package:anywhere/ui/app_style.dart';
import 'package:anywhere/ui/file_type_appearance.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

double contrast(Color foreground, Color background) {
  final ink = Color.alphaBlend(foreground, background).computeLuminance();
  final paper = background.computeLuminance();
  return ink > paper
      ? (ink + .05) / (paper + .05)
      : (paper + .05) / (ink + .05);
}

void main() {
  test('neutral glass veil separates labels from a bright native material', () {
    // Representative native glass colors, not a claim that Flutter can sample
    // the desktop underneath an AppKit surface. Native appearance is checked live.
    for (final material in [const Color(0xff808080), const Color(0xff969696)]) {
      for (final veil in [AppStyle.surface, AppStyle.window]) {
        final glass = Color.alphaBlend(veil, material);
        final selected = Color.alphaBlend(AppStyle.selectedSurface, glass);
        for (final ink in [
          AppStyle.primary,
          AppStyle.secondary,
          AppStyle.muted
        ]) {
          expect(contrast(ink, glass), greaterThanOrEqualTo(4.5));
          expect(contrast(ink, selected), greaterThanOrEqualTo(4.5));
        }
      }
    }
  });

  test('menu labels use an opaque, readable surface in Material 3', () {
    final menu = AppStyle.theme.popupMenuTheme;
    expect(menu.color, AppStyle.menuSurface);
    expect(menu.color!.a, 1);
    for (final states in <Set<WidgetState>>[
      {},
      {WidgetState.focused},
      {WidgetState.hovered}
    ]) {
      final ink = menu.labelTextStyle!.resolve(states)!.color!;
      expect(contrast(ink, menu.color!), greaterThanOrEqualTo(4.5));
    }
  });

  test('file glyphs remain distinct on bright glass and selected rows', () {
    final glass = Color.alphaBlend(AppStyle.surface, const Color(0xff969696));
    for (final surface in [
      glass,
      Color.alphaBlend(AppStyle.selectedSurface, glass)
    ]) {
      for (final type in FileTypeAppearance.values) {
        expect(contrast(type.tint, surface), greaterThanOrEqualTo(3),
            reason: type.name);
        if (type.stamp != null) {
          expect(contrast(DesignTokens.iconBadgeInk, type.tint),
              greaterThanOrEqualTo(4.5),
              reason: '${type.name} type label');
        }
      }
    }
  });

  test('input labels carry contrast independently of the backdrop', () {
    for (final behind in [
      Colors.white,
      Colors.black,
      Colors.red,
      Colors.blue
    ]) {
      for (final surface in [
        AppStyle.controlSurface,
        AppStyle.controlSelection
      ]) {
        final control = Color.alphaBlend(surface, behind);
        for (final text in [
          AppStyle.primary,
          AppStyle.secondary,
          AppStyle.muted
        ]) {
          expect(contrast(text, control), greaterThanOrEqualTo(4.5));
        }
      }
    }
  });

  test(
      'transparent selected rows remain readable on representative dark surfaces',
      () {
    for (final backdrop in [
      const Color(0xff24262b),
      const Color(0xff33353a),
      const Color(0xff5c6066),
      AppStyle.overlay
    ]) {
      final selected = Color.alphaBlend(AppStyle.selectedSurface, backdrop);
      for (final interaction in [
        Colors.transparent,
        AppStyle.hover,
        AppStyle.buttonSelection
      ]) {
        expect(
            contrast(AppStyle.primary, Color.alphaBlend(interaction, selected)),
            greaterThanOrEqualTo(4.5));
      }
    }
  });

  test('translucent button labels are readable on representative dark surfaces',
      () {
    for (final backdrop in [
      const Color(0xff24262b),
      const Color(0xff33353a),
      const Color(0xff5c6066),
      AppStyle.overlay,
    ]) {
      for (final fill in [
        AppStyle.actionSurface,
        AppStyle.primaryButtonSurface,
        AppStyle.buttonSelection
      ]) {
        final surface = Color.alphaBlend(fill, backdrop);
        expect(contrast(AppStyle.primary, surface), greaterThanOrEqualTo(4.5));
      }
    }
  });

  test('button hover and focus preserve readable labels on dark glass', () {
    final theme = AppStyle.theme;
    for (final style in [
      theme.textButtonTheme.style!,
      theme.outlinedButtonTheme.style!,
      theme.filledButtonTheme.style!,
      theme.outlinedButtonTheme.style!.copyWith(
          backgroundColor: const WidgetStatePropertyAll(AppStyle.actionSurface))
    ]) {
      for (final states in <Set<WidgetState>>[
        {},
        {WidgetState.hovered},
        {WidgetState.focused},
        {WidgetState.pressed}
      ]) {
        final fill =
            style.backgroundColor?.resolve(states) ?? Colors.transparent;
        final overlay =
            style.overlayColor?.resolve(states) ?? Colors.transparent;
        final color = style.foregroundColor!.resolve(states)!;
        final surface = Color.alphaBlend(
            overlay, Color.alphaBlend(fill, const Color(0xff5c6066)));
        expect(contrast(color, surface), greaterThanOrEqualTo(4.5));
      }
    }
  });
}
