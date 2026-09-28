//! Activation: capture the environment, publish the first snapshot.
//
// The capture happens before the host is told to show anything, so the ambient context is
// frozen at the moment the hotkey was pressed rather than after focus moved.
use super::launcher_service::Application;
use super::session_model::{ActivationCapture, CaptureStatus, LauncherSession};
use super::session_runtime::Resolver;
use crate::core::context::CaptureMetadata;
use crate::core::session::facts::{FactValue, CLIPBOARD, GIT};
use crate::features::filesystem::provider::DirectoryMemo;
use anyhow::{Context, Result};
use anywhere_protocol::*;
use std::time::Instant;
use tracing::Instrument;

impl Application {
    /// Reserve a session before any asynchronous work. Hosts can cancel it immediately.
    pub fn begin_activation(&self) -> Result<SessionId> {
        let mut session = self.session()?;
        let session_id = session
            .state
            .session_id
            .checked_add(1)
            .context("session id exhausted; restart launcher")?;
        *session = crate::application::session_model::LauncherSession::reserved(
            session_id,
            self.resolver.base_registry(),
        );
        self.resolver.reset_session(session_id);
        Ok(session_id)
    }

    pub async fn activate_session(&self, session_id: SessionId) -> Result<Option<LauncherState>> {
        self.activate_session_with(session_id, false).await
    }

    pub async fn activate_session_with(
        &self,
        session_id: SessionId,
        prompt_accessibility: bool,
    ) -> Result<Option<LauncherState>> {
        if !self.resolver.is_current(session_id, 0) {
            return Ok(None);
        }
        {
            let mut session = self.session()?;
            if session.state.session_id != session_id || session.activation_started {
                return Ok(None);
            }
            session.activation_started = true;
        }
        let start = Instant::now();
        let platform = self.platform.clone();
        let frontmost = tokio::task::spawn_blocking(move || platform.frontmost_application())
            .await
            .context("foreground capture task failed")?
            .unwrap_or(None);
        if !self.resolver.is_current(session_id, 0) {
            return Ok(None);
        }
        let source_app = Self::source_app_of(frontmost.as_ref());
        {
            let mut session = self.session()?;
            if !self.resolver.is_current(session_id, 0) {
                return Ok(None);
            }
            session.identity.frontmost = frontmost.clone();
            session.identity.source_app = source_app.clone();
            session.supplemental_pending = true;
        }
        self.resolver.set_directory(
            frontmost
                .as_ref()
                .map(|app| DirectoryMemo::start(self.platform.clone(), app.clone())),
        );
        tracing::info!(
            session_id,
            duration_us = start.elapsed().as_micros() as u64,
            "activation_context_captured"
        );
        self.spawn_context_captures(session_id, frontmost.clone());
        self.prepare_app_actions(session_id, frontmost.as_ref(), prompt_accessibility)
            .await;
        let catalog_generation = self.spawn_catalog_scan();
        let supplemental = self.spawn_supplemental(session_id, source_app);
        let graph_started = Instant::now();
        let execution = self.resolver.execution(session_id, 0);
        let Some(seeds) = self.resolver.seeds_for(session_id, "") else {
            self.finish_supplemental(session_id);
            supplemental.abort();
            return Ok(None);
        };
        let run = self.resolver.build_run(&execution, seeds);
        let state = async {
            tracing::info!("session_created");
            let committed = self
                .publish_resolution(
                    session_id,
                    0,
                    run,
                    graph_started,
                    self.startup_diagnostics.clone(),
                )
                .await?;
            if committed.is_some() {
                tracing::info!(
                    session_id,
                    duration_us = graph_started.elapsed().as_micros() as u64,
                    "activation_actions_ready"
                );
            }
            Ok(committed)
        }
        .instrument(tracing::info_span!("activate", session_id, revision = 0))
        .await;
        self.spawn_index_recompute(session_id, catalog_generation);
        state
    }

