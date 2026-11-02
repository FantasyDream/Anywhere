//! Read admission, session ownership and ranked metadata publication.
use super::HistoryService;
use crate::{
    core::search::query::{MatchLevel, Query, SearchFields},
    features::application_history::model::{Item, Notice, SearchResult, SourcePreference},
};
use anyhow::{ensure, Result};
use sha2::{Digest, Sha256};
use std::sync::{atomic::Ordering, Arc};

/// Source recency is separate from the Launcher's action execution frequency.
struct RankedEntry {
    matched: Option<MatchLevel>,
    source_order: usize,
    item: Item,
}

struct Read {
    epoch: u64,
    source: SourcePreference,
}
enum Prepared {
    Read(Read),
    Complete(SearchResult),
}
fn empty(notice: Notice) -> SearchResult {
    SearchResult {
        items: Arc::default(),
        failed_sources: vec![],
        notice: Some(notice),
    }
}

impl HistoryService {
    pub fn open_session(&self) -> Result<u64> {
        self.refresh()?;
        let mut state = self.state.lock().unwrap();
        let owner = self
            .owner
            .try_update(Ordering::AcqRel, Ordering::Acquire, |n| n.checked_add(1))
            .map_err(|_| anyhow::anyhow!("history session exhausted"))?
            + 1;
        self.advance_epoch();
        state.items = Arc::default();
        Ok(owner)
    }
    pub fn cancel_session(&self, owner: u64) {
        let mut state = self.state.lock().unwrap();
        if owner == self.owner.load(Ordering::Acquire) {
            self.advance_epoch();
            state.items = Arc::default();
        }
    }
    /// Standalone synchronous readers fail fast when a physical read is active.
    pub fn search(&self, owner: u64, text: &str, source_id: &str) -> Result<SearchResult> {
        let read = match self.prepare_search(owner, text, source_id)? {
            Prepared::Read(read) => read,
            Prepared::Complete(result) => return Ok(result),
        };
        let Ok(_worker) = self.worker.try_acquire() else {
            return Ok(empty(Notice::Busy));
        };
        self.read_search(read, text)
    }

    /// Interactive queries wait for the cancelled reader to relinquish its slot.
    /// A superseded waiter exits immediately; it never starts another source read.
    pub async fn search_wait(
        self: &Arc<Self>,
        owner: u64,
        text: String,
        source_id: String,
    ) -> Result<SearchResult> {
        let read = match self.prepare_search(owner, &text, &source_id)? {
            Prepared::Read(read) => read,
            Prepared::Complete(result) => return Ok(result),
        };
        let changed = self.query_changed.notified();
        tokio::pin!(changed);
        changed.as_mut().enable();
        if (self.cancellation(read.epoch))() {
            return Ok(empty(Notice::Stale));
        }
        let permit = tokio::select! {
            _ = changed => return Ok(empty(Notice::Stale)),
            result = tokio::time::timeout(std::time::Duration::from_secs(5), self.worker.clone().acquire_owned()) => {
                match result {
                    Ok(Ok(permit)) => permit,
                    _ => return Ok(empty(Notice::Busy)),
                }
            }
        };
        let service = self.clone();
        tokio::task::spawn_blocking(move || {
            // Retain physical admission even when the async caller goes away.
            let _permit = permit;
            service.read_search(read, &text)
        })
        .await?
    }

    fn prepare_search(&self, owner: u64, text: &str, source_id: &str) -> Result<Prepared> {
        ensure!(text.len() <= 1024, "history query too large");
        let mut state = self.state.lock().unwrap();
        if owner != self.owner.load(Ordering::Acquire) {
            return Ok(Prepared::Complete(empty(Notice::Stale)));
        }
        let epoch = self.advance_epoch();
        state.items = Arc::default();
        if !self.enabled.load(Ordering::Acquire) {
            return Ok(Prepared::Complete(empty(Notice::Disabled)));
        }
        // Each query names one source; never fall back to reading every source.
        let Some(source) = state
            .sources
            .iter()
            .find(|source| source.id == source_id)
            .cloned()
        else {
            return Ok(Prepared::Complete(empty(Notice::Unconfigured)));
        };
        Ok(Prepared::Read(Read { epoch, source }))
    }

