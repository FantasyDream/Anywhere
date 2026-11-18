"""Unit tests for the deterministic GPUI localization projection."""

import importlib.util
import json
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location(
    'gpui_localizations',
    Path(__file__).resolve().parents[1] / 'generate-gpui-localizations.py')
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


class GpuiLocalizationTests(unittest.TestCase):

    def test_plural_parser_preserves_exact_and_category_options(self):
        key, options, prefix, suffix = module.parse_plural(
            '{count, plural, =1{one} few{few} other{many}}')
        self.assertEqual(key, 'count')
        self.assertEqual(options['=1'], 'one')
        self.assertEqual(options['few'], 'few')
        self.assertEqual(options['other'], 'many')
        self.assertEqual(prefix, '')
        self.assertEqual(suffix, '')

    def test_plural_requires_other(self):
        with self.assertRaises(ValueError):
            module.parse_plural('{count, plural, =1{one}}')

    def test_generated_catalog_has_full_message_count(self):
        value = module.render()
        self.assertIn(
            f'pub const MESSAGE_COUNT: usize = {module.message_count()};',
            value)
        self.assertIn('pub fn search_issues(', value)
        self.assertIn('pub fn history_storage_usage(', value)
        self.assertIn(
            'pub fn permission_context_read(locale: Locale, types: &str)',
            value)
        self.assertIn('Locale::En => "Settings".to_string(),', value)

    def test_rust_ident_only_escapes_keywords(self):
        self.assertEqual(module.rust_ident('type'), 'r#type')
        self.assertEqual(module.rust_ident('count'), 'count')

    def test_duplicate_keys_are_rejected_including_nested_metadata(self):
        for value in [
                '{"same": "one", "same": "two"}',
                '{"@message": {"type": "int", "type": "String"}}'
        ]:
            with self.assertRaisesRegex(ValueError,
                                        'Duplicate localization key'):
                json.loads(value,
                           object_pairs_hook=module.reject_duplicate_keys)

    def test_committed_output_is_current(self):
        self.assertEqual(module.OUTPUT.read_text(), module.render())


if __name__ == '__main__':
    unittest.main()
