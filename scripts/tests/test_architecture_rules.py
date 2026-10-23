import sys
from pathlib import Path
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from architecture_rules import check_core_imports, check_feature_imports, check_persistence_imports


class CoreBoundaries(unittest.TestCase):

    def test_persistence_cannot_publish_application_state(self):
        for source in [
                'use crate::Application;',
                'use crate::application::web_plugins::WebPluginCatalog;',
                'fn publish() { crate::Application::new(); }'
        ]:
            with self.subTest(source=source), self.assertRaises(AssertionError):
                check_persistence_imports(source, 'persistence/fixture.rs')
        check_persistence_imports(
            'use anywhere_plugin::web::VerifiedWebPackage;',
            'persistence/fixture.rs')

    def test_extension_dependency_is_rejected_in_imports_and_qualified_calls(
            self):
        for source in [
                'use crate::extensions::git;',
                'use crate::{core::storage::ExtensionStorage, extensions as modules};',
                'fn f() { crate::extensions::modules(); }',
                'use super::super::application as app;'
        ]:
            with self.subTest(source=source), self.assertRaises(AssertionError):
                check_core_imports(source, 'core/fixture.rs')

    def test_core_contracts_comments_and_test_only_product_usage_are_allowed(
            self):
        check_core_imports(
            '''use crate::core::storage::ExtensionStorage;
// use crate::extensions::git;
#[cfg(test)] mod tests { use crate::extensions::git; }
''', 'core/fixture.rs')

    def test_feature_services_reject_concrete_composition_and_storage(self):
        for source in [
                'use crate::persistence::clipboard::SqliteHistoryRepository;',
                'use crate::{core::storage::StorageHost, application as app};',
                'fn f() { crate::application::activate(); }'
        ]:
            with self.subTest(source=source), self.assertRaises(AssertionError):
                check_feature_imports(source, 'features/fixture.rs')
        check_feature_imports(
            'use crate::{core::storage::StorageHost, features::clipboard::history::HistoryRepository};',
            'features/fixture.rs')


if __name__ == '__main__':
    unittest.main()
