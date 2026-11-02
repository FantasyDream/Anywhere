import importlib.util
from pathlib import Path
import tempfile
import unittest

spec = importlib.util.spec_from_file_location(
    'versions',
    Path(__file__).resolve().parents[1] / 'check-versions.py')
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


class BridgeVersionsTests(unittest.TestCase):

    def setUp(self):
        directory = tempfile.TemporaryDirectory()
        self.addCleanup(directory.cleanup)
        self.root = Path(directory.name)
        self.contents = {
            'apps/flutter/pubspec.yaml':
                'dependencies:\n  flutter_rust_bridge: 2.13.0\n',
            'crates/anywhere-bridge-flutter/Cargo.toml':
                'flutter_rust_bridge = "=2.13.0"\n',
            'apps/flutter/lib/src/rust/frb_generated.dart':
                "String get codegenVersion => '2.13.0';\n",
            'crates/anywhere-bridge-flutter/src/frb_generated.rs':
                'pub(crate) const FLUTTER_RUST_BRIDGE_CODEGEN_VERSION: &str = "2.13.0";\n',
        }
        for name, content in self.contents.items():
            path = self.root / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(content)

    def test_matched_runtimes_and_generated_bindings_pass(self):
        self.assertEqual(module.check_bridge_versions(self.root), '2.13.0')

    def test_independently_stale_runtimes_or_bindings_fail(self):
        for name, content in self.contents.items():
            with self.subTest(name=name):
                path = self.root / name
                path.write_text(content.replace('2.13.0', '2.11.1'))
                with self.assertRaisesRegex(SystemExit, 'FRB versions differ'):
                    module.check_bridge_versions(self.root)
                path.write_text(content)

    def test_missing_version_metadata_fails(self):
        (self.root /
         'apps/flutter/lib/src/rust/frb_generated.dart').write_text('')
        with self.assertRaisesRegex(SystemExit, 'metadata is missing'):
            module.check_bridge_versions(self.root)


if __name__ == '__main__':
    unittest.main()
