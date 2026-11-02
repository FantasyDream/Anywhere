//! The single, compile-time catalogue of bundled tool features.
//!
//! Module-owned registrations are discovered by Cargo at compile time; metadata
//! is typed Rust. BuiltinFeature describes startup-only contributions.
//! This catalogue does not load code, create runtimes or manage feature settings.
use crate::{features::descriptor::valid_id, features::BuiltinFeature};
use anyhow::{ensure, Result};
use std::{collections::BTreeSet, sync::LazyLock};

#[derive(Clone, Copy)]
pub struct BuiltinFeatureRegistration {
    pub id: &'static str,
    order: u16,
    factory: fn() -> Box<dyn BuiltinFeature>,
}

impl BuiltinFeatureRegistration {
    pub const fn new(id: &'static str, factory: fn() -> Box<dyn BuiltinFeature>) -> Self {
        Self::ordered(id, 1000, factory)
    }

    pub const fn ordered(
        id: &'static str,
        order: u16,
        factory: fn() -> Box<dyn BuiltinFeature>,
    ) -> Self {
        Self { id, order, factory }
    }
}

include!(concat!(env!("OUT_DIR"), "/builtin_feature_catalog.rs"));

// Reject invalid / duplicate declarations while compiling the App, before any
// constructor runs. Each module owns its ordering; ties are deterministic by ID.
const _: () = assert!(
    valid_registration_groups(BUILTIN_FEATURE_GROUPS),
    "invalid or duplicate builtin feature registration"
);
pub static BUILTIN_FEATURES: LazyLock<Vec<BuiltinFeatureRegistration>> =
    LazyLock::new(|| ordered_registrations(BUILTIN_FEATURE_GROUPS));

fn ordered_registrations(
    groups: &[&[BuiltinFeatureRegistration]],
) -> Vec<BuiltinFeatureRegistration> {
    let mut entries: Vec<_> = groups
        .iter()
        .flat_map(|group| group.iter().copied())
        .collect();
    entries.sort_by_key(|entry| (entry.order, entry.id));
    entries
}

const fn same_id(left: &str, right: &str) -> bool {
    let left = left.as_bytes();
    let right = right.as_bytes();
    if left.len() != right.len() {
        return false;
    }
    let mut index = 0;
    while index < left.len() {
        if left[index] != right[index] {
            return false;
        }
        index += 1;
    }
    true
}

const fn valid_registration_groups(groups: &[&[BuiltinFeatureRegistration]]) -> bool {
    if groups.is_empty() {
        return false;
    }
    let mut group = 0;
    while group < groups.len() {
        if groups[group].is_empty() {
            return false;
        }
        let mut item = 0;
        while item < groups[group].len() {
            let id = groups[group][item].id;
            if !valid_id(id) {
                return false;
            }
            let mut previous_group = 0;
            while previous_group <= group {
                let limit = if previous_group == group {
                    item
                } else {
                    groups[previous_group].len()
                };
                let mut previous_item = 0;
                while previous_item < limit {
                    if same_id(id, groups[previous_group][previous_item].id) {
                        return false;
                    }
                    previous_item += 1;
                }
                previous_group += 1;
            }
            item += 1;
        }
        group += 1;
    }
    true
}

pub fn instantiate() -> Result<Vec<Box<dyn BuiltinFeature>>> {
    instantiate_catalog(&BUILTIN_FEATURES)
}

/// Validate identities before running constructors. A bad catalogue cannot
/// start construction and fail only when it encounters a duplicate later.
fn instantiate_catalog(
    entries: &[BuiltinFeatureRegistration],
) -> Result<Vec<Box<dyn BuiltinFeature>>> {
    let mut ids = BTreeSet::new();
    for entry in entries {
        ensure!(
            valid_id(entry.id) && ids.insert(entry.id),
            "invalid or duplicate builtin feature id: {}",
            entry.id
        );
    }
    let mut modules = Vec::with_capacity(entries.len());
    for entry in entries {
        let module = (entry.factory)();
        module.descriptor().validate()?;
        ensure!(
            module.descriptor().id == entry.id,
            "builtin feature constructor identity differs: {}",
            entry.id
        );
        modules.push(module);
    }
    Ok(modules)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features;

    #[test]
    fn registration_groups_reject_duplicate_invalid_and_empty_declarations() {
        let item =
            BuiltinFeatureRegistration::new("tools.valid-id", || Box::new(features::url::Feature));
        assert!(valid_registration_groups(&[&[item]]));
        assert!(!valid_registration_groups(&[&[item], &[item]]));
        assert!(!valid_registration_groups(&[&[item, item]]));
        assert!(!valid_registration_groups(&[]));
        assert!(!valid_registration_groups(&[&[]]));
        for id in ["", "Bad", "bad/path", "bad..path", "bad.", "1bad"] {
            let invalid = BuiltinFeatureRegistration::new(id, item.factory);
            assert!(!valid_registration_groups(&[&[invalid]]));
        }
    }

    #[test]
    fn module_order_and_ties_are_independent_of_discovery_order() {
        let factory = || Box::new(features::url::Feature) as Box<dyn BuiltinFeature>;
        let early = BuiltinFeatureRegistration::ordered("early", 10, factory);
        let alpha = BuiltinFeatureRegistration::new("alpha", factory);
        let zeta = BuiltinFeatureRegistration::new("zeta", factory);
        let entries = ordered_registrations(&[&[zeta], &[alpha, early]]);
        assert_eq!(
            entries.iter().map(|e| e.id).collect::<Vec<_>>(),
            ["early", "alpha", "zeta"]
        );
    }

    #[test]
    fn catalogue_preserves_existing_module_and_contribution_identities() {
        let modules = instantiate().unwrap();
        assert_eq!(
            modules
                .iter()
                .map(|m| m.descriptor().id)
                .collect::<Vec<_>>(),
            [
                "git",
                "url",
                "rust",
                "maven",
                "gradle",
                "node",
                "spring_boot",
                "saved_directories",
                "clipboard",
                "application_history"
            ]
        );
        for (entry, module) in BUILTIN_FEATURES.iter().zip(modules) {
            assert_eq!(entry.id, module.contribution().id);
        }
    }

    #[test]
    fn invalid_catalogue_fails_before_any_constructor_runs() {
        fn must_not_construct() -> Box<dyn BuiltinFeature> {
            panic!("invalid catalogue invoked a constructor")
        }
        let item = BuiltinFeatureRegistration::new("same", must_not_construct);
        assert!(instantiate_catalog(&[item, item]).is_err());
        for id in ["", "Bad", "bad/path", "bad..path"] {
            assert!(instantiate_catalog(&[BuiltinFeatureRegistration::new(
                id,
                must_not_construct
            )])
            .is_err());
        }
    }

    #[test]
    fn constructor_cannot_register_another_feature_identity() {
        let entry = BuiltinFeatureRegistration::new("pretend", || Box::new(features::url::Feature));
        assert!(instantiate_catalog(&[entry])
            .err()
            .unwrap()
            .to_string()
            .contains("identity differs"));
    }

    #[test]
    fn another_registration_uses_the_same_generic_instantiation_path() {
        let entry = BuiltinFeatureRegistration::new("url", || Box::new(features::url::Feature));
        let modules = instantiate_catalog(&[entry]).unwrap();
        assert_eq!(modules.len(), 1);
        assert_eq!(modules[0].contribution().id, "url");
    }
}
