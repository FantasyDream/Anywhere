use crate::core::action::descriptor::{BuiltinExecutor, CommandExecutor, CommandRegistry};
use crate::core::context::{ContextValue, ResolvedContext};
use anyhow::{bail, Context, Result};
use anywhere_platform::{GitOperation, Platform};

pub struct ExecutionOutput {
    pub text: Option<String>,
    pub close_window: bool,
}

pub fn execute(
    executor: &CommandExecutor,
    context: &ResolvedContext,
    platform: &dyn Platform,
    registry: &CommandRegistry,
) -> Result<ExecutionOutput> {
    match executor {
        CommandExecutor::Builtin(builtin) => execute_builtin(*builtin, context, platform),
        CommandExecutor::Declarative(action) => {
            use anywhere_protocol::DeclarativeAction::*;
            let value = context.value.text();
            match action {
                OpenUrl { url } => {
                    platform.open_url(&anywhere_plugin::interpolate(url, &value, true)?)?
                }
                OpenFile { path } => platform.open_file(std::path::Path::new(
                    &anywhere_plugin::interpolate(path, &value, false)?,
                ))?,
                Copy { text } => {
                    platform.copy_text(&anywhere_plugin::interpolate(text, &value, false)?)?
                }
                ExecuteBuiltin { command_id } => {
                    let command = registry
                        .get(command_id)
                        .context("unknown built-in command")?;
                    if !command.matches(context) {
                        bail!("built-in command does not accept this context");
                    }
                    let CommandExecutor::Builtin(builtin) = command.executor else {
                        bail!("execute_builtin target is not built-in");
                    };
                    return execute_builtin(builtin, context, platform);
                }
            }
            Ok(ExecutionOutput {
                text: None,
                close_window: true,
            })
        }
    }
}

fn execute_builtin(
    executor: BuiltinExecutor,
    context: &ResolvedContext,
    platform: &dyn Platform,
) -> Result<ExecutionOutput> {
    use BuiltinExecutor::*;
    let mut text = None;
    match executor {
        OpenDirectory => {
            platform.open_directory(context.value.path().context("directory context required")?)?
        }
        OpenTerminal => {
            platform.open_terminal(context.value.path().context("directory context required")?)?
        }
        OpenFile => platform.open_file(context.value.path().context("file context required")?)?,
        RevealFile => {
            platform.reveal_file(context.value.path().context("file context required")?)?
        }
        CopyPath => {
            let path = context.value.path().context("path context required")?;
            platform.copy_text(&path.to_string_lossy())?;
        }
        OpenBrowser => {
            let ContextValue::Url(url) = &context.value else {
                bail!("URL context required");
            };
            platform.open_url(url.as_str())?;
        }
        CopyUrl => {
            let ContextValue::Url(url) = &context.value else {
                bail!("URL context required");
            };
            platform.copy_text(url.as_str())?;
        }
        FormatJson | MinifyJson | CopyFormattedJson => {
            let ContextValue::Json { parsed, .. } = &context.value else {
                bail!("JSON context required");
            };
            let rendered = if matches!(executor, MinifyJson) {
                serde_json::to_string(parsed)?
            } else {
                serde_json::to_string_pretty(parsed)?
            };
            if matches!(executor, CopyFormattedJson) {
                platform.copy_text(&rendered)?;
            } else {
                text = Some(rendered);
            }
        }
        // Read-only Git reports go into the result preview; the window stays open.
        GitStatus | GitLog | GitDiff => {
            let repository = context
                .value
                .path()
                .context("repository context required")?;
            let operation = match executor {
                GitStatus => GitOperation::Status,
                GitLog => GitOperation::Log,
                _ => GitOperation::Diff,
            };
            let output = platform.run_git(repository, operation)?;
            text = Some(output.text);
        }
    }
    Ok(ExecutionOutput {
        close_window: text.is_none(),
        text,
    })
}
