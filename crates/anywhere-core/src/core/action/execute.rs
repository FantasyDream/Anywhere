use super::descriptor::{ActionExecutor, ActionRegistry};
use super::handler::ExecutionOutput;
use crate::core::context::ResolvedContext;
use anyhow::{bail, Context, Result};
use anywhere_platform::Platform;

pub fn execute(
    executor: &ActionExecutor,
    context: &ResolvedContext,
    platform: &dyn Platform,
    registry: &ActionRegistry,
) -> Result<ExecutionOutput> {
    match executor {
        ActionExecutor::Native(handler) => handler.execute(context, platform),
        ActionExecutor::Declarative(action) => {
            execute_declarative(action, context, platform, registry)
        }
    }
}

fn execute_declarative(
    action: &anywhere_protocol::DeclarativeAction,
    context: &ResolvedContext,
    platform: &dyn Platform,
    registry: &ActionRegistry,
) -> Result<ExecutionOutput> {
    use anywhere_protocol::DeclarativeAction::*;
    let value = context.value.text();
    match action {
        OpenUrl { url } => platform.open_url(&anywhere_plugin::interpolate(url, &value, true)?)?,
        OpenFile { path } => platform.open_file(std::path::Path::new(
            &anywhere_plugin::interpolate(path, &value, false)?,
        ))?,
        Copy { text } => platform.copy_text(&anywhere_plugin::interpolate(text, &value, false)?)?,
        ExecuteBuiltin { command_id } => {
            let action = registry
                .get(command_id)
                .context("unknown built-in action")?;
            if !action.matches(context) {
                bail!("built-in action does not accept this context");
            }
            let ActionExecutor::Native(_) = action.executor else {
                bail!("execute_builtin target is not built-in");
            };
            return execute(&action.executor, context, platform, registry);
        }
    }
    Ok(ExecutionOutput::dismiss())
}
