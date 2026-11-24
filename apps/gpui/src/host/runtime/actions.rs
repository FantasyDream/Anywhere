use super::*;

impl LiveLauncher {
    pub fn execute(&self, revision: Revision, candidate_id: String) -> Receiver<LiveEvent> {
        let application = self.application.application().clone();
        let session_id = self.session_id();
        self.spawn(async move {
            match application
                .execute(session_id, revision, candidate_id)
                .await
            {
                Ok(result) => LiveEvent::ActionResult(result),
                Err(error) => LiveEvent::Error(format!("{error:#}")),
            }
        })
    }

    pub fn shortcut_catalog(&self) -> Receiver<LiveEvent> {
        let application = self.application.application().clone();
        self.spawn_blocking(move || {
            Ok(LiveEvent::ShortcutCatalog(
                application
                    .shortcut_actions()?
                    .into_iter()
                    .map(|action| ShortcutCatalogAction {
                        action_id: action.action_id,
                        title: action.title,
                        localized_titles: action.localized_titles,
                        context_types: action.context_types,
                    })
                    .collect(),
            ))
        })
    }

    pub fn wait_action_shortcut(
        &self,
        session_id: SessionId,
        revision: Revision,
        binding: ActionShortcutBinding,
    ) -> Receiver<LiveEvent> {
        let application = self.application.application().clone();
        self.spawn(async move {
            let action_id = binding.action_id;
            let routes = binding
                .routes
                .into_iter()
                .map(|route| ShortcutRoute {
                    action_id: route.action_id,
                    app_bundle_id: route.app_bundle_id,
                    context_type: route.context_type,
                })
                .collect::<Vec<_>>();
            let resolution = if routes.is_empty() {
                application
                    .wait_action_binding(session_id, revision, &action_id)
                    .await
            } else {
                application
                    .wait_shortcut_intent(session_id, revision, &routes)
                    .await
            };
            let resolution = match resolution {
                Ok(resolution) => resolution,
                Err(error) => {
                    return LiveEvent::ActionShortcutFailed {
                        action_id,
                        message: format!("{error:#}"),
                    };
                }
            };
            LiveEvent::ActionShortcutReady {
                action_id,
                revision: resolution.state.revision,
                candidates: resolution
                    .candidates
                    .into_iter()
                    .map(|candidate| ShortcutCandidate {
                        candidate_id: candidate.id,
                        title: candidate.title,
                    })
                    .collect(),
            }
        })
    }

    pub fn application_icon(
        &self,
        candidate_id: String,
    ) -> tokio::sync::oneshot::Receiver<Option<anywhere_platform::RawIconImage>> {
        let application = self.application.application().clone();
        let (sender, receiver) = tokio::sync::oneshot::channel();
        self.runtime().spawn_blocking(move || {
            let image = match application.application_icon_bgra(&candidate_id) {
                Ok(image) => image,
                Err(error) => {
                    tracing::debug!(%error, candidate_id, "application_icon_unavailable");
                    None
                }
            };
            let _ = sender.send(image);
        });
        receiver
    }

    pub fn file_icon(
        &self,
        candidate_id: String,
    ) -> tokio::sync::oneshot::Receiver<Option<anywhere_platform::RawIconImage>> {
        let application = self.application.application().clone();
        let (sender, receiver) = tokio::sync::oneshot::channel();
        self.runtime().spawn_blocking(move || {
            let image = match application.file_icon_bgra(&candidate_id) {
                Ok(image) => image,
                Err(error) => {
                    tracing::debug!(%error, candidate_id, "file_icon_unavailable");
                    None
                }
            };
            let _ = sender.send(image);
        });
        receiver
    }

    pub fn object_actions(&self, revision: Revision, candidate_id: String) -> Receiver<LiveEvent> {
        let application = self.application.application().clone();
        let selection = application.select_action_object(self.session_id(), revision);
        self.spawn_blocking(move || {
            let state = application.file_actions_snapshot(selection, candidate_id)?;
            Ok(LiveEvent::SharedState(Arc::new(state)))
        })
    }

    pub fn load_more_files(&self, revision: Revision) -> Receiver<LiveEvent> {
        let application = self.application.application().clone();
        let session_id = self.session_id();
        self.spawn(async move {
            match application.load_more_files(session_id, revision).await {
                Ok(Some(state)) => LiveEvent::SharedState(Arc::new(state)),
                Ok(None) => LiveEvent::Ignored,
                Err(error) => LiveEvent::Error(format!("{error:#}")),
            }
        })
    }

    pub fn preview_file(&self, revision: Revision, candidate_id: String) -> Receiver<LiveEvent> {
        let application = self.application.application().clone();
        let session_id = self.session_id();
        self.spawn(async move {
            match application
                .preview_file(session_id, revision, candidate_id)
                .await
            {
                Ok(value) => LiveEvent::Preview(preview(value)),
                Err(error) => LiveEvent::Error(format!("{error:#}")),
            }
        })
    }

    pub fn preview_clipboard(&self, id: i64) -> Receiver<LiveEvent> {
        let application = self.application.clone();
        self.spawn_blocking(move || {
            let history = {
                let settings = application.settings().map_err(anyhow::Error::new)?;
                settings.clipboard()
            };
            Ok(LiveEvent::Preview(preview(
                application
                    .application()
                    .preview_clipboard(history.as_ref(), id)?,
            )))
        })
    }
}
