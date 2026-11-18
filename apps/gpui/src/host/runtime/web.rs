use super::*;

impl LiveLauncher {
    pub fn web_packages(&self) -> Receiver<LiveEvent> {
        let application = self.application.clone();
        self.spawn_blocking(move || Ok(LiveEvent::WebPackages(web_packages_from(&application)?)))
    }

    pub fn inspect_web_package(&self, path: String) -> Receiver<LiveEvent> {
        let application = self.application.clone();
        self.spawn_blocking(move || {
            let review = application.inspect_web_package(std::path::Path::new(&path))?;
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
                Ok(frame) => LiveEvent::WebFrameReply { handle, frame },
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
