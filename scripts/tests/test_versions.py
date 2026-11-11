import importlib.util
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location(
    'versions',
    Path(__file__).resolve().parents[1] / 'check-versions.py')
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


class AppVersionTests(unittest.TestCase):

    def test_workspace_members_match_the_app_release(self):
        metadata = {
            'workspace_members': ['core', 'gpui'],
            'packages': [
                {
                    'id': 'core',
                    'name': 'anywhere-core',
                    'version': '0.14.0'
                },
                {
                    'id': 'gpui',
                    'name': 'anywhere-gpui',
                    'version': '0.14.0'
                },
                {
                    'id': 'external',
                    'name': 'external',
                    'version': '9.0.0'
                },
            ],
        }
        module.check_workspace_versions(metadata, '0.14.0')

    def test_stale_workspace_member_fails(self):
        metadata = {
            'workspace_members': ['gpui'],
            'packages': [{
                'id': 'gpui',
                'name': 'anywhere-gpui',
                'version': '0.13.0'
            },],
        }
        with self.assertRaisesRegex(SystemExit, 'anywhere-gpui'):
            module.check_workspace_versions(metadata, '0.14.0')


if __name__ == '__main__':
    unittest.main()
