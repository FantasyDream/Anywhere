//! Menu execution is a normal feature handler; Core has no menu-specific executor.
use crate::core::{
    action::handler::{ActionHandler, ExecutionOutput},
    context::ResolvedContext,
};
use anyhow::Result;
use anywhere_platform::{ApplicationInfo, MenuLocator, Platform};
use std::time::Instant;

pub struct MenuAction {
    pub app: ApplicationInfo,
    pub locator: MenuLocator,
}

#[derive(Debug)]
pub struct MenuExecutionFailure(pub anyhow::Error);
impl std::fmt::Display for MenuExecutionFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
impl std::error::Error for MenuExecutionFailure {}

impl ActionHandler for MenuAction {
    fn execute(&self, _: &ResolvedContext, platform: &dyn Platform) -> Result<ExecutionOutput> {
        let started = Instant::now();
        let result = platform.perform_menu_action(&self.app, &self.locator);
        tracing::info!(
            execute_duration = started.elapsed().as_micros() as u64,
            success = result.is_ok(),
            "app_action_execute"
        );
        result.map_err(MenuExecutionFailure)?;
        Ok(ExecutionOutput::dismiss())
    }
}