    fn read_search(&self, read: Read, text: &str) -> Result<SearchResult> {
        let Read { epoch, source } = read;
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
                        "{}:{}:{}",
                        source.id,
                        epoch,
                        hex::encode(Sha256::digest(entry.key.as_bytes()))
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
        let items: Arc<[_]> = ranked.into_iter().map(|entry| entry.item).collect();
        let mut state = self.state.lock().unwrap();
        if !current() {
            return Ok(empty(Notice::Stale));
        }
        // The caller and execution validation share immutable metadata. Revoking
        // the session drops our reference instead of retaining an empty Vec capacity.
        state.items = items.clone();
        Ok(SearchResult {
            notice: (!failed.is_empty()).then_some(Notice::ReadFailed),
            failed_sources: failed,
            items,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::application_history::model::*;
    use crate::persistence::module_storage::ModuleStorageHost;
    use std::path::PathBuf;

    struct Source;
    impl SourceLoader for Source {
        fn descriptors(&self) -> Vec<SourceDescriptor> {
            vec![SourceDescriptor {
                id: "test".into(),
                title: "Test".into(),
                file_hint: String::new(),
                applications: vec!["test.app".into()],
                built_in: false,
                action: SourceAction {
                    title: "History".into(),
                    localized_titles: Default::default(),
                    aliases: vec![],
                },
            }]
        }
        fn detect(&self, _: &str) -> Option<PathBuf> {
            Some("/synthetic/source".into())
        }
        fn read(&self, _: &SourcePreference, _: Cancelled) -> Result<Vec<Entry>> {
            Ok(vec![Entry {
                key: "entry".into(),
                title: "Project".into(),
                target: Target::Directory {
                    path: "/synthetic/project".into(),
                },
                last_used_at: None,
            }])
        }
    }

    fn service() -> Arc<HistoryService> {
        let storage = ModuleStorageHost::memory()
            .unwrap()
            .bind("history")
            .unwrap();
        Arc::new(
            HistoryService::new(Ok(storage), Arc::new(Source))
                .with_installed_applications(["test.app".into()]),
        )
    }

    #[tokio::test(start_paused = true)]
    async fn interactive_read_waits_past_the_old_ui_retry_window() {
        let service = service();
        let owner = service.open_session().unwrap();
        let old_reader = service.worker.clone().acquire_owned().await.unwrap();
        let task = {
            let service = service.clone();
            tokio::spawn(async move {
                service
                    .search_wait(owner, "".into(), "test".into())
                    .await
                    .unwrap()
            })
        };
        tokio::task::yield_now().await;
        tokio::time::advance(std::time::Duration::from_secs(1)).await;
        assert!(!task.is_finished());
        drop(old_reader);
        let result = task.await.unwrap();
        assert!(result.notice.is_none());
        assert_eq!(result.items[0].entry.title, "Project");
    }

    #[tokio::test(start_paused = true)]
    async fn newer_query_and_session_cancellation_revoke_waiters_before_admission() {
        let service = service();
        let owner = service.open_session().unwrap();
        let old_reader = service.worker.clone().acquire_owned().await.unwrap();
        let first = {
            let service = service.clone();
            tokio::spawn(async move {
                service
                    .search_wait(owner, "old".into(), "test".into())
                    .await
                    .unwrap()
            })
        };
        tokio::task::yield_now().await;
        let latest = {
            let service = service.clone();
            tokio::spawn(async move {
                service
                    .search_wait(owner, "".into(), "test".into())
                    .await
                    .unwrap()
            })
        };
        tokio::task::yield_now().await;
        assert_eq!(first.await.unwrap().notice, Some(Notice::Stale));
        assert!(!latest.is_finished());
        service.cancel_session(owner);
        assert_eq!(latest.await.unwrap().notice, Some(Notice::Stale));
        drop(old_reader);
        assert!(service.state.lock().unwrap().items.is_empty());
        assert!(service
            .search_wait(owner, "".into(), "test".into())
            .await
            .unwrap()
            .notice
            .is_none());
    }

    #[tokio::test(start_paused = true)]
    async fn interactive_admission_has_a_finite_budget() {
        let service = service();
        let owner = service.open_session().unwrap();
        let _old_reader = service.worker.clone().acquire_owned().await.unwrap();
        let result = service
            .search_wait(owner, "".into(), "test".into())
            .await
            .unwrap();
        assert_eq!(result.notice, Some(Notice::Busy));
    }

    #[test]
    fn result_shares_metadata_and_cancellation_releases_ownership_without_reviving_execution() {
        let storage = ModuleStorageHost::memory()
            .unwrap()
            .bind("history")
            .unwrap();
        let service = HistoryService::new(Ok(storage), Arc::new(Source))
            .with_installed_applications(["test.app".into()]);
        let owner = service.open_session().unwrap();
        let result = service.search(owner, "", "test").unwrap();
        assert!(Arc::ptr_eq(
            &result.items,
            &service.state.lock().unwrap().items
        ));
        service.cancel_session(owner);
        assert!(service.state.lock().unwrap().items.is_empty());
        assert_eq!(Arc::strong_count(&result.items), 1);
        assert_eq!(result.items[0].entry.title, "Project");
        assert!(service
            .with_validated_item(&result.items[0].id, |_, _| Ok(()))
            .is_err());
    }
}
