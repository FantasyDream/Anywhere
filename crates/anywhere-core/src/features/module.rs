//! Compiled native feature contributions; only startup binds services.
use crate::features::{
    contribution::FeatureContribution, descriptor::FeatureDescriptor, services::FeatureServices,
};
use anyhow::{ensure, Result};
pub trait BuiltinFeature: Send + Sync {
    fn descriptor(&self) -> &'static FeatureDescriptor;
    fn contribution(&self) -> FeatureContribution;
    fn bind_services(&self, _services: &mut FeatureServices) -> Result<FeatureBindings> {
        Ok(FeatureBindings::default())
    }
}
#[derive(Default, Clone)]
pub struct FeatureBindings {
    pub actions: Vec<crate::core::action::descriptor::ActionDefinition>,
    pub sources: crate::core::search::candidate_source::SourceSet,
}
pub fn builtin_contributions() -> Result<Vec<FeatureContribution>> {
    let modules = crate::features::catalog::instantiate()?;
    let mut contributions = Vec::new();
    for module in modules {
        let metadata = module.descriptor();
        crate::features::host_services::grant(metadata)?;
        let contribution = module.contribution();
        ensure!(
            metadata.id == contribution.id,
            "feature ownership differs from its descriptor"
        );
        contributions.push(contribution);
    }
    Ok(contributions)
}
