"""Exercise repository-wide docs discovery and moved / ignored link failures."""
import importlib.util
from pathlib import Path
import subprocess
import tempfile
import unittest

spec = importlib.util.spec_from_file_location(
    'check_docs', Path(__file__).resolve().parents[1] / 'check-docs.py')
docs = importlib.util.module_from_spec(spec)
spec.loader.exec_module(docs)


class DocumentationTests(unittest.TestCase):
    def setUp(self):
        self.work = tempfile.TemporaryDirectory()
        self.addCleanup(self.work.cleanup)
        self.root = Path(self.work.name).resolve()
        subprocess.run(['git', 'init', '-q', str(self.root)], check=True)
        self.write('.gitignore', 'build/\n')

    def write(self, name, text):
        target = self.root / name
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(text)
        return target

    def test_discovers_root_github_sdk_and_examples_without_build_cache(self):
        names = ['README.md', 'CONTRIBUTING.md', '.github/PULL_REQUEST_TEMPLATE.md',
                 'packages/plugin-sdk/README.md', 'plugins/examples/tool/README.md']
        for name in [*names, 'build/README.md']:
            self.write(name, '# Guide\n')
        removed = self.write('docs/old.md', '# Old\n')
        subprocess.run(['git', 'add', 'docs/old.md'], cwd=self.root, check=True)
        removed.unlink()
        found = {p.relative_to(self.root).as_posix()
                 for p in docs.repository_documents(self.root)}
        self.assertEqual(found, set(names))

    def test_checks_links_in_new_root_files_and_rebased_archives(self):
        self.write('docs/archive/old.md', '# Historical\n\n[Current](../current.md#current)\n')
        self.write('docs/current.md', '# Current\n')
        contribution = self.write('CONTRIBUTING.md', '[Moved](docs/old.md)\n')
        failures, _ = docs.check_documents(self.root, docs.repository_documents(self.root))
        self.assertEqual(len(failures), 1)
        self.assertIn('CONTRIBUTING.md: docs/old.md: missing target', failures[0])
        contribution.write_text('[Moved](docs/archive/old.md#historical)\n')
        self.assertEqual(docs.check_documents(self.root, docs.repository_documents(self.root))[0], [])

    def test_rejects_ignored_outside_missing_anchor_and_unclosed_fence(self):
        self.write('build/result.md', '# Result\n')
        self.write('docs/current.md', '# Current\n')
        self.write('README.md', '[Local](build/result.md)\n[Outside](../outside.md)\n'
                   '[Heading](docs/current.md#missing)\n```\n')
        failures, _ = docs.check_documents(self.root, docs.repository_documents(self.root))
        self.assertEqual(len(failures), 4)
        for reason in ['ignored target', 'outside repository', 'missing heading', 'unclosed code fence']:
            self.assertTrue(any(reason in failure for failure in failures), reason)


if __name__ == '__main__':
    unittest.main()
