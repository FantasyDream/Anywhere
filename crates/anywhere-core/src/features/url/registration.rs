use crate::{features::url, features::catalog::BuiltinFeatureRegistration};

pub const REGISTRATIONS: &[BuiltinFeatureRegistration] = &[
    BuiltinFeatureRegistration::ordered("url", 20, || Box::new(url::Feature)),
];
