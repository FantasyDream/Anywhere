use super::*;
use anywhere_core::features::application_history::service::InteractiveSearch;

impl LiveLauncher {
    pub fn app_history_sources(&self) -> Receiver<LiveEvent> {
        let application = self.application.clone();
        self.spawn_blocking(move || {
            Ok(LiveEvent::AppHistorySources {
                request: 0,
                session_id: None,
                sources: app_history_sources_from(&application)?,
            })
        })
    }

    pub fn configure_app_history(
        &self,
        id: String,
        path: String,
        mode: SourceMode,
    ) -> Receiver<LiveEvent> {
        let application = self.application.clone();
        self.spawn_blocking(move || {
            {
                let mut settings = application.settings().map_err(anyhow::Error::new)?;
                settings.configure_application_history(
                    application.application(),
                    &id,
                    mode,
                    (!path.trim().is_empty()).then_some(path.as_str()),
                )?;
            }
            Ok(LiveEvent::AppHistorySources {
                request: 0,
                session_id: None,
                sources: app_history_sources_from(&application)?,
            })
        })
    }

    pub fn set_app_history_action_enabled(&self, id: String, enabled: bool) -> Receiver<LiveEvent> {
        let application = self.application.clone();
        self.spawn_blocking(move || {
            {
                let mut settings = application.settings().map_err(anyhow::Error::new)?;
                settings.set_application_history_action_enabled(
                    application.application(),
                    &id,
                    enabled,
                )?;
            }
            Ok(LiveEvent::AppHistorySources {
                request: 0,
                session_id: None,
                sources: app_history_sources_from(&application)?,
            })
        })
    }

    pub fn search_app_history(
        &self,
        session_id: u64,
        revision: u64,
        query: String,
        source_id: String,
        force_refresh: bool,
    ) -> Receiver<LiveEvent> {
        let application = self.application.clone();
        self.spawn(async move {
            let service = {
                let settings = match application.settings() {
                    Ok(settings) => settings,
                    Err(error) => return LiveEvent::Error(error.to_string()),
                };
                match settings.application_history() {
                    Ok(service) => service,
                    Err(error) => return LiveEvent::Error(format!("{error:#}")),
                }
            };
            match service
                .search_wait(InteractiveSearch {
                    owner: session_id,
                    revision,
                    query,
                    source_id: source_id.clone(),
                    force_refresh,
                })
                .await
            {
                Ok(result) => LiveEvent::AppHistory(AppHistorySnapshot {
                    source_id,
                    items: result
                        .items
                        .iter()
                        .map(|item| {
                            let kind = match &item.entry.target {
                                Target::Directory { .. } | Target::ObsidianVault { .. } => {
                                    "directory"
                                }
                                Target::File { .. } => "file",
                                Target::Workspace { .. } => "workspace",
                                Target::Remote { .. } => "remote",
                            };
                            AppHistoryItem {
                                id: item.id.clone(),
                                object_key: format!("{}:{}", item.source_id, item.entry.key),
                                source_id: item.source_id.clone(),
                                title: item.entry.title.clone(),
                                value: item.entry.target.text(),
                                kind,
                                can_open: !matches!(item.entry.target, Target::Remote { .. }),
                            }
                        })
                        .collect(),
                    failed_sources: result.failed_sources,
                    notice: result.notice,
                }),
                Err(error) => LiveEvent::Error(format!("{error:#}")),
            }
        })
    }

    pub fn open_app_history_session(&self, request: u64) -> Receiver<LiveEvent> {
        let application = self.application.clone();
        self.spawn_blocking(move || {
            let session_id = {
                let settings = application.settings().map_err(anyhow::Error::new)?;
                settings.application_history()?.open_session()?
            };
            Ok(LiveEvent::AppHistorySources {
                request,
                session_id: Some(session_id),
                sources: app_history_sources_from(&application)?,
            })
        })
    }

    pub fn cancel_app_history_session(&self, session_id: u64) {
        let application = self.application.clone();
        self.runtime().spawn_blocking(move || {
            let result = application
                .settings()
                .map_err(anyhow::Error::new)
                .and_then(|settings| settings.application_history())
                .map(|history| history.cancel_session(session_id));
            if let Err(error) = result {
                tracing::warn!(%error, session_id, "app_history_session_cancel_failed");
            }
        });
    }

    pub fn use_app_history_item(&self, id: String, copy: bool) -> Receiver<LiveEvent> {
        let application = self.application.clone();
        self.spawn_blocking(move || {
            let settings = application.settings().map_err(anyhow::Error::new)?;
            settings.application_history()?.use_item(
                &id,
                copy,
                application.application().platform(),
            )?;
            Ok(LiveEvent::AppHistoryAction { item_id: id, copy })
        })
    }

    pub fn app_history_object_actions(
        &self,
        request: u64,
        session_id: SessionId,
        revision: Revision,
        item_id: String,
    ) -> Receiver<LiveEvent> {
        let application = self.application.clone();
        self.spawn(async move {
            let history = match application
                .settings()
                .map_err(anyhow::Error::new)
                .and_then(|settings| settings.application_history())
            {
                Ok(history) => history,
                Err(error) => {
                    tracing::warn!(
                        %error,
                        item_id,
                        "app_history_object_actions_unavailable"
                    );
                    return LiveEvent::AppHistoryObjectActionsFailed { request };
                }
            };
            let actions = match application
                .application()
                .history_object_actions(history, session_id, revision, item_id.clone())
                .await
            {
                Ok(actions) => actions,
                Err(error) => {
                    tracing::warn!(%error, item_id, "app_history_object_actions_failed");
                    return LiveEvent::AppHistoryObjectActionsFailed { request };
                }
            };
            LiveEvent::AppHistoryObjectActions {
                request,
                item_id,
                revision,
                actions: actions
                    .into_iter()
                    .filter_map(app_history_object_action)
                    .collect(),
            }
        })
    }

    pub fn open_app_history_adapters_directory(&self) -> Receiver<LiveEvent> {
        let application = self.application.clone();
        self.spawn_blocking(move || {
            let directory = application
                .paths()
                .data_directory
                .join("application-history-sources");
            std::fs::create_dir_all(&directory)?;
            application
                .application()
                .platform()
                .open_directory(&directory)?;
            Ok(LiveEvent::AppHistoryAdaptersOpened {
                directory: directory.display().to_string(),
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn open_session_publishes_sources_without_reentering_the_settings_lock() {
        let directory = tempfile::tempdir().unwrap();
        let plugins = directory.path().join("plugins");
        std::fs::create_dir_all(&plugins).unwrap();
        let live = LiveLauncher::open_at(RuntimePaths::new(directory.path().join("data"), plugins))
            .unwrap();
        let mut events = live.open_app_history_session(17);

        let event = live
            .runtime()
            .block_on(async { tokio::time::timeout(Duration::from_secs(2), events.recv()).await })
            .expect("opening an application history session must not deadlock")
            .expect("application history session channel must stay open");

        match event {
            LiveEvent::AppHistorySources {
                request,
                session_id,
                ..
            } => {
                assert_eq!(request, 17);
                assert!(session_id.is_some());
            }
            other => panic!("unexpected application history event: {other:?}"),
        }
    }
}
