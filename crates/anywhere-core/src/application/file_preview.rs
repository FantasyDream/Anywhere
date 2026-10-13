//! Preview only a current search candidate, without executing it or recording Usage.
use super::Application;
use anyhow::{bail, Context, Result};
use anywhere_protocol::*;
use std::{sync::atomic::Ordering, time::Duration};

impl Application {
    pub async fn preview_file(
        &self,
        session_id: SessionId,
        revision: Revision,
        candidate_id: CandidateId,
    ) -> Result<crate::features::filesystem::preview::FilePreview> {
        let epoch = self.resolver.execution_epoch.clone();
        let expected_epoch = epoch.load(Ordering::Acquire);
        let ticket = self.resolver.execution(session_id, revision);
        let policy = self.file_search_policy.clone();
        let generation = policy
            .read()
            .map_err(|_| anyhow::anyhow!("search settings unavailable"))?
            .generation;
        let catalog = self.resolver.action_catalog_version.clone();
        let version = catalog.load(Ordering::Acquire);
        let context = {
            let session = self.session()?;
            if session.suspended
                || !ticket.is_current()
                || session.input_mode != InputMode::FileSearch
                || session.execution.revision != Some(revision)
                || session.execution.action_catalog_version != Some(version)
            {
                bail!("preview expired; choose a current file");
            }
            let candidate = session
                .execution
                .candidates
                .get(candidate_id.as_str())
                .context("preview target expired")?;
            if candidate.presentation.kind != CandidateKind::File
                || !session
                    .state
                    .candidates
                    .iter()
                    .any(|c| c.id == candidate_id)
            {
                bail!("preview requires a file search result");
            }
            candidate
                .context()
                .context("preview target missing")?
                .clone()
        };
        let permit = tokio::select! {
            permit = tokio::time::timeout(Duration::from_secs(2), self.preview_worker.clone().acquire_owned()) => permit.context("preview is busy")??,
            _ = ticket.superseded() => bail!("preview expired"),
        };
        let is_current = move || {
            ticket.is_current()
                && epoch.load(Ordering::Acquire) == expected_epoch
                && catalog.load(Ordering::Acquire) == version
                && policy
                    .read()
                    .is_ok_and(|p| p.enabled && p.generation == generation)
        };
        tokio::time::timeout(
            Duration::from_secs(2),
            tokio::task::spawn_blocking(move || {
                let _permit = permit;
                if !is_current() {
                    bail!("preview expired");
                }
                let preview = crate::features::filesystem::preview::read(&context)?;
                if !is_current() {
                    bail!("preview expired");
                }
                Ok(preview)
            }),
        )
        .await
        .context("preview timed out")??
    }
}
