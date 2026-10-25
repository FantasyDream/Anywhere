//! Revalidate the offered object before copying or opening its source application.
use super::HistoryService;
use crate::features::application_history::model::Target;
use anyhow::{bail, ensure, Result};
use std::sync::atomic::Ordering;

impl HistoryService {
    pub fn use_item(
        &self,
        id: &str,
        copy: bool,
        platform: &dyn anywhere_platform::Platform,
    ) -> Result<()> {
        let epoch = self.epoch.load(Ordering::Acquire);
        ensure!(
            self.enabled.load(Ordering::Acquire),
            "history feature disabled"
        );
        let (item, source) = {
            let state = self.state.lock().unwrap();
            let item = state
                .items
                .iter()
                .find(|i| i.id == id)
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("history selection expired"))?;
            let source = state
                .sources
                .iter()
                .find(|s| s.id == item.source_id)
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("history source disabled"))?;
            (item, source)
        };
        let _worker = self
            .worker
            .try_lock()
            .map_err(|_| anyhow::anyhow!("history reader busy"))?;
        let cancelled = self.cancellation(epoch);
        let fresh = self.loader.read(&source, cancelled.clone())?;
        ensure!(
            !cancelled()
                && fresh
                    .iter()
                    .any(|e| e.key == item.entry.key && e.target == item.entry.target),
            "history target changed"
        );
        if copy {
            return platform.copy_text(&item.entry.target.text());
        }
        let Some(path) = item.entry.target.local_path() else {
            bail!("remote history targets are not supported yet");
        };
        ensure!(
            match item.entry.target {
                Target::Directory { .. } | Target::ObsidianVault { .. } => path.is_dir(),
                _ => path.is_file(),
            },
            "history target unavailable"
        );
        let descriptor = self
            .loader
            .descriptors()
            .into_iter()
            .find(|d| d.id == item.source_id)
            .ok_or_else(|| anyhow::anyhow!("source unavailable"))?;
        let apps = platform.installed_applications();
        let matches: Vec<_> = apps
            .applications
            .iter()
            .filter(|app| descriptor.applications.contains(&app.bundle_id))
            .collect();
        ensure!(
            matches.len() == 1 && !cancelled(),
            "source application unavailable or ambiguous"
        );
        let app = matches[0]
            .path
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("application path unavailable"))?;
        if let Target::ObsidianVault { vault_id, .. } = &item.entry.target {
            ensure!(
                matches[0].bundle_id == "md.obsidian",
                "vault target requires Obsidian"
            );
            let mut uri = url::Url::parse("obsidian://open")?;
            uri.query_pairs_mut().append_pair("vault", vault_id);
            return platform.open_url_in_application(std::path::Path::new(app), uri.as_str());
        }
        platform
            .open_file_in_application(std::path::Path::new(app), path)
            .map_err(|_| anyhow::anyhow!("history open could not be submitted"))
    }
}
