import importlib.util
from pathlib import Path
import tempfile
import unittest

spec = importlib.util.spec_from_file_location(
    'macos_app_path',
    Path(__file__).resolve().parents[1] / 'macos_app_path.py')
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


class MacosAppPathTests(unittest.TestCase):

    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name).resolve()
        (self.root / 'build').mkdir()

    def test_new_existing_and_normalized_bundle_paths(self):
        app = self.root / 'build/Anywhere.app'
        self.assertEqual(module.app_path(self.root, 'build/Anywhere.app'), app)
        app.mkdir()
        self.assertEqual(module.app_path(self.root, str(app)), app)
        self.assertEqual(
            module.app_path(self.root, 'build/temp/../Anywhere.app'), app)

    def test_directory_traversal_sibling_and_wrong_suffix_are_rejected(self):
        for value in [
                'build/../outside.app', 'build/nested/../../outside.app',
                'build-other/Anywhere.app', 'build', 'build/Anywhere.zip'
        ]:
            with self.subTest(value=value), self.assertRaises(ValueError):
                module.app_path(self.root, value)

    def test_leaf_and_escaping_parent_symlinks_are_rejected(self):
        target = self.root / 'build/target.app'
        target.mkdir()
        leaf = self.root / 'build/alias.app'
        leaf.symlink_to(target, target_is_directory=True)
        with self.assertRaises(ValueError):
            module.app_path(self.root, str(leaf))
        outside = self.root / 'outside'
        outside.mkdir()
        (self.root / 'build/escape').symlink_to(outside,
                                                target_is_directory=True)
        with self.assertRaises(ValueError):
            module.app_path(self.root, 'build/escape/Anywhere.app')
        self.assertTrue(target.is_dir())
        self.assertTrue(outside.is_dir())

    def test_build_root_symlink_cannot_redirect_bundle_removal(self):
        (self.root / 'build').rmdir()
        outside = self.root / 'outside'
        outside.mkdir()
        (self.root / 'build').symlink_to(outside, target_is_directory=True)
        with self.assertRaises(ValueError):
            module.app_path(self.root, 'build/Anywhere.app')


if __name__ == '__main__':
    unittest.main()
