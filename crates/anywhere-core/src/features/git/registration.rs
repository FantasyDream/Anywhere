use crate::{features::git, features::catalog::BuiltinFeatureRegistration};

pub const REGISTRATIONS: &[BuiltinFeatureRegistration] = &[
    BuiltinFeatureRegistration::ordered("git", 10, || Box::new(git::Feature)),
];
