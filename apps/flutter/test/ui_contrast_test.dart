import 'package:anywhere/ui/app_style.dart';
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