    /// Git availability, clipboard and history do not block the first snapshot.
    ///
    /// The graph budget starts when resolution starts, so these probes cannot spend it.
    fn spawn_supplemental(
        &self,
        session_id: SessionId,
        source_app: String,
    ) -> tokio::task::JoinHandle<()> {
        let platform = self.platform.clone();
        let history = self.resolver.history.clone();
        let resolver = self.resolver.clone();
        let capture_policy = self.capture_policy.clone();
        let clipboard_token = self.capture_token(ContextSource::Clipboard);
        tokio::spawn(async move {
            let git_available = tokio::task::spawn_blocking({
                let platform = platform.clone();
                move || platform.git_available()
            });
            let clipboard = clipboard_token
                .filter(|token| {
                    capture_policy
                        .lock()
                        .is_ok_and(|policy| policy.allows(ContextSource::Clipboard, *token))
                })
                .map(|_| {
                    tokio::task::spawn_blocking({
                        let platform = platform.clone();
                        move || platform.clipboard_text()
                    })
                });
            let usage = tokio::task::spawn_blocking(move || {
                let history = history
                    .lock()
                    .map_err(|_| anyhow::anyhow!("history lock poisoned"))?;
                Ok::<_, anyhow::Error>((history.load(&source_app), history.load_global()))
            });
            // Each probe publishes when it finishes. A clipboard permission prompt can remain
            // open indefinitely; it must not keep usage/frecency or Git availability stale.
            let usage_refresh = async {
                let mut diagnostics = Vec::new();
                let (usage_map, app_usage) = match usage.await {
                    Ok(Ok((scoped, global))) => {
                        let mut recover = |result: anyhow::Result<
                            crate::core::ranking::usage::UsageSnapshot,
                        >| match result {
                            Ok(value) => value,
                            Err(error) => {
                                diagnostics
                                    .push(format!("Usage history could not be loaded: {error:#}"));
                                Default::default()
                            }
                        };
                        (recover(scoped), recover(global))
                    }
                    failure => {
                        diagnostics.push(format!("Usage history task failed: {failure:?}"));
                        (Default::default(), Default::default())
                    }
                };
                publish_supplemental(&resolver, session_id, diagnostics, move |session, _| {
                    session.replace_usage(
                        std::sync::Arc::new(usage_map),
                        std::sync::Arc::new(app_usage),
                    );
                    true
                });
            };
            let git_refresh = async {
                let mut diagnostics = Vec::new();
                let git_flag = match git_available.await {
                    Ok(available) => Some(available),
                    Err(error) => {
                        diagnostics.push(format!("{GIT}: {error}"));
                        None
                    }
                };
                publish_supplemental(
                    &resolver,
                    session_id,
                    diagnostics,
                    move |session, details| {
                        git_flag.is_some_and(|available| {
                            if let Err(error) = session.record_fact(GIT, FactValue::Flag(available))
                            {
                                details.push(format!("{GIT}: {error:#}"));
                                false
                            } else {
                                true
                            }
                        })
                    },
                );
            };
            let clipboard_refresh = async {
                let Some(token) = clipboard_token else { return };
                let Some(clipboard) = clipboard else { return };
                let (status, clipboard_text) = match clipboard.await {
                    Ok(Ok(Some(text))) => (CaptureStatus::Captured, Some(text)),
                    Ok(Ok(None)) => (CaptureStatus::Empty, None),
                    Ok(Err(_)) | Err(_) => (CaptureStatus::Failed, None),
                };
                if !capture_policy
                    .lock()
                    .is_ok_and(|policy| policy.allows(ContextSource::Clipboard, token))
                {
                    return;
                }
                let fact_text = clipboard_text.clone();
                publish_supplemental(
                    &resolver,
                    session_id,
                    if status == CaptureStatus::Failed {
                        vec!["clipboard: read failed".into()]
                    } else {
                        Vec::new()
                    },
                    move |session, details| {
                        fact_text.is_some_and(|text| {
                            if let Err(error) =
                                session.record_fact(CLIPBOARD, FactValue::Text(text))
                            {
                                details.push(format!("{CLIPBOARD}: {error:#}"));
                                false
                            } else {
                                true
                            }
                        })
                    },
                );
                let captured_at_ms = super::context_capture::now_ms();
                let mut contexts = clipboard_text
                    .and_then(|text| {
                        super::context_capture::text_context(text, ContextSource::Clipboard)
                    })
                    .into_iter()
                    .collect::<Vec<_>>();
                for context in &mut contexts {
                    context.capture = Some(CaptureMetadata {
                        captured_at_ms,
                        owning_app: None,
                    });
                }
                super::context_capture::publish_capture(
                    resolver.clone(),
                    capture_policy.clone(),
                    token,
                    session_id,
                    ActivationCapture {
                        source: ContextSource::Clipboard,
                        captured_at_ms,
                        status: if status == CaptureStatus::Captured && contexts.is_empty() {
                            CaptureStatus::Unavailable
                        } else {
                            status
                        },
                        contexts,
                    },
                )
                .await;
            };
            tokio::join!(usage_refresh, git_refresh, clipboard_refresh);
            if let Ok(mut session) = resolver.session.lock() {
                if session.state.session_id == session_id {
                    session.supplemental_pending = false;
                }
            }
        })
    }

