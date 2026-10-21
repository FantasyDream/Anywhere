import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
import zipfile

spec = importlib.util.spec_from_file_location("web_pack", Path(__file__).parents[1] / "tools/package.py")
pack = importlib.util.module_from_spec(spec)
spec.loader.exec_module(pack)


class PackageTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        (self.root / "dist").mkdir()
        (self.root / "dist/index.html").write_text("<p>test</p>")
        (self.root / "manifest.json").write_text(json.dumps({"entry": "dist/index.html"}))
        self.output = self.root / "output/tool.awplugin"

    def test_only_runtime_files_are_packaged_reproducibly(self):
        (self.root / "private.txt").write_text("must not ship")
        (self.root / "node_modules").mkdir()
        (self.root / "node_modules/file.js").write_text("must not ship")
        (self.root / "dist/.DS_Store").write_bytes(b"ignored")
        pack.package(self.root, self.output)
        first = self.output.read_bytes()
        pack.package(self.root, self.output)
        self.assertEqual(first, self.output.read_bytes())
        with zipfile.ZipFile(self.output) as archive:
            self.assertEqual(archive.namelist(), ["dist/index.html", "manifest.json"])

    def test_missing_entry_does_not_replace_existing_output(self):
        pack.package(self.root, self.output)
        before = self.output.read_bytes()
        (self.root / "dist/index.html").unlink()
        with self.assertRaises(ValueError):
            pack.package(self.root, self.output)
        self.assertEqual(before, self.output.read_bytes())

    def test_symlinked_file_or_directory_is_not_packaged(self):
        outside = self.root / "outside.txt"
        outside.write_text("private")
        for directory in [False, True]:
            link = self.root / "dist/link"
            link.symlink_to(self.root if directory else outside, target_is_directory=directory)
            with self.assertRaises(ValueError):
                pack.package(self.root, self.output)
            link.unlink()

    def test_output_inside_assets_and_parent_paths_are_rejected(self):
        with self.assertRaises(ValueError):
            pack.package(self.root, self.root / "dist/tool.awplugin")
        with self.assertRaises(ValueError):
            pack.package(self.root, self.output, ["../"])

    def test_nondefault_runtime_roots_are_explicit(self):
        (self.root / "assets").mkdir()
        (self.root / "assets/tool.png").write_bytes(b"test")
        manifest = {"entry": "dist/index.html", "icon": "assets/tool.png"}
        (self.root / "manifest.json").write_text(json.dumps(manifest))
        with self.assertRaises(ValueError):
            pack.package(self.root, self.output)
        pack.package(self.root, self.output, ["dist", "assets"])
        with zipfile.ZipFile(self.output) as archive:
            self.assertIn("assets/tool.png", archive.namelist())

    def test_invalid_manifest_and_large_asset_fail_clearly(self):
        (self.root / "manifest.json").write_text("[]")
        with self.assertRaises(ValueError):
            pack.package(self.root, self.output)
        (self.root / "manifest.json").write_text('{"entry":"dist/index.html"}')
        with (self.root / "dist/huge.js").open("wb") as file:
            file.truncate(16 * 1024 * 1024 + 1)
        with self.assertRaises(ValueError):
            pack.package(self.root, self.output)
