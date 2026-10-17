use crate::features::{catalog::BuiltinFeatureRegistration, clipboard};
pub const REGISTRATIONS: &[BuiltinFeatureRegistration] = &[
    BuiltinFeatureRegistration::ordered("clipboard", 100, || Box::new(clipboard::Feature)),
];
