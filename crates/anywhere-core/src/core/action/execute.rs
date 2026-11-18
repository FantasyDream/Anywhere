use super::descriptor::{ActionExecutor, ActionRegistry};
use super::handler::ExecutionOutput;
use crate::core::context::ResolvedContext;
use anyhow::Result;
use anywhere_platform::Platform;

pub fn execute(
    executor: &ActionExecutor,
    context: &ResolvedContext,
    platform: &dyn Platform,
    _registry: &ActionRegistry,
) -> Result<ExecutionOutput> {
    match executor {
        ActionExecutor::WebCommand(command) => Ok(ExecutionOutput::web(command.clone())),
        ActionExecutor::Native(handler) => handler.execute(context, platform),
    }
}
