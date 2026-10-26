//! Read admission, session ownership and ranked metadata publication.
use super::HistoryService;
use crate::{
    core::search::query::{MatchLevel, Query, SearchFields},
    features::application_history::model::{Item, Notice, SearchResult},
};
use anyhow::{ensure, Result};
use sha2::{Digest, Sha256};
use std::sync::atomic::Ordering;

/// Source recency is separate from the Launcher's action execution frequency.
struct RankedEntry {
    matched: Option<MatchLevel>,
    source_order: usize,
    item: Item,
}

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
    pub fn search(&self, owner: u64, text: &str, source_id: &str) -> Result<SearchResult> {
        ensure!(text.len() <= 1024, "history query too large");
        let empty = |notice| SearchResult {
            items: vec![],
            failed_sources: vec![],
            notice: Some(notice),
        };
        let epoch = {
            let mut state = self.state.lock().unwrap();
            if owner != self.owner.load(Ordering::Acquire) {
                return Ok(empty(Notice::Stale));
            }
            let epoch = self.epoch.fetch_add(1, Ordering::AcqRel) + 1;
            state.items.clear();
            epoch
        };
        if !self.enabled.load(Ordering::Acquire) {
            return Ok(empty(Notice::Disabled));
        }
        // Each query names one source. An absent source must never revive the
        // removed cross-application route or read every enabled history file.
        let source = self
            .state
            .lock()
            .unwrap()
            .sources
            .iter()
            .find(|source| source.id == source_id)
            .cloned();
        let Some(source) = source else {
            return Ok(empty(Notice::Unconfigured));
        };
        let Ok(_worker) = self.worker.try_lock() else {
            return Ok(empty(Notice::Busy));
        };
        let cancelled = self.cancellation(epoch);
        let current = || !cancelled();
        if !current() {
            return Ok(empty(Notice::Stale));
        }
        let query = Query::parse(text);
        let mut ranked = Vec::new();
        let mut failed = Vec::new();
        match self.loader.read(&source, cancelled.clone()) {
            Ok(entries) => {
                for (order, entry) in entries.into_iter().enumerate() {
                    // Opening recent history is normally an empty query;
                    // do not allocate normalized title/path search fields.
                    let matched = if query.is_empty() {
                        None
                    } else {
                        SearchFields::new(&entry.title, &[entry.target.text()], &[]).matched(&query)
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
                    ranked.push(RankedEntry {
                        matched,
                        source_order: order,
                        item: Item {
                            id,
                            source_id: source.id.clone(),
                            entry,
                        },
                    });
                }
            }
            Err(_) => failed.push(source.id.clone()),
        }
        ranked.sort_by(|a, b| {
            b.matched
                .cmp(&a.matched)
                .then(b.item.entry.last_used_at.cmp(&a.item.entry.last_used_at))
                .then(a.source_order.cmp(&b.source_order))
        });
        let items: Vec<_> = ranked.into_iter().map(|entry| entry.item).collect();
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
