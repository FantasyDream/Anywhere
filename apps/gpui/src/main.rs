#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

fn main() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("anywhere_gpui=info")),
        )
        .with_writer(std::io::stderr)
        .try_init();
    anywhere_gpui::platform::require_supported_host();
    let preview = std::env::var_os("ANYWHERE_PREVIEW").is_some();
    let startup_package = (!preview)
        .then(|| anywhere_gpui::plugin_open::launch_package(std::env::args_os().skip(1)))
        .flatten();
    let (packages, receiver) = tokio::sync::mpsc::channel(4);
    let Some(_instance) = anywhere_gpui::platform::acquire_application_instance(
        startup_package.as_deref(),
        packages.clone(),
    ) else {
        return;
    };
    if let Some(path) = startup_package {
        let _ = packages.try_send(path);
    }
    let gui = gpui_kit::application().with_assets(gpui_kit::assets::AllAssets);
    #[cfg(target_os = "macos")]
    if !preview {
        gui.on_open_urls(move |urls| {
            if let Some(path) = anywhere_gpui::plugin_open::opened_file_url(&urls) {
                let _ = packages.try_send(path);
            }
        });
    }
    gui.run(move |cx| anywhere_gpui::app::run(cx, receiver));
}
