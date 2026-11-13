#!/usr/bin/env python3
"""Generate presentation-only Flutter and Web tokens from one source. Colors are ARGB."""
import argparse
import json
from pathlib import Path
import re
import subprocess
import sys

ROOT = Path(__file__).resolve().parent.parent


def css_name(name):
    return '--aw-' + re.sub(r'([A-Z])', r'-\1', name).lower()


def css_color(argb):
    if not isinstance(argb, str) or not re.fullmatch(r'#[0-9a-fA-F]{8}', argb):
        raise ValueError('Expected #AARRGGBB color')
    return '#' + argb[3:].lower() + argb[1:3].lower()


def render(spec):
    if spec['schemaVersion'] != 2 or spec['colorFormat'] != 'ARGB' or spec[
            'appearance'] != 'dark':
        raise ValueError('Unsupported design token schema')
    colors = spec['colors']
    variants = spec['variants']
    palettes = {
        'dark': colors,
        'light': {
            **colors,
            **variants['light']
        },
        'darkContrast': {
            **colors,
            **variants['contrast']['dark']
        },
        'lightContrast': {
            **colors,
            **variants['light'],
            **variants['contrast']['light']
        },
    }
    expected_colors = set(colors)
    if any(set(palette) != expected_colors for palette in palettes.values()):
        raise ValueError(
            'Appearance variant colors differ from the base palette')
    names = [
        key for group in ['colors', 'metrics', 'fonts'] for key in spec[group]
    ]
    if len(names) != len(set(names)) or any(
            not re.fullmatch(r'[a-z][A-Za-z0-9]*', k) for k in names):
        raise ValueError('Invalid or duplicate token name')
    dart, css, common_css = [], [], []
    for palette_name, palette in palettes.items():
        for key, value in palette.items():
            if not isinstance(value, str) or not re.fullmatch(
                    r'#[0-9a-fA-F]{8}', value):
                raise ValueError('Invalid appearance color')
        dart.append(f'  static const {palette_name} = DesignPalette(' +
                    ', '.join(f'{key}: Color(0x{value[1:].lower()})'
                              for key, value in palette.items()) + ');')
        selector = (
            ':root, :root[data-aw-appearance="dark"][data-aw-contrast="standard"]'
            if palette_name == 'dark' else
            f':root[data-aw-appearance="{ "light" if "light" in palette_name else "dark" }"]'
            f'[data-aw-contrast="{ "increased" if "Contrast" in palette_name else "standard" }"]'
        )
        css.append(selector + ' {')
        css.extend(f'  {css_name(key)}: {css_color(value)};'
                   for key, value in palette.items())
        css.append(
            f'  --aw-color-scheme: {"light" if "light" in palette_name else "dark"};'
        )
        css.append(
            f'  color-scheme: {"light" if "light" in palette_name else "dark"};'
        )
        css.append('}')
    for key, value in colors.items():
        dart.append(f'  static const {key} = Color(0x{value[1:].lower()});')
    for key, item in spec['metrics'].items():
        value, unit = item['value'], item['unit']
        if isinstance(value, bool) or not isinstance(
                value, (int, float)) or not 0 <= value <= 1000 or unit not in [
                    'px', 'number', 'fontWeight'
                ]:
            raise ValueError('Invalid numeric design token')
        if unit == 'fontWeight':
            if value not in range(100, 1000, 100):
                raise ValueError('Invalid font weight')
            dart.append(f'  static const {key} = FontWeight.w{int(value)};')
        else:
            dart.append(f'  static const {key} = {float(value)};')
        common_css.append(
            f'  {css_name(key)}: {value:g}{"px" if unit == "px" else ""};')
    for key, value in spec['fonts'].items():
        if not isinstance(value, str) or any(c in value for c in ';{}<>\n\r'):
            raise ValueError('Invalid font stack')
        common_css.append(f'  {css_name(key)}: {value};')
    for key, target in spec['aliases'].items():
        if not re.fullmatch(r'[a-z][A-Za-z0-9]*',
                            key) or key in names or target not in names:
            raise ValueError('Invalid alias')
        common_css.append(f'  {css_name(key)}: var({css_name(target)});')
    dart.append(
        '  static DesignPalette forAppearance({required String brightness, '
        'required bool increasedContrast}) {')
    dart.append(
        "    if (increasedContrast) return brightness == 'light' ? lightContrast : darkContrast;"
    )
    dart.append("    return brightness == 'light' ? light : dark;")
    dart.append('  }')
    forced_colors_css = '''@media (forced-colors: active) {
  :root {
    --aw-surface: Canvas;
    --aw-overlay: Canvas;
    --aw-menu-surface: Canvas;
    --aw-action-surface: Canvas;
    --aw-primary-button-surface: ButtonFace;
    --aw-button-selection: Highlight;
    --aw-keycap-surface: Canvas;
    --aw-keycap-border: CanvasText;
    --aw-card: Canvas;
    --aw-field: Field;
    --aw-elevated: Canvas;
    --aw-sidebar: Canvas;
    --aw-window: Canvas;
    --aw-sidebar-selection: Highlight;
    --aw-hover: ButtonFace;
    --aw-border: CanvasText;
    --aw-divider: CanvasText;
    --aw-control-surface: ButtonFace;
    --aw-control-selection: Highlight;
    --aw-control-border: ButtonText;
    --aw-primary: CanvasText;
    --aw-secondary: CanvasText;
    --aw-muted: GrayText;
    --aw-accent: LinkText;
    --aw-accent-fill: Highlight;
    --aw-warning: CanvasText;
    --aw-danger: LinkText;
    --aw-selected-surface: Highlight;
    --aw-selection-outline: Highlight;
    --aw-selected-border: Highlight;
    --aw-success: CanvasText;
    --aw-fallback-surface: Canvas;
    --aw-text-selection: Highlight;
    --aw-icon-blue: CanvasText;
    --aw-icon-green: CanvasText;
    --aw-icon-coral: CanvasText;
    --aw-icon-purple: CanvasText;
    --aw-icon-amber: CanvasText;
    --aw-icon-pink: CanvasText;
    --aw-icon-neutral: CanvasText;
    --aw-icon-badge-ink: Canvas;
    --aw-color-scheme: light;
    color-scheme: light;
  }
}
'''
    css = (
        '/* Generated by scripts/generate-design-tokens.py. Do not edit. */\n' +
        '\n'.join(css) + '\n' + ':root {\n' + '\n'.join(common_css) + '\n}\n' +
        'html, body { background: var(--aw-web-canvas, var(--aw-fallback-surface)); color: var(--aw-text); '
        + 'font-family: var(--aw-font-ui); font-size: var(--aw-body-size); ' +
        'line-height: var(--aw-body-line-height); }\n' + forced_colors_css)
    palette_fields = '\n'.join(f'  final Color {key};' for key in colors)
    palette_constructor = ',\n'.join(
        f'    required this.{key}' for key in colors)
    dart = ('// Generated by scripts/generate-design-tokens.py. Do not edit.\n'
            "import 'package:flutter/painting.dart';\n\n"
            'final class DesignPalette {\n'
            '  const DesignPalette({\n' + palette_constructor + ',\n  });\n\n' +
            palette_fields + '\n}\n\n'
            'abstract final class DesignTokens {\n' + '\n'.join(dart) +
            '\n}\n\n'
            "const webDesignTokensCss = r'''\n" + css + "''';\n")
    return dart, css


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check', action='store_true')
    args = parser.parse_args()
    dart, css = render(
        json.loads((ROOT / 'resources/design-tokens.json').read_text()))
    controls = (ROOT / 'resources/web-controls.css').read_text()
    dart += "\nconst webControlsCss = r'''\n" + controls + "''';\n"
    # Format through the pinned SDK so generated drift checks match normal Dart formatting.
    dart = subprocess.run(['dart', 'format', '--output=show', '--summary=none'],
                          input=dart,
                          capture_output=True,
                          text=True,
                          check=True).stdout
    for path, content in [
        (ROOT / 'apps/flutter/lib/ui/design_tokens.g.dart', dart),
        (ROOT / 'packages/plugin-sdk/styles/tokens.css', css),
        (ROOT / 'packages/plugin-sdk/styles/controls.css',
         '/* Generated by scripts/generate-design-tokens.py. Do not edit. */\n'
         + controls)
    ]:
        if args.check:
            if not path.exists() or path.read_text() != content:
                print(
                    f'Outdated generated design tokens: {path.relative_to(ROOT)}',
                    file=sys.stderr)
                return 1
        else:
            path.write_text(content)
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
