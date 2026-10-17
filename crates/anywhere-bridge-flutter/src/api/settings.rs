//! Bridge settings adapter; exported names remain in the api facade.
use super::*;

pub(super) fn initialize(data_directory: String, plugin_directory: String) -> Result<()> {
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
    let mut diagnostics = Vec::new();
    let history = match SqliteUsageRepository::open(&data.join("usage.sqlite3")) {
        Ok(history) => {
            Box::new(history) as Box<dyn anywhere_core::core::ranking::usage::UsageRepository>
        }
        Err(error) => {
            tracing::error!(error = %error, "usage_history_unavailable");
            diagnostics.push(format!(
                "Usage history is unavailable; this session will not be saved: {error:#}"
            ));
            Box::new(anywhere_core::core::ranking::usage::MemoryUsage::default())
        }
    };
    let user_plugins = data.join("plugins");
    std::fs::create_dir_all(&user_plugins)?;
    let app = Application::new(
        anywhere_platform::native_platform()?,
        anywhere_core::application::builtin_registry(),
        history,
        diagnostics,
    );
    let manager = anywhere_core::application::settings_service::SettingsService::open(
        data.join("extensions.json"),
        vec![PathBuf::from(plugin_directory), user_plugins],
        &app,
    )?;
    APPLICATION
        .set(app)
        .map_err(|_| anyhow::anyhow!("application already initialized"))?;
    EXTENSIONS
        .set(Mutex::new(manager))
        .map_err(|_| anyhow::anyhow!("extensions already initialized"))?;
    Ok(())
}

pub(super) fn extensions() -> Result<
    std::sync::MutexGuard<'static, anywhere_core::application::settings_service::SettingsService>,
> {
    EXTENSIONS
        .get()
        .context("call initialize first")?
        .lock()
        .map_err(|_| anyhow::anyhow!("extensions unavailable"))
}
pub(super) fn extension_status() -> Result<Vec<ExtensionStatusDto>> {
    Ok(extensions()?
        .statuses()
        .into_iter()
        .map(Into::into)
        .collect())
}
pub(super) fn change_extension(
    change: anywhere_core::application::settings_service::SettingsChange,
) -> Result<Vec<ExtensionStatusDto>> {
    Ok(extensions()?
        .change(application()?, change)?
        .into_iter()
        .map(Into::into)
        .collect())
}
fn plugin_reports(
    statuses: Vec<anywhere_core::application::settings_service::ModuleStatus>,
) -> Vec<PluginStatusDto> {
    statuses
        .into_iter()
        .filter(|s| !s.first_party)
        .map(|s| PluginStatusDto {
            id: s.id,
            ok: s.active,
            detail: s.detail,
        })
        .collect()
}
pub(super) fn plugin_status() -> Vec<PluginStatusDto> {
    extensions()
        .map(|m| plugin_reports(m.statuses()))
        .unwrap_or_default()
}
pub(super) fn refresh_features() -> Result<Vec<ExtensionStatusDto>> {
    Ok(extensions()?
        .refresh(application()?)?
        .into_iter()
        .map(Into::into)
        .collect())
}
pub(super) fn refresh_plugins() -> Result<Vec<PluginStatusDto>> {
    Ok(plugin_reports(extensions()?.refresh(application()?)?))
}

pub(super) fn excluded_apps() -> Result<Vec<String>> {
    Ok(application()?.excluded_apps())
}

pub(super) fn set_excluded_apps(bundle_ids: Vec<String>) -> Result<()> {
    application()?.set_excluded_apps(bundle_ids);
    Ok(())
}

pub(super) fn set_file_search(
    enabled: bool,
    roots: Vec<String>,
    excluded_directory_names: Vec<String>,
    engine: FileSearchEngineDto,
    include_hidden: bool,
    restrict_to_roots: bool,
) -> Result<()> {
    application()?.configure_file_search(
        enabled,
        anywhere_platform::FileSearchOptions {
            roots: roots.into_iter().map(PathBuf::from).collect(),
            restrict_to_roots,
            excluded_directory_names,
            engine: match engine {
                FileSearchEngineDto::SystemIndex => {
                    anywhere_platform::FileSearchEngine::SystemIndex
                }
                FileSearchEngineDto::Ripgrep => anywhere_platform::FileSearchEngine::Ripgrep,
                FileSearchEngineDto::Combined => anywhere_platform::FileSearchEngine::Combined,
            },
            include_hidden,
        },
    )
}

pub(super) fn set_context_sources(
    finder_selection: bool,
    selected_text: bool,
    browser_current_page: bool,
    clipboard: bool,
) -> Result<()> {
    application()?.set_capture_sources(
        finder_selection,
        selected_text,
        browser_current_page,
        clipboard,
    );
    Ok(())
}
