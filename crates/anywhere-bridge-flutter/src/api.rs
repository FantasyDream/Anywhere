use anyhow::{Context, Result};
use anywhere_core::{persistence::usage::SqliteUsageRepository, Application, LauncherApplication};
use anywhere_protocol as protocol;
use std::{
    path::Path,
    sync::{Arc, OnceLock},
    time::Duration,
};

static APPLICATION: OnceLock<Application> = OnceLock::new();

/// Longest a single `wait_for_snapshot` call may block before Dart polls again.
const SNAPSHOT_WAIT: Duration = Duration::from_secs(30);

pub fn ping() -> String {
    anywhere_core::ping().to_owned()
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
    let history =
        SqliteUsageRepository::open(&data.join("usage.sqlite3")).context("open usage history")?;
    let mut registry = anywhere_core::application::composition::builtin_registry();
    let mut diagnostics = Vec::new();
    let user_plugins = data.join("plugins");
    std::fs::create_dir_all(&user_plugins)?;
    for directory in [Path::new(&plugin_directory), user_plugins.as_path()] {
        let discovery = anywhere_plugin::discover(directory);
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
    let platform = Arc::new(anywhere_platform::MacOsPlatform);
    #[cfg(target_os = "windows")]
    let platform = Arc::new(anywhere_platform::WindowsPlatform::default());
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

/// Reserve synchronously so Dart can cancel even before foreground capture finishes.
#[flutter_rust_bridge::frb(sync)]
pub fn begin_activation() -> Result<u32> {
    application()?.begin_activation()
}

pub async fn activate(
    session_id: u32,
    prompt_accessibility: bool,
) -> Result<Option<LauncherStateDto>> {
    Ok(application()?
        .activate_session_with(session_id, prompt_accessibility)
        .await?
        .map(Into::into))
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

/// Latest committed snapshot for a session, read straight from the core.
///
/// The core is the only source of truth: a late pass publishes there, and nothing
/// here has to be kept in step with it.
pub fn latest_state(session_id: u32) -> Option<LauncherStateDto> {
    application().ok()?.latest_state(session_id).map(Into::into)
}

/// A snapshot position that is monotonic for the whole session.
///
/// `snapshot_seq` alone is not monotonic: the core restarts it at 1 for every revision, so
/// a late snapshot of an older revision can carry a higher sequence than the newest
/// revision's first snapshot. Pairing the two makes the order unambiguous, which is what
/// this wait needs. The Dart loop builds the same value from the fields it already has
/// (`revision` and `snapshotSeq`), which is why it stays private to this module.
fn cursor(state: &protocol::LauncherState) -> u64 {
    ((state.revision as u64) << 32) | state.snapshot_seq as u64
}

/// Block until the core publishes a snapshot beyond `after` for this session.
///
/// Late context is committed by a background task, so the core's own update channel is the
/// only place a caller can observe it. Returns the newest snapshot when one arrives, or
/// `None` on timeout, so the caller can wait again from the same cursor — a wait that ends
/// because nothing happened must not be mistaken for the session being finished.
pub async fn wait_for_snapshot(session_id: u32, after: u64) -> Result<Option<LauncherStateDto>> {
    let app = application()?;
    if let Some(state) = beyond(app, session_id, after) {
        return Ok(Some(state.into()));
    }
    let mut updates = app.state_updates();
    let deadline = tokio::time::Instant::now() + SNAPSHOT_WAIT;
    loop {
        if !app.is_session_active(session_id) {
            return Ok(None);
        }
        if let Some(state) = beyond(app, session_id, after) {
            return Ok(Some(state.into()));
        }
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            break;
        }
        match tokio::time::timeout(remaining, updates.changed()).await {
            // A newer snapshot for another session is fine: the loop re-checks ours.
            Ok(Ok(())) => continue,
            // Sender gone: the core is shutting down, so report nothing.
            Ok(Err(_)) => return Ok(None),
            // The wait elapsed. Nothing newer exists; the caller waits again.
            Err(_) => break,
        }
    }
    Ok(None)
}

/// The committed snapshot for a session, when it is beyond `after`.
fn beyond(app: &Application, session_id: u32, after: u64) -> Option<protocol::LauncherState> {
    app.latest_state(session_id)
        .filter(|state| cursor(state) > after)
}

pub async fn execute(
    session_id: u32,
    revision: u32,
    candidate_id: String,
) -> Result<ActionResultDto> {
    Ok(application()?
        .execute(session_id, revision, candidate_id)
        .await?
        .into())
}

/// Small PNG for an application candidate, or `None` when there is no icon.
pub fn application_icon(candidate_id: String) -> Result<Option<Vec<u8>>> {
    let result = application()?.application_icon(&candidate_id);
    match &result {
        Ok(Some(bytes)) => tracing::debug!(candidate_id, bytes = bytes.len(), "app_icon_served"),
        Ok(None) => tracing::info!(candidate_id, "app_icon_absent"),
        Err(error) => tracing::error!(candidate_id, error = %error, "app_icon_failed"),
    }
    result
}

pub fn deactivate(session_id: u32) -> Result<()> {
    application()?.deactivate(session_id)
}

#[flutter_rust_bridge::frb(sync)]
pub fn prompt_accessibility() -> Result<bool> {
    application()?.prompt_accessibility()
}

// Explicit conversion keeps generated/Flutter types out of the protocol and core.
#[derive(Clone, Debug)]
pub struct LauncherStateDto {
    pub session_id: u32,
    pub revision: u32,
    pub snapshot_seq: u32,
    pub input: String,
    pub context_complete: bool,
    pub contexts: Vec<ContextDto>,
    pub candidates: Vec<CandidateDto>,
    pub scopes: Vec<ScopeDto>,
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
pub struct CandidateDto {
    pub id: String,
    /// `application` or `action`, mirroring `CandidateKind`.
    pub kind: String,
    pub title: String,
    pub subtitle: Option<String>,
    pub shortcut: Option<String>,
    pub icon_key: String,
    pub score: f32,
    pub group_id: String,
    /// `enabled`, `disabled` or `hidden`.
    pub availability: String,
    pub availability_reason: Option<String>,
    pub command_id: Option<String>,
    pub bundle_id: Option<String>,
}
#[derive(Clone, Debug)]
pub struct ScopeDto {
    pub id: String,
    /// `all`, `app` or `group`.
    pub kind: String,
    pub label: String,
    pub candidate_ids: Vec<String>,
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
            snapshot_seq: state.snapshot_seq,
            input: state.input,
            context_complete: state.context_complete,
            contexts: state
                .context_summary
                .into_iter()
                .map(|context| ContextDto {
                    id: context.id,
                    type_id: context.type_id,
                    label: context.label,
                    detail: context.detail,
                })
                .collect(),
            candidates: state
                .candidates
                .into_iter()
                .map(|candidate| CandidateDto {
                    id: candidate.id,
                    kind: kind_name(candidate.kind).into(),
                    title: candidate.title,
                    subtitle: candidate.subtitle,
                    shortcut: candidate.shortcut,
                    icon_key: candidate.icon.key,
                    score: candidate.score,
                    group_id: candidate.group_id,
                    availability: availability_state(candidate.availability.state).into(),
                    availability_reason: candidate.availability.reason,
                    command_id: candidate.command_id,
                    bundle_id: candidate.bundle_id,
                })
                .collect(),
            scopes: state
                .scopes
                .into_iter()
                .map(|scope| ScopeDto {
                    id: scope.id,
                    kind: scope_kind_name(scope.kind).into(),
                    label: scope.label,
                    candidate_ids: scope.candidate_ids,
                })
                .collect(),
            diagnostics: state.diagnostics,
        }
    }
}

fn kind_name(kind: protocol::CandidateKind) -> &'static str {
    match kind {
        protocol::CandidateKind::Application => "application",
        protocol::CandidateKind::Action => "action",
    }
}

fn availability_state(state: protocol::AvailabilityState) -> &'static str {
    match state {
        protocol::AvailabilityState::Enabled => "enabled",
        protocol::AvailabilityState::Disabled => "disabled",
        protocol::AvailabilityState::Hidden => "hidden",
    }
}

fn scope_kind_name(kind: protocol::ScopeKind) -> &'static str {
    match kind {
        protocol::ScopeKind::App => "app",
        protocol::ScopeKind::Group => "group",
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
