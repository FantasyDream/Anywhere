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
    ) -> Result<crate::features::preview::PreviewResource> {
        let epoch = self.resolver.execution_epoch.clone();
        let expected_epoch = epoch.load(Ordering::Acquire);
        let ticket = self.resolver.execution(session_id, revision);
        let policy = self.file_search_policy.clone();
        let generation = policy
            .read()
            .map_err(|_| anyhow::anyhow!("search settings unavailable"))?
            .generation;
        let catalog = self.resolver.catalog.clone();
        let version = catalog.version();
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
                .execution_candidate(candidate_id.as_str())
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
        let previews = self.previews.clone();
        let permit = tokio::select! {
            permit = tokio::time::timeout(Duration::from_secs(2), self.preview_worker.clone().acquire_owned()) => permit.context("preview is busy")??,
            _ = ticket.superseded() => bail!("preview expired"),
        };
        let is_current = move || {
            ticket.is_current()
                && epoch.load(Ordering::Acquire) == expected_epoch
                && catalog.version() == version
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
                crate::features::filesystem::search::validate_target(&context)?;
                let path = context
                    .value
                    .path()
                    .context("preview target missing")?
                    .to_path_buf();
                if !is_current() {
                    bail!("preview expired");
                }
                let mut previews = previews
                    .lock()
                    .map_err(|_| anyhow::anyhow!("preview service unavailable"))?;
                let resource = previews.file(path)?;
                if !is_current() {
                    previews.release(&resource.id);
                    bail!("preview expired");
                }
                Ok(resource)
            }),
        )
        .await
        .context("preview timed out")??
    }
}

impl Application {
    pub fn preview_clipboard(
        &self,
        history: &crate::features::clipboard::service::HistoryService,
        id: i64,
    ) -> Result<crate::features::preview::PreviewResource> {
        let mut prepared = None;
        let result = history.inspect_payload(id, |item, payload| {
            let resource = self
                .previews
                .lock()
                .map_err(|_| anyhow::anyhow!("preview service unavailable"))?
                .clipboard(&payload, item.summary)?;
            prepared = Some(resource.id.clone());
            Ok(resource)
        });
        match result {
            Ok(Some(resource)) => Ok(resource),
            result => {
                if let Some(id) = prepared {
                    self.release_preview(&id)?;
                }
                result?.context("clipboard entry expired")
            }
        }
    }
    pub fn retain_preview(&self, id: &str) -> Result<()> {
        self.previews
            .lock()
            .map_err(|_| anyhow::anyhow!("preview service unavailable"))?
            .retain(id)
    }
    pub fn release_preview(&self, id: &str) -> Result<()> {
        self.previews
            .lock()
            .map_err(|_| anyhow::anyhow!("preview service unavailable"))?
            .release(id);
        Ok(())
    }
}
