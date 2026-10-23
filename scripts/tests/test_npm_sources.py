import importlib.util
import json
from pathlib import Path
import tempfile
import unittest

spec = importlib.util.spec_from_file_location(
    'npm_sources',
    Path(__file__).resolve().parents[1] / 'check-npm-sources.py')
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


class NpmSourceTests(unittest.TestCase):

    def check(self, entries):
        with tempfile.TemporaryDirectory() as temporary:
            lock = Path(temporary) / 'package-lock.json'
            lock.write_text(json.dumps({'packages': entries}))
            return module.check_lock(lock)

    def test_public_registry_and_local_sdk_are_supported(self):
        self.assertEqual(
            self.check({
                'node_modules/example': {
                    'resolved':
                        'https://registry.npmjs.org/example/-/example-1.0.0.tgz'
                },
                'node_modules/sdk': {
                    'resolved': '../../../packages/plugin-sdk',
                    'link': True
                },
                'vendor/sdk': {
                    'resolved': 'file:vendor/plugin-sdk'
                },
            }), [])

    def test_private_insecure_and_credential_urls_are_rejected_without_leaking_them(
            self):
        for source in [
                'https://private.example/pkg.tgz',
                'http://registry.npmjs.org/pkg.tgz',
                'https://secret:password@registry.npmjs.org/pkg.tgz',
                'https://registry.npmjs.org.example/pkg.tgz',
                'https://registry.npmjs.org/pkg.tgz?token=secret'
        ]:
            with self.subTest(source=source):
                failures = self.check(
                    {'node_modules/pkg': {
                        'resolved': source
                    }})
                self.assertEqual(len(failures), 1)
                self.assertNotIn('secret', failures[0])
                self.assertNotIn(source, failures[0])


if __name__ == '__main__':
    unittest.main()
