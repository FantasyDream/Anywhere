use super::*;

impl LiveLauncher {
    pub fn settings_snapshot(&self) -> Result<SettingsSnapshot> {
        self.settings_cache
            .lock()
            .map(|snapshot| snapshot.clone())
            .map_err(|_| anyhow::anyhow!("settings cache is unavailable"))
    }

    pub fn set_clipboard_recording(&self, enabled: bool) -> Receiver<LiveEvent> {
        let application = self.application.clone();
        self.spawn_settings(move || {
            let settings = application.settings().map_err(anyhow::Error::new)?;
            settings
                .clipboard()
                .request_recording(enabled, application.application().platform())
        })
    }

    pub fn change_feature(&self, key: String, enabled: bool) -> Receiver<LiveEvent> {
        let application = self.application.clone();
        self.spawn_settings(move || {
            let mut settings = application.settings().map_err(anyhow::Error::new)?;
            settings
                .change(
                    application.application(),
                    anywhere_core::application::settings_service::SettingsChange::Enable {
                        key,
                        enabled,
                    },
                )
                .map(|_| ())
        })
    }

    pub fn set_preferences(&self, preferences: HostPreferences) -> Receiver<LiveEvent> {
        let application = self.application.clone();
        let path = self.preferences_path.clone();
        let directory_access = self.directory_access.clone();
        let access_error = self.search_access_error.clone();
        let preferences_write = self.preferences_write.clone();
        let preferences_cache = self.preferences_cache.clone();
        let settings_cache = self.settings_cache.clone();
        let (sender, receiver) = channel(1);
        self.runtime().spawn(async move {
            let _guard = preferences_write.lock().await;
            let event = tokio::task::spawn_blocking(move || {
                let previous = HostPreferences::load(&path)?;
                let roots = if preferences.file_search_enabled {
                    directory_access.reconcile(&preferences.file_search_roots)?
                } else {
                    directory_access.clear();
                    Vec::new()
                };
                let rollback = || -> Result<()> {
                    let previous_roots = if previous.file_search_enabled {
                        directory_access.reconcile(&previous.file_search_roots)?
                    } else {
                        directory_access.clear();
                        Vec::new()
                    };
                    apply_preferences(&application, &previous, &previous_roots)
                };
                if let Err(error) = apply_preferences(&application, &preferences, &roots) {
                    return match rollback() {
                        Ok(()) => Err(error),
                        Err(rollback_error) => Err(error.context(format!(
                            "preferences rollback also failed: {rollback_error:#}"
                        ))),
                    };
                }
                if let Err(error) = preferences.save(&path) {
                    return match rollback() {
                        Ok(()) => Err(error.context("persist preferences")),
                        Err(rollback_error) => Err(error.context(format!(
                            "persist preferences; rollback also failed: {rollback_error:#}"
                        ))),
                    };
                }
                if let Ok(mut error) = access_error.lock() {
                    *error = None;
                }
                let host_error = login_item::apply(preferences.launch_at_login)
                    .err()
                    .map(|error| format!("{error:#}"));
                let snapshot = settings_snapshot_from(&application)?;
                *preferences_cache
                    .lock()
                    .map_err(|_| anyhow::anyhow!("preferences cache is unavailable"))? =
                    preferences.clone();
                *settings_cache
                    .lock()
                    .map_err(|_| anyhow::anyhow!("settings cache is unavailable"))? = snapshot;
                Ok(LiveEvent::Preferences {
                    preferences,
                    host_error,
                })
            })
            .await;
            let event = match event {
                Ok(Ok(event)) => event,
                Ok(Err(error)) => LiveEvent::Error(format!("{error:#}")),
                Err(error) => {
                    LiveEvent::Error(format!("preferences worker could not finish: {error}"))
                }
            };
            let _ = sender.send(event).await;
        });
        receiver
    }

    pub fn authorize_search_directories(
        &self,
        selected: Vec<String>,
        roots: Vec<String>,
    ) -> Receiver<LiveEvent> {
        let directory_access = self.directory_access.clone();
        let access_error = self.search_access_error.clone();
        self.spawn_blocking(move || {
            let selected = selected.into_iter().map(PathBuf::from).collect::<Vec<_>>();
            let roots = directory_access.authorize(&selected, &roots)?;
            if let Ok(mut error) = access_error.lock() {
                *error = None;
            }
            Ok(LiveEvent::DirectoriesAuthorized { roots })
        })
    }
}
