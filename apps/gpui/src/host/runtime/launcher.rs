use super::*;
use anywhere_protocol::ContextCapturePresentation;

impl LiveLauncher {
    pub fn reserve_activation(&self) -> Result<SessionId> {
        let session_id = self
            .application
            .application()
            .begin_activation()
            .context("reserve launcher session")?;
        self.session_id.store(session_id, Ordering::Release);
        Ok(session_id)
    }

    pub fn activate_session(
        &self,
        session_id: SessionId,
        input_mode: InputMode,
    ) -> Receiver<LiveEvent> {
        let application = self.application.application().clone();
        self.spawn(async move {
            match application
                .activate_session_in_mode(session_id, false, input_mode)
                .await
            {
                Ok(Some(state)) => LiveEvent::SharedState(Arc::new(state)),
                Ok(None) => LiveEvent::Error("Launcher session was superseded".to_owned()),
                Err(error) => LiveEvent::Error(format!("{error:#}")),
            }
        })
    }

    pub fn resume_current(&self) -> Receiver<LiveEvent> {
        let application = self.application.application().clone();
        let session_id = self.session_id();
        self.spawn(async move {
            match application.resume_session(session_id) {
                Ok(Some(state)) => LiveEvent::SharedState(Arc::new(state)),
                Ok(None) => {
                    LiveEvent::Error("Launcher session expired before it could resume".to_owned())
                }
                Err(error) => LiveEvent::Error(format!("{error:#}")),
            }
        })
    }

    pub fn choose_activation_input(
        &self,
        captures: Vec<ContextCapturePresentation>,
        now_ms: i64,
        selected_enabled: bool,
        clipboard_enabled: bool,
        allow_pending_selection: bool,
    ) -> Receiver<LiveEvent> {
        let application = self.application.application().clone();
        self.spawn_blocking(move || {
            Ok(LiveEvent::ActivationInput(
                application
                    .choose_activation_input(
                        &captures,
                        now_ms,
                        selected_enabled,
                        clipboard_enabled,
                        allow_pending_selection,
                    )
                    .map(|input| ActivationInput {
                        source: input.source,
                        text: input.text,
                        valid_until_ms: input.valid_until_ms,
                    }),
            ))
        })
    }

    pub fn suspend_current_now(&self) -> Result<()> {
        let session_id = self.session_id();
        if session_id == 0 {
            return Ok(());
        }
        // This method is called from GPUI/AppKit's main thread. Core may
        // schedule follow-up work with `tokio::spawn`, so enter the runtime
        // that owns `ApplicationRuntime` before crossing the synchronous API.
        let _runtime = self.runtime().enter();
        self.application
            .application()
            .suspend_session_now(session_id)
    }

    pub fn discard_session_now(&self, session_id: SessionId) -> Result<()> {
        let _runtime = self.runtime().enter();
        self.application.application().deactivate(session_id)
    }

    pub fn update_input(
        &self,
        input: String,
        revision: Revision,
        scope: Option<GroupId>,
        mode: InputMode,
    ) -> Receiver<LiveEvent> {
        let application = self.application.application().clone();
        let session_id = self.session_id();
        self.spawn(async move {
            match application
                .update_context_input(session_id, input, revision, scope, mode)
                .await
            {
                Ok(Some(state)) => LiveEvent::SharedState(Arc::new(state)),
                Ok(None) => LiveEvent::Ignored,
                Err(error) => LiveEvent::Error(format!("{error:#}")),
            }
        })
    }

    pub fn update_file_input(
        &self,
        input: String,
        revision: Revision,
        filter: anywhere_protocol::FileSearchFilter,
    ) -> Receiver<LiveEvent> {
        let application = self.application.application().clone();
        let session_id = self.session_id();
        self.spawn(async move {
            match application
                .update_file_search_input(session_id, input, revision, filter)
                .await
            {
                Ok(Some(state)) => LiveEvent::SharedState(Arc::new(state)),
                Ok(None) => LiveEvent::Ignored,
                Err(error) => LiveEvent::Error(format!("{error:#}")),
            }
        })
    }

    /// Direct watch subscription to Core's shared snapshot stream.
    /// GPUI awaits `watch::Receiver::changed()` directly on its own executor,
    /// avoiding an intermediate Tokio relay task and `mpsc` hop.
    pub fn subscribe_snapshots(
        &self,
    ) -> (
        SessionId,
        tokio::sync::watch::Receiver<Option<Arc<LauncherState>>>,
    ) {
        (
            self.session_id(),
            self.application.application().state_updates(),
        )
    }

    pub fn is_session_active(&self, session_id: SessionId) -> bool {
        self.application.application().is_session_active(session_id)
    }

    /// Follow every later snapshot for this session. Kept for host tests that
    /// exercise the channel adapter explicitly.
    #[cfg(test)]
    pub fn follow_snapshots(&self, after: u64) -> Receiver<LiveEvent> {
        let application = self.application.application().clone();
        let session_id = self.session_id();
        let (sender, receiver) = channel(1);
        self.runtime().spawn(async move {
            let mut cursor = after;
            let mut updates = application.state_updates();
            loop {
                let next = {
                    let current = updates.borrow_and_update();
                    current.as_ref().and_then(|state| {
                        let state_cursor = snapshot_cursor(state);
                        (state.session_id == session_id && state_cursor > cursor)
                            .then(|| state.clone())
                    })
                };
                if let Some(state) = next {
                    cursor = snapshot_cursor(&state);
                    if sender.send(LiveEvent::SharedState(state)).await.is_err() {
                        break;
                    }
                }
                if !application.is_session_active(session_id) {
                    let _ = sender.send(LiveEvent::Ignored).await;
                    break;
                }
                tokio::select! {
                    _closed = sender.closed() => {
                        break;
                    }
                    changed = updates.changed() => {
                        if changed.is_err() {
                            let _ = sender.send(LiveEvent::Ignored).await;
                            break;
                        }
                    }
                }
            }
        });
        receiver
    }
}
