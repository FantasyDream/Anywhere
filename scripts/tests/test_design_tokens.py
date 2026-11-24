"""Guard alpha preservation and invalid token values across GPUI and Web."""
import copy
import importlib.util
import json
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location(
    'design_tokens',
    Path(__file__).resolve().parents[1] / 'generate-design-tokens.py')
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)

ROOT = Path(__file__).resolve().parents[2]


def _parse_argb(value):
    raw = int(value[1:], 16)
    return (((raw >> 24) & 255) / 255.0, ((raw >> 16) & 255) / 255.0,
            ((raw >> 8) & 255) / 255.0, (raw & 255) / 255.0)


def _blend(fg, bg):
    alpha, r, g, b = fg
    _, br, bg_c, bb = bg
    return (1.0, alpha * r + (1.0 - alpha) * br,
            alpha * g + (1.0 - alpha) * bg_c, alpha * b + (1.0 - alpha) * bb)


def _luminance(color):
    _, r, g, b = color

    def channel(value):
        return value / 12.92 if value <= 0.03928 else ((value + 0.055) /
                                                       1.055)**2.4

    return 0.2126 * channel(r) + 0.7152 * channel(g) + 0.0722 * channel(b)


def _contrast_ratio(first, second):
    light, dark = sorted([_luminance(first), _luminance(second)], reverse=True)
    return (light + 0.05) / (dark + 0.05)


class DesignTokenTests(unittest.TestCase):

    def test_argb_becomes_css_rgba_without_losing_transparency(self):
        self.assertEqual(module.css_color('#a0303438'), '#303438a0')
        self.assertEqual(module.css_color('#00123456'), '#12345600')
        self.assertEqual(module.css_color('#FFabcdef'), '#abcdefff')

    def test_ambiguous_color_forms_are_rejected(self):
        for value in ['#123456', '#fff', 'rgba(1,2,3,4)', '#zz123456', 12]:
            with self.subTest(value=value), self.assertRaises(ValueError):
                module.css_color(value)

    def valid(self):
        return {
            'schemaVersion': 3,
            'colorFormat': 'ARGB',
            'palettes': {
                'opaqueDark': {
                    'primary': '#fff7f8fa'
                },
                'glassDark': {
                    'extends': 'opaqueDark',
                    'primary': '#ffffffff'
                },
                'glassLight': {
                    'extends': 'opaqueDark',
                    'primary': '#ff101010'
                }
            },
            'contrast': {
                'opaqueDark': {
                    'primary': '#ffffffff'
                },
                'glassDark': {},
                'glassLight': {
                    'primary': '#ff000000'
                },
                'fixed': {}
            },
            'fixed': {
                'menuSurface': '#ff2b2e35'
            },
            'metrics': {
                'radius': {
                    'value': 8,
                    'unit': 'px'
                },
                'headingWeight': {
                    'value': 600,
                    'unit': 'fontWeight'
                }
            },
            'fonts': {
                'fontUi': 'system-ui'
            },
            'aliases': {
                'text': 'primary'
            }
        }

    def test_font_weights_have_matching_gpui_and_css_types(self):
        css, rust = module.render(self.valid())
        self.assertIn(':root { color-scheme: dark; }', css)
        self.assertIn('--aw-primary: #f7f8faff;', css)
        self.assertIn('--aw-menu-surface: #2b2e35ff;', css)
        self.assertIn('--aw-heading-weight: 600;', css)
        self.assertIn('--aw-text: var(--aw-primary);', css)
        self.assertIn('pub const HEADING_WEIGHT: f32 = 600.0;', rust)
        self.assertIn('pub const TEXT: u32 = colors::OPAQUE_DARK.primary;',
                      rust)
        self.assertIn('pub fn web_variables(&self)', rust)
        self.assertIn('pub const FONT_UI: &str = ".SystemUIFont";', rust)

    def test_invalid_units_weights_and_style_injection_are_rejected(self):
        for mutate in [
                lambda s: s['metrics']['radius'].update(unit='em'),
                lambda s: s['metrics']['radius'].update(value=float('nan')),
                lambda s: s['metrics']['radius'].update(value=True),
                lambda s: s['metrics']['headingWeight'].update(value=650),
                lambda s: s['fonts'].update(fontUi='x;</style><script>'),
                lambda s: s['aliases'].update(text='missing'),
                lambda s: s['metrics'].update(primary={
                    'value': 1,
                    'unit': 'px'
                }),
                lambda s: s['palettes']['glassDark'].update(extends='glassLight'
                                                           ),
                lambda s: s['palettes']['glassLight'].update(glassLightPrimary=
                                                             '#ff000000'),
                lambda s: s['fixed'].update(primary='#ff000000'),
                lambda s: s['contrast']['opaqueDark'].update(unknown='#ff000000'
                                                            ),
        ]:
            value = copy.deepcopy(self.valid())
            mutate(value)
            with self.assertRaises(ValueError):
                module.render(value)

    def test_repository_tokens_keep_palette_roles_and_light_contrast(self):
        spec_data = json.loads(
            (ROOT / 'resources/design-tokens.json').read_text())
        palettes = module.expand_palettes(spec_data['palettes'])
        self.assertEqual(set(palettes['opaqueDark']),
                         set(palettes['glassDark']))
        self.assertEqual(set(palettes['opaqueDark']),
                         set(palettes['glassLight']))
        self.assertFalse(set(palettes['opaqueDark']) & set(spec_data['fixed']))

        light = palettes['glassLight']
        win = _blend(_parse_argb(light['window']), _parse_argb('#ffffffff'))
        selected = _blend(_parse_argb(light['resultSurfaceSelected']), win)
        for role in [
                'primary', 'secondary', 'muted', 'accent', 'warning', 'danger',
                'success'
        ]:
            fg = _parse_argb(light[role])
            self.assertGreaterEqual(_contrast_ratio(fg, win), 4.5, role)
            self.assertGreaterEqual(_contrast_ratio(fg, selected), 4.5, role)
        for role in [
                'iconBlue', 'iconGreen', 'iconCoral', 'iconPurple', 'iconAmber',
                'iconPink', 'iconNeutral'
        ]:
            fg = _parse_argb(light[role])
            self.assertGreaterEqual(_contrast_ratio(fg, win), 3.0, role)
            self.assertGreaterEqual(_contrast_ratio(fg, selected), 3.0, role)

    def test_action_menu_and_editor_text_contrast(self):
        spec_data = json.loads(
            (ROOT / 'resources/design-tokens.json').read_text())
        palettes = module.expand_palettes(spec_data['palettes'])
        for name, palette in palettes.items():
            for role in ['actionMenuSurface', 'actionMenuSelection']:
                surface = _parse_argb(palette[role])
                self.assertEqual(surface[0], 1.0, (name, role))
                self.assertGreaterEqual(
                    _contrast_ratio(_parse_argb(palette['primary']), surface),
                    4.5, (name, role))

        light = palettes['glassLight']
        backdrop = _parse_argb('#ff000000')
        window = _blend(_parse_argb(light['window']), backdrop)
        editor = _blend(_parse_argb(light['editorSurface']), window)
        for role in [
                'muted', 'iconBlue', 'iconGreen', 'iconAmber', 'iconPurple'
        ]:
            self.assertGreaterEqual(
                _contrast_ratio(_parse_argb(light[role]), editor), 4.5, role)


if __name__ == '__main__':
    unittest.main()
