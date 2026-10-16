use crate::{
    features::developer::{provider::ProjectKind, Feature},
    features::catalog::BuiltinFeatureRegistration,
};

pub const REGISTRATIONS: &[BuiltinFeatureRegistration] = &[
    BuiltinFeatureRegistration::ordered("rust", 40, || Box::new(Feature(ProjectKind::Rust))),
    BuiltinFeatureRegistration::ordered("maven", 50, || Box::new(Feature(ProjectKind::Maven))),
    BuiltinFeatureRegistration::ordered("gradle", 60, || Box::new(Feature(ProjectKind::Gradle))),
    BuiltinFeatureRegistration::ordered("node", 70, || Box::new(Feature(ProjectKind::Node))),
    BuiltinFeatureRegistration::ordered("spring_boot", 80, || Box::new(Feature(ProjectKind::SpringBoot))),
];
