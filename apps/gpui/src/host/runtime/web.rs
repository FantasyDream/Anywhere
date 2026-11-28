use super::*;

impl LiveLauncher {
    pub fn web_packages(&self) -> Receiver<LiveEvent> {
        let application = self.application.clone();
        self.spawn_blocking(move || Ok(LiveEvent::WebPackages(web_packages_from(&application)?)))
    }

    pub fn inspect_web_package(&self, path: std::path::PathBuf) -> Receiver<LiveEvent> {
        let application = self.application.clone();
        self.spawn_blocking(move || {
            let review = application.inspect_web_package(&path)?;
            use anywhere_protocol::web_plugin::WebPermission;
            let origins = review.permissions.iter().find_map(|permission| {
                if let WebPermission::UrlOpen { origins } = permission {
                    Some(origins)
                } else {
                    None
                }
            });
            let data_to_url = origins.is_some()
                && review.permissions.iter().any(|permission| {
                    matches!(
                        permission,
                        WebPermission::ResourcesRead {} | WebPermission::ClipboardReadText {}
                    )
                });
            let details = crate::host::WebReviewDetails {
                command_count: review.command_count,
                commands: review
                    .commands
                    .into_iter()
                    .map(|command| (command.title, command.localized_titles))
                    .collect(),
                resolver_count: review.resolver_count,
                background_actions: review.background_action_count,
                added: review
                    .permissions_added
                    .iter()
                    .map(web_permission_label)
                    .collect(),
                removed: review
                    .permissions_removed
                    .iter()
                    .map(web_permission_label)
                    .collect(),
                scope_changes: review
                    .permission_scopes
                    .iter()
                    .map(|(old, new)| (web_permission_label(old), web_permission_label(new)))
                    .collect(),
                data_to_url,
                any_origin: origins.is_some_and(|origins| origins == &["*"]),
                insecure_http: origins.is_some_and(|origins| {
                    origins.iter().any(|origin| {
                        origin
                            .split_once("://")
                            .is_some_and(|(scheme, _)| scheme.eq_ignore_ascii_case("http"))
                    })
                }),
            };
            Ok(LiveEvent::WebReview(WebPackageReview {
                token: review.token,
                id: review.id,
                name: review.name,
                version: review.version,
                digest: review.digest,
                permissions: review
                    .permissions
                    .iter()
                    .map(web_permission_label)
                    .collect(),
                updating: review.updating,
                same_version: review.same_version,
                details,
            }))
        })
    }

    pub fn cancel_web_review(&self, token: String) -> Receiver<LiveEvent> {
        let application = self.application.clone();
        self.spawn_blocking(move || {
            application.cancel_web_review(&token)?;
            Ok(LiveEvent::Ignored)
        })
    }

    pub fn approve_web_package(&self, token: String) -> Receiver<LiveEvent> {
        let application = self.application.clone();
        self.spawn_blocking(move || {
            application.approve_web_package(&token)?;
            Ok(LiveEvent::WebPackages(web_packages_from(&application)?))
        })
    }

    pub fn change_web_package(
        &self,
        id: String,
        operation: crate::host::WebPackageOperation,
    ) -> Receiver<LiveEvent> {
        let application = self.application.clone();
        self.spawn_blocking(move || {
            application.change_web_package(&id, operation)?;
            Ok(LiveEvent::WebPackages(web_packages_from(&application)?))
        })
    }

    pub fn open_web_document(
        &self,
        invocation_id: String,
        resume_handle: Option<String>,
    ) -> Receiver<LiveEvent> {
        let application = self.application.clone();
        self.spawn(async move {
            match application
                .open_web_document_view(&invocation_id, resume_handle.as_deref())
                .await
            {
                Ok(document) => LiveEvent::WebDocument(web_document(document)),
                Err(error) => LiveEvent::Error(format!("{error:#}")),
            }
        })
    }

    pub fn resume_web_document(&self, handle: String) -> Receiver<LiveEvent> {
        let application = self.application.clone();
        self.spawn(async move {
            match application
                .open_web_document_view("", Some(handle.as_str()))
                .await
            {
                Ok(document) => LiveEvent::WebDocument(web_document(document)),
                Err(error) => LiveEvent::Error(format!("{error:#}")),
            }
        })
    }

    pub fn close_web_document(&self, handle: String) -> Receiver<LiveEvent> {
        let application = self.application.clone();
        self.spawn_blocking(move || {
            application.close_web_document(&handle)?;
            Ok(LiveEvent::Ignored)
        })
    }

    pub fn dispatch_web_frame(
        &self,
        handle: String,
        origin: String,
        main_frame: bool,
        frame: String,
    ) -> Receiver<LiveEvent> {
        let application = self.application.clone();
        self.spawn(async move {
            match application
                .dispatch_web_frame(&handle, &origin, main_frame, &frame)
                .await
            {
                Ok(outcome) => LiveEvent::WebFrameReply {
                    handle,
                    frame: outcome.reply_frame,
                    host_action: outcome.host_action,
                },
                Err(error) => LiveEvent::Error(format!("{error:#}")),
            }
        })
    }

    pub fn wait_web_state(&self, handle: String, after: u64) -> Receiver<LiveEvent> {
        let application = self.application.clone();
        self.spawn(async move {
            match application.wait_web_state(&handle, after).await {
                Ok(Some(frame)) => LiveEvent::WebState { handle, frame },
                Ok(None) => LiveEvent::Ignored,
                Err(error) => LiveEvent::Error(format!("{error:#}")),
            }
        })
    }

    pub fn retain_preview(&self, id: String) -> Receiver<LiveEvent> {
        let application = self.application.application().clone();
        self.spawn_blocking(move || {
            application.retain_preview(&id)?;
            Ok(LiveEvent::Ignored)
        })
    }

    pub fn release_preview(&self, id: String) -> Receiver<LiveEvent> {
        let application = self.application.application().clone();
        self.spawn_blocking(move || {
            application.release_preview(&id)?;
            Ok(LiveEvent::Ignored)
        })
    }
}
