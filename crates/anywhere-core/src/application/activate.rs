//! Activation: capture the environment, publish the first snapshot.
//
// The capture happens before the host is told to show anything, so the ambient context is
// frozen at the moment the hotkey was pressed rather than after focus moved.
use super::launcher_service::Application;
use crate::features::filesystem::provider::DirectoryMemo;
use anyhow::{Context, Result};
use anywhere_protocol::*;
use std::time::Instant;
use tracing::Instrument;

impl Application {
    pub(super) async fn activate_use_case(&self) -> Result<Option<LauncherState>> {
        let start = Instant::now();
        // The original foreground application is captured before the host shows anything.
        let platform = self.platform.clone();
        let frontmost = tokio::task::spawn_blocking(move || platform.frontmost_application())
            .await
            .context("foreground capture task failed")?
            .unwrap_or(None);
        let mut diagnostics = self.startup_diagnostics.clone();
        let git_available = {
            let platform = self.platform.clone();
            tokio::task::spawn_blocking(move || platform.git_available())
                .await
                .unwrap_or(false)
        };
        let clipboard = match tokio::task::spawn_blocking({
            let platform = self.platform.clone();
            move || platform.clipboard_text()
        })
        .await
        {
            Ok(Ok(text)) => text,
            Ok(Err(error)) => {
                diagnostics.push(format!("Clipboard snapshot: {error:#}"));
                None
            }
            Err(error) => {
                diagnostics.push(format!("Clipboard snapshot task: {error}"));
                None
            }
        };
        let source_app = Self::source_app_of(frontmost.as_ref());
        let session_id = {
            let mut session = self.session()?;
            let session_id = session
                .state
                .session_id
                .checked_add(1)
                .context("session id exhausted; restart launcher")?;
            let directory = frontmost
                .as_ref()
                .map(|frontmost| DirectoryMemo::start(self.platform.clone(), frontmost.clone()));
            *session = crate::core::session::LauncherSession {
                frontmost: frontmost.clone(),
                directory,
                clipboard,
                git_available,
                state: LauncherState {
                    session_id,
                    ..Default::default()
                },
                ..Default::default()
            };
            self.resolver.current.store(
                crate::core::context::engine::stamp(session_id, 0),
                std::sync::atomic::Ordering::Release,
            );
            session_id
        };
        tracing::info!(
            session_id,
            duration_us = start.elapsed().as_micros() as u64,
            "activation_context_captured"
        );
        self.spawn_catalog_scan();
        let (usage, app_usage) = self.load_usage(&source_app);
        if let Ok(mut session) = self.resolver.session.lock() {
            if session.state.session_id == session_id {
                session.usage = usage;
                session.app_usage = app_usage;
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
