//! Read admission, session ownership and ranked metadata publication.
use super::HistoryService;
use crate::{
    core::search::query::{Query, SearchFields},
    features::application_history::model::{Item, Notice, SearchResult},
};
use anyhow::{ensure, Result};
use sha2::{Digest, Sha256};
use std::sync::atomic::Ordering;

impl HistoryService {
    pub fn open_session(&self) -> Result<u64> {
        self.refresh()?;
        let mut state = self.state.lock().unwrap();
        let owner = self
            .owner
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |n| n.checked_add(1))
            .map_err(|_| anyhow::anyhow!("history session exhausted"))?
            + 1;
        self.epoch.fetch_add(1, Ordering::AcqRel);
        state.items.clear();
        Ok(owner)
    }
    pub fn cancel_session(&self, owner: u64) {
        let mut state = self.state.lock().unwrap();
        if owner == self.owner.load(Ordering::Acquire) {
            self.epoch.fetch_add(1, Ordering::AcqRel);
            state.items.clear();
        }
    }
    pub fn search(&self, owner: u64, text: &str, filter: &str) -> Result<SearchResult> {
        ensure!(text.len() <= 1024, "history query too large");
        let epoch = {
            let mut state = self.state.lock().unwrap();
            if owner != self.owner.load(Ordering::Acquire) {
                return Ok(SearchResult {
                    items: vec![],
                    failed_sources: vec![],
                    notice: Some(Notice::Stale),
                });
            }
            let epoch = self.epoch.fetch_add(1, Ordering::AcqRel) + 1;
            state.items.clear();
            epoch
        };
        let empty = |notice| SearchResult {
            items: vec![],
            failed_sources: vec![],
            notice: Some(notice),
        };
        if !self.enabled.load(Ordering::Acquire) {
            return Ok(empty(Notice::Disabled));
        }
        let Ok(_worker) = self.worker.try_lock() else {
            return Ok(empty(Notice::Busy));
        };
        let cancelled = self.cancellation(epoch);
        let current = || !cancelled();
        let sources = self.state.lock().unwrap().sources.clone();
        if !sources
            .iter()
            .any(|source| filter.is_empty() || source.id == filter)
        {
            return Ok(empty(Notice::Unconfigured));
        }
        let query = Query::parse(text);
        let mut ranked = Vec::new();
        let mut failed = Vec::new();
        for source in sources
            .iter()
            .filter(|s| filter.is_empty() || filter == s.id)
        {
            if !current() {
                return Ok(empty(Notice::Stale));
            }
            match self.loader.read(source, cancelled.clone()) {
                Ok(entries) => {
                    for (order, entry) in entries.into_iter().enumerate() {
                        // Opening recent history is normally an empty query;
                        // do not allocate normalized title/path search fields.
                        let matched = if query.is_empty() {
                            None
                        } else {
                            SearchFields::new(&entry.title, &[entry.target.text()], &[])
                                .matched(&query)
                        };
                        if !query.is_empty() && matched.is_none() {
                            continue;
                        }
                        let id = format!(
                            "{}:{}:{:x}",
                            source.id,
                            epoch,
                            Sha256::digest(entry.key.as_bytes())
                        );
                        ranked.push((
                            matched,
                            order,
                            Item {
                                id,
                                source_id: source.id.clone(),
                                entry,
                            },
                        ));
                    }
                }
                Err(_) => failed.push(source.id.clone()),
            }
        }
        ranked.sort_by(|a, b| {
            b.0.cmp(&a.0)
                .then(a.2.source_id.cmp(&b.2.source_id))
                .then(b.2.entry.last_used_at.cmp(&a.2.entry.last_used_at))
                .then(a.1.cmp(&b.1))
        });
        let items: Vec<_> = ranked.into_iter().map(|x| x.2).collect();
        let mut state = self.state.lock().unwrap();
        if !current() {
            return Ok(empty(Notice::Stale));
        }
        state.items = items.clone();
        Ok(SearchResult {
            notice: (!failed.is_empty()).then_some(Notice::ReadFailed),
            failed_sources: failed,
            items,
        })
    }
}
