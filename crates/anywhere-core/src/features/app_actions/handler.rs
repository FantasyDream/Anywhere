//! Menu execution is a normal feature handler; Core has no menu-specific executor.
use crate::core::{
    action::handler::{ActionHandler, ExecutionOutput},
    context::ResolvedContext,
};
use anyhow::Result;
use anywhere_platform::{ApplicationInfo, MenuLocator, Platform};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

static NEXT_EXECUTION_TOKEN: AtomicU64 = AtomicU64::new(1);

pub struct MenuAction {
    pub app: ApplicationInfo,
    pub locator: MenuLocator,
    execution_token: u64,
}

impl MenuAction {
    pub fn new(app: ApplicationInfo, locator: MenuLocator) -> Self {
        Self {
            app,
            locator,
            execution_token: NEXT_EXECUTION_TOKEN
                .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_add(1))
                .expect("menu execution token space exhausted"),
        }
    }
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
    fn binding_policy(&self) -> crate::core::action::identity::BindingPolicy {
        crate::core::action::identity::BindingPolicy::Unavailable {
            reason: "Menu identity is not verified for persistent shortcuts",
        }
    }
    fn records_usage(&self) -> bool {
        // AXIdentifier and semantic paths can be reused by a different command.
        // Retain existing rows, but quarantine them until a durable identity exists.
        false
    }

    fn execution_token(&self) -> Option<u64> {
        Some(self.execution_token)
    }

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
