//! Activation: capture the environment, publish the first snapshot.
//
// The capture happens before the host is told to show anything, so the ambient context is
// frozen at the moment the hotkey was pressed rather than after focus moved.
use super::launcher_service::Application;
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
        *session = crate::application::session_model::LauncherSession {
            state: LauncherState {
                session_id,
                ..Default::default()
            },
            actions: self.resolver.registry.clone(),
            ..Default::default()
        };
        self.resolver.current.store(
            crate::core::context::engine::stamp(session_id, 0),
            std::sync::atomic::Ordering::Release,
        );
        self.resolver.latest_tx.send_replace(None);
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
        // The original foreground application is captured before the host shows anything.
        let platform = self.platform.clone();
        let frontmost = tokio::task::spawn_blocking(move || platform.frontmost_application())
            .await
            .context("foreground capture task failed")?
            .unwrap_or(None);
        if !self.resolver.is_current(session_id, 0) {
            return Ok(None);
        }
        let mut diagnostics = self.startup_diagnostics.clone();
        let git_available = match tokio::task::spawn_blocking({
            let platform = self.platform.clone();
            move || platform.git_available()
        })
        .await
        {
            Ok(available) => Some(available),
            Err(error) => {
                diagnostics.push(format!("{GIT}: {error}"));
                None
            }
        };
        if !self.resolver.is_current(session_id, 0) {
            return Ok(None);
        }
        let clipboard = match tokio::task::spawn_blocking({
            let platform = self.platform.clone();
            move || platform.clipboard_text()
        })
        .await
        {
            Ok(Ok(text)) => text,
            Ok(Err(error)) => {
                diagnostics.push(format!("{CLIPBOARD}: {error:#}"));
                None
            }
            Err(error) => {
                diagnostics.push(format!("{CLIPBOARD}: {error}"));
                None
            }
        };
        let source_app = Self::source_app_of(frontmost.as_ref());
        {
            let mut session = self.session()?;
            if !self.resolver.is_current(session_id, 0) {
                return Ok(None);
            }
            session.frontmost = frontmost.clone();
            session.directory = frontmost
                .as_ref()
                .map(|app| DirectoryMemo::start(self.platform.clone(), app.clone()));
            if let Some(text) = clipboard {
                if let Err(error) = session.record_fact(CLIPBOARD, FactValue::Text(text)) {
                    diagnostics.push(format!("{CLIPBOARD}: {error:#}"));
                }
            }
            if let Some(available) = git_available {
                if let Err(error) = session.record_fact(GIT, FactValue::Flag(available)) {
                    diagnostics.push(format!("{GIT}: {error:#}"));
                }
            }
            session.source_app = source_app.clone();
        }
        tracing::info!(
            session_id,
            duration_us = start.elapsed().as_micros() as u64,
            "activation_context_captured"
        );
        self.prepare_app_actions(session_id, frontmost.as_ref(), prompt_accessibility)
            .await;
        self.spawn_catalog_scan();
        let (usage, app_usage, history_diagnostics) = self.load_usage(source_app.clone()).await;
        diagnostics.extend(history_diagnostics);
        if let Ok(mut session) = self.resolver.session.lock() {
            if self.resolver.is_current(session_id, 0) {
                session.usage = std::sync::Arc::new(usage);
                session.app_usage = std::sync::Arc::new(app_usage);
                // Same value the commit path would derive, decided once here.
                session.source_app = source_app;
            }
        }
        let execution = self.resolver.execution(session_id, 0);
        let Some(seeds) = self.resolver.seeds_for(session_id, "") else {
            return Ok(None);
        };
        let run = self.resolver.build_run(&execution, seeds);
        let state = async {
            tracing::info!("session_created");
            let Some(resolution) = self
                .resolver
                .resolve_until_with_grace(session_id, run, self.budget, self.content_grace, start)
                .await
            else {
                return Ok(None);
            };
            let complete_by_budget = resolution.complete;
            let run = resolution.run;
            let mut diagnostics = diagnostics;
            diagnostics.extend(resolution.diagnostics);
            let committed = self.resolver.commit(
                session_id,
                0,
                resolution.contexts,
                diagnostics.clone(),
                complete_by_budget,
                start,
            )?;
            if committed.is_some() {
                tracing::info!(
                    session_id,
                    duration_us = start.elapsed().as_micros() as u64,
                    "activation_actions_ready"
                );
            }
            if !complete_by_budget {
                self.spawn_completion(session_id, 0, run, start, diagnostics);
            }
            Ok(committed)
        }
        .instrument(tracing::info_span!("activate", session_id, revision = 0))
        .await;
        self.spawn_index_recompute(session_id);
        state
    }
}
