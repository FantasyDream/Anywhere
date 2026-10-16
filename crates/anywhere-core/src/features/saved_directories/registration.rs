use crate::{features::saved_directories, features::catalog::BuiltinFeatureRegistration};

pub const REGISTRATIONS: &[BuiltinFeatureRegistration] = &[
    BuiltinFeatureRegistration::ordered("saved_directories", 90, || Box::new(saved_directories::Feature)),
];