    fn finish_supplemental(&self, session_id: SessionId) {
        if let Ok(mut session) = self.session() {
            if session.state.session_id == session_id {
                session.supplemental_pending = false;
            }
        }
    }
}

/// Merge one late result only into the activation that requested it. `record_fact` and
/// `replace_usage` advance the session input epoch, so a concurrent snapshot commit retries.
fn publish_supplemental(
    resolver: &Resolver,
    session_id: SessionId,
    mut diagnostics: Vec<String>,
    apply: impl FnOnce(&mut LauncherSession, &mut Vec<String>) -> bool,
) {
    let changed = {
        let Ok(mut session) = resolver.session.lock() else {
            return;
        };
        if session.state.session_id != session_id || !resolver.is_session_active(session_id) {
            return;
        }
        let changed = apply(&mut session, &mut diagnostics);
        let has_diagnostics = !diagnostics.is_empty();
        session.state.diagnostics.extend(diagnostics);
        changed || has_diagnostics
    };
    if changed {
        let _ = resolver.recompute_current(session_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::action::descriptor::ActionRegistry;
    use crate::core::ranking::usage::MemoryUsage;
    use anywhere_platform::{ApplicationInfo, Platform};
    use std::path::Path;
    use std::sync::{Arc, Condvar, Mutex};
    use std::time::Duration;
    use tokio::sync::Notify;

    struct HeldClipboard {
        released: Mutex<bool>,
        signal: Condvar,
        entered: Notify,
    }

    impl HeldClipboard {
        fn release(&self) {
            *self.released.lock().unwrap() = true;
            self.signal.notify_all();
        }
    }

    impl Platform for HeldClipboard {
        fn frontmost_application(&self) -> Result<Option<ApplicationInfo>> {
            Ok(None)
        }
        fn application_directory(&self, _: &ApplicationInfo) -> Result<Option<std::path::PathBuf>> {
            Ok(None)
        }
        fn open_url(&self, _: &str) -> Result<()> {
            Ok(())
        }
        fn open_file(&self, _: &Path) -> Result<()> {
            Ok(())
        }
        fn reveal_file(&self, _: &Path) -> Result<()> {
            Ok(())
        }
        fn open_directory(&self, _: &Path) -> Result<()> {
            Ok(())
        }
        fn open_terminal(&self, _: &Path) -> Result<()> {
            Ok(())
        }
        fn copy_text(&self, _: &str) -> Result<()> {
            Ok(())
        }
        fn clipboard_text(&self) -> Result<Option<String>> {
            self.entered.notify_one();
            let mut released = self.released.lock().unwrap();
            while !*released {
                released = self.signal.wait(released).unwrap();
            }
            Ok(Some("stale clipboard".into()))
        }
    }

    #[tokio::test]
    async fn old_clipboard_result_cannot_modify_a_new_session() {
        let platform = Arc::new(HeldClipboard {
            released: Mutex::new(false),
            signal: Condvar::new(),
            entered: Notify::new(),
        });
        let app = Application::new(
            platform.clone(),
            ActionRegistry::default(),
            Box::<MemoryUsage>::default(),
            vec![],
        );
        let first = app.begin_activation().unwrap();
        let worker = app.spawn_supplemental(first, String::new());
        let started =
            tokio::time::timeout(Duration::from_secs(2), platform.entered.notified()).await;
        if started.is_err() {
            platform.release();
        }
        started.expect("clipboard probe never started");
        let second = app.begin_activation().unwrap();
        platform.release();
        tokio::time::timeout(Duration::from_secs(2), worker)
            .await
            .expect("old supplemental task did not finish")
            .unwrap();

        let session = app.session().unwrap();
        assert_eq!(session.state.session_id, second);
        assert!(session.environment.is_empty());
        assert!(session.usage.is_empty());
        assert!(session.app_usage.is_empty());
        assert!(session.state.diagnostics.is_empty());
        assert_eq!(session.resolution.inputs_epoch, 0);
    }
}
