import sys
from pathlib import Path
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from architecture_rules import check_core_imports


class CoreBoundaries(unittest.TestCase):
    def test_extension_dependency_is_rejected_in_imports_and_qualified_calls(self):
        for source in ['use crate::extensions::git;',
                       'use crate::{core::storage::ExtensionStorage, extensions as modules};',
                       'fn f() { crate::extensions::modules(); }',
                       'use super::super::application as app;']:
            with self.subTest(source=source), self.assertRaises(AssertionError):
                check_core_imports(source, 'core/fixture.rs')

    def test_core_contracts_comments_and_test_only_product_usage_are_allowed(self):
        check_core_imports('''use crate::core::storage::ExtensionStorage;
// use crate::extensions::git;
#[cfg(test)] mod tests { use crate::extensions::git; }
''', 'core/fixture.rs')


if __name__ == '__main__':
    unittest.main()
