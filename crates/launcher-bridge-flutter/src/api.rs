use anyhow::{Context, Result};
use launcher_core::{
    command::CommandRegistry, history::SqliteHistory, Application, LauncherApplication,
};
use launcher_protocol as protocol;
use std::{
    path::Path,
    sync::{Arc, OnceLock},
};

static APPLICATION: OnceLock<Application> = OnceLock::new();

pub fn ping() -> String {
    launcher_core::ping().to_owned()
}

pub fn initialize(data_directory: String, plugin_directory: String) -> Result<()> {
    if APPLICATION.get().is_some() {
        return Ok(());
    }
    let _ = tracing_subscriber::fmt()
        .json()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .with_writer(std::io::stderr)
        .try_init();
    let data = Path::new(&data_directory);
    std::fs::create_dir_all(data)?;
    let history = SqliteHistory::open(&data.join("usage.sqlite3")).context("open usage history")?;
    let mut registry = CommandRegistry::builtin();
    let mut diagnostics = Vec::new();
    let user_plugins = data.join("plugins");
    std::fs::create_dir_all(&user_plugins)?;
    for directory in [Path::new(&plugin_directory), user_plugins.as_path()] {
        let discovery = launcher_plugin::discover(directory);
        diagnostics.extend(discovery.diagnostics);
        for manifest in discovery.manifests {
            let id = manifest.id.clone();
            if let Err(error) = registry.register_plugin(manifest) {
                tracing::error!(plugin_id = id, error = %error, "plugin_load_failure");
                diagnostics.push(format!("Plugin {id}: {error:#}"));
            }
        }
    }
    #[cfg(target_os = "macos")]
    let platform = Arc::new(launcher_platform::MacOsPlatform);
    #[cfg(target_os = "windows")]
    let platform = Arc::new(launcher_platform::WindowsPlatform::default());
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    anyhow::bail!("The host supports macOS and Windows only");
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    APPLICATION
        .set(Application::new(
            platform,
            registry,
            Box::new(history),
            diagnostics,
        ))
        .map_err(|_| anyhow::anyhow!("application already initialized"))?;
    Ok(())
}

fn application() -> Result<&'static Application> {
    APPLICATION.get().context("call initialize first")
}

pub async fn activate() -> Result<Option<LauncherStateDto>> {
    Ok(application()?.activate().await?.map(Into::into))
}
pub async fn update_input(
    session_id: u32,
    input: String,
    revision: u32,
) -> Result<Option<LauncherStateDto>> {
    Ok(application()?
        .update_input(session_id, input, revision)
        .await?
        .map(Into::into))
}
pub async fn execute(session_id: u32, action_id: String) -> Result<ActionResultDto> {
    Ok(application()?.execute(session_id, action_id).await?.into())
}
pub fn deactivate(session_id: u32) -> Result<()> {
    application()?.deactivate(session_id)
}

// Explicit conversion keeps generated/Flutter types out of the protocol and core.
#[derive(Clone, Debug)]
pub struct LauncherStateDto {
    pub session_id: u32,
    pub revision: u32,
    pub input: String,
    pub contexts: Vec<ContextDto>,
    pub actions: Vec<ActionDto>,
    pub diagnostics: Vec<String>,
}
#[derive(Clone, Debug)]
pub struct ContextDto {
    pub id: String,
    pub type_id: String,
    pub label: String,
    pub detail: String,
}
#[derive(Clone, Debug)]
pub struct ActionDto {
    pub id: String,
    pub command_id: String,
    pub title: String,
    pub subtitle: Option<String>,
    pub icon_key: String,
    pub score: f32,
}
#[derive(Clone, Debug)]
pub struct ActionResultDto {
    pub session_id: u32,
    pub revision: u32,
    pub success: bool,
    pub message: String,
    pub output_text: Option<String>,
    pub close_window: bool,
}

impl From<protocol::LauncherState> for LauncherStateDto {
    fn from(state: protocol::LauncherState) -> Self {
        Self {
            session_id: state.session_id,
            revision: state.revision,
            input: state.input,
            contexts: state
                .context_summary
                .into_iter()
                .map(|c| ContextDto {
                    id: c.id,
                    type_id: c.type_id,
                    label: c.label,
                    detail: c.detail,
                })
                .collect(),
            actions: state
                .actions
                .into_iter()
                .map(|a| ActionDto {
                    id: a.id,
                    command_id: a.command_id,
                    title: a.title,
                    subtitle: a.subtitle,
                    icon_key: a.icon.key,
                    score: a.score,
                })
                .collect(),
            diagnostics: state.diagnostics,
        }
    }
}
impl From<protocol::ActionResult> for ActionResultDto {
    fn from(result: protocol::ActionResult) -> Self {
        Self {
            session_id: result.session_id,
            revision: result.revision,
            success: result.success,
            message: result.message,
            output_text: result.output_text,
            close_window: result.close_window,
        }
    }
}
