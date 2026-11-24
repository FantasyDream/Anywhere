use super::*;

impl LiveLauncher {
    pub fn settings_snapshot(&self) -> Result<SettingsSnapshot> {
        anywhere_core::core::sync::lock_state(&self.settings_cache, "settings_cache")
            .map(|snapshot| snapshot.clone())
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

    pub fn retry_login_item(&self) -> Receiver<LiveEvent> {
        let explicit_data_directory = self.explicit_data_directory;
        let preferences = self.preferences_cache.clone();
        let preferences_write = self.preferences_write.clone();
        self.spawn(async move {
            if explicit_data_directory {
                return LiveEvent::LoginItemReconciled(None);
            }
            let _guard = preferences_write.lock().await;
            let Ok(desired) =
                anywhere_core::core::sync::lock_state(&preferences, "preferences_cache")
                    .map(|saved| saved.launch_at_login)
            else {
                return LiveEvent::Error("preferences cache is unavailable".into());
            };
            match tokio::task::spawn_blocking(move || apply_login_item(desired)).await {
                Ok(error) => LiveEvent::LoginItemReconciled(error),
                Err(error) => {
                    LiveEvent::Error(format!("login item worker could not finish: {error}"))
                }
            }
        })
    }

    pub fn set_preferences(&self, preferences: HostPreferences) -> Receiver<LiveEvent> {
        let locale = preferences.ui_language.locale();
        let application = self.application.clone();
        let path = self.preferences_path.clone();
        let explicit_data_directory = self.explicit_data_directory;
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
                *anywhere_core::core::sync::lock_cache(&access_error, "search_access_error") = None;
                let host_error = if !explicit_data_directory
                    && previous.launch_at_login != preferences.launch_at_login
                {
                    apply_login_item(preferences.launch_at_login)
                } else {
                    None
                };
                let snapshot = settings_snapshot_from(&application)?;
                *anywhere_core::core::sync::lock_state(&preferences_cache, "preferences_cache")? =
                    preferences.clone();
                *anywhere_core::core::sync::lock_state(&settings_cache, "settings_cache")? =
                    snapshot;
                Ok(LiveEvent::Preferences {
                    preferences,
                    host_error,
                })
            })
            .await;
            let event = match event {
                Ok(Ok(event)) => event,
                Ok(Err(error)) => LiveEvent::Error(
                    super::super::directory_access::localized_error(&error, locale),
                ),
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
        let path = self.preferences_path.clone();
        let preferences_write = self.preferences_write.clone();
        let locale = self
            .preferences()
            .map(|p| p.ui_language.locale())
            .unwrap_or(crate::l10n::Locale::En);
        self.spawn(async move {
            let _guard = preferences_write.lock().await;
            match tokio::task::spawn_blocking(move || {
                let saved = HostPreferences::load(&path)?;
                let selected = selected.into_iter().map(PathBuf::from).collect::<Vec<_>>();
                directory_access.authorize(&selected, &roots, &saved.file_search_roots)
            })
            .await
            {
                Ok(Ok(roots)) => LiveEvent::DirectoriesAuthorized { roots },
                Ok(Err(error)) => LiveEvent::Error(
                    super::super::directory_access::localized_error(&error, locale),
                ),
                Err(error) => LiveEvent::Error(format!(
                    "directory authorization worker could not finish: {error}"
                )),
            }
        })
    }
}

fn apply_login_item(desired: bool) -> Option<String> {
    login_item::apply(desired)
        .err()
        .map(|error| format!("{error:#}"))
}
