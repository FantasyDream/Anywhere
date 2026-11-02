use crate::features::{application_history, catalog::BuiltinFeatureRegistration};
pub const REGISTRATIONS: &[BuiltinFeatureRegistration] = &[
    BuiltinFeatureRegistration::ordered("application_history", 110, || Box::new(application_history::Feature)),
];
