"""Guard alpha preservation and invalid token values across GPUI and Web."""
import copy
import importlib.util
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location(
    'design_tokens',
    Path(__file__).resolve().parents[1] / 'generate-design-tokens.py')
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


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
            'schemaVersion': 2,
            'appearance': 'dark',
            'colorFormat': 'ARGB',
            'colors': {
                'primary': '#fff7f8fa'
            },
            'variants': {
                'light': {
                    'primary': '#ff101010'
                },
                'contrast': {
                    'dark': {
                        'primary': '#ffffffff'
                    },
                    'light': {
                        'primary': '#ff000000'
                    }
                }
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
        self.assertIn('--aw-heading-weight: 600;', css)
        self.assertIn('--aw-text: var(--aw-primary);', css)
        self.assertIn('pub const HEADING_WEIGHT: f32 = 600.0;', rust)
        self.assertIn('pub const TEXT: u32 = colors::PRIMARY;', rust)
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
        ]:
            value = copy.deepcopy(self.valid())
            mutate(value)
            with self.assertRaises(ValueError):
                module.render(value)


if __name__ == '__main__':
    unittest.main()
