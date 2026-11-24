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
    #[cfg(windows)]
    let Some(_instance) = anywhere_gpui::platform::windows::acquire_instance() else {
        return;
    };
    gpui_kit::application()
        .with_assets(gpui_kit::assets::AllAssets)
        .run(anywhere_gpui::app::run);
}
