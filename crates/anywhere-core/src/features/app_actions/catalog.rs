//! Metadata cache. Generation tickets reject obsolete discoveries and invalidations.
use anywhere_platform::{ApplicationInfo, DiscoveredMenuItem};
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
};

#[derive(Clone, Default)]
pub struct AppActionCatalog {
    inner: Arc<Mutex<HashMap<String, Entry>>>,
    next: Arc<AtomicU64>,
}
#[derive(Default)]
struct Entry {
    generation: u64,
    cached: Option<CachedApp>,
}
struct CachedApp {
    app: ApplicationInfo,
    version: Option<String>,
    items: Vec<DiscoveredMenuItem>,
}
#[derive(Clone)]
pub struct DiscoveryTicket {
    pub generation: u64,
    pub app: ApplicationInfo,
}

impl AppActionCatalog {
    pub fn ticket(&self, app: ApplicationInfo) -> DiscoveryTicket {
        DiscoveryTicket {
            generation: self.next.fetch_add(1, Ordering::Relaxed) + 1,
            app,
        }
    }
    pub fn get(
        &self,
        app: &ApplicationInfo,
        version: Option<&str>,
    ) -> Option<Vec<DiscoveredMenuItem>> {
        let guard = self.inner.lock().ok()?;
        let cached = guard.get(&app.bundle_id)?.cached.as_ref()?;
        if cached.app != *app || cached.version.as_deref() != version {
            return None;
        }
        Some(cached.items.clone())
    }
    pub fn begin(&self, ticket: &DiscoveryTicket, invalidate: bool) -> bool {
        let Ok(mut entries) = self.inner.lock() else {
            return false;
        };
        let entry = entries.entry(ticket.app.bundle_id.clone()).or_default();
        if entry.generation >= ticket.generation {
            return false;
        }
        entry.generation = ticket.generation;
        if invalidate {
            entry.cached = None;
        }
        true
    }
    /// Failure may arrive after focus loss. Invalidate even then, without erasing a
    /// newer session's cache or cancelling its discovery.
    pub fn invalidate_through(&self, bundle: &str, generation: u64) {
        let Ok(mut entries) = self.inner.lock() else {
            return;
        };
        if let Some(entry) = entries.get_mut(bundle) {
            if entry.generation <= generation {
                entry.generation = self.next.fetch_add(1, Ordering::Relaxed) + 1;
                entry.cached = None;
            }
        }
    }
    /// Cache and publish under the same generation check. The callback must not reenter us.
    pub fn finish(
        &self,
        ticket: &DiscoveryTicket,
        version: Option<String>,
        items: Option<Vec<DiscoveredMenuItem>>,
        publish: impl FnOnce(),
    ) -> bool {
        let Ok(mut entries) = self.inner.lock() else {
            return false;
        };
        let Some(entry) = entries.get_mut(&ticket.app.bundle_id) else {
            return false;
        };
        if entry.generation != ticket.generation {
            return false;
        }
        entry.cached = items.map(|items| CachedApp {
            app: ticket.app.clone(),
            version,
            items,
        });
        publish();
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn app() -> ApplicationInfo {
        ApplicationInfo {
            bundle_id: "test".into(),
            name: "Test".into(),
            pid: 42,
            instance_id: Some("one".into()),
        }
    }
    #[test]
    fn obsolete_finish_cannot_publish_or_repopulate_invalidated_cache() {
        let catalog = AppActionCatalog::default();
        let old = catalog.ticket(app());
        assert!(catalog.begin(&old, false));
        let new = catalog.ticket(app());
        assert!(catalog.begin(&new, true));
        assert!(!catalog.finish(&old, None, Some(vec![]), || panic!("stale publication")));
        assert!(catalog.get(&app(), None).is_none());
        assert!(catalog.finish(&new, Some("1".into()), Some(vec![]), || {}));
        assert!(catalog.get(&app(), Some("1")).is_some());
    }
    #[test]
    fn failure_after_dismissal_invalidates_only_its_own_generation() {
        let catalog = AppActionCatalog::default();
        let first = catalog.ticket(app());
        catalog.begin(&first, false);
        catalog.finish(&first, None, Some(vec![]), || {});
        catalog.invalidate_through("test", first.generation);
        assert!(catalog.get(&app(), None).is_none());
        assert!(!catalog.finish(&first, None, Some(vec![]), || panic!("invalidated result")));
        let next = catalog.ticket(app());
        catalog.begin(&next, false);
        catalog.finish(&next, None, Some(vec![]), || {});
        catalog.invalidate_through("test", first.generation);
        assert!(catalog.get(&app(), None).is_some());
    }
    #[test]
    fn restart_version_and_reused_pid_invalidate_metadata_independently() {
        let catalog = AppActionCatalog::default();
        let ticket = catalog.ticket(app());
        catalog.begin(&ticket, false);
        catalog.finish(&ticket, Some("1".into()), Some(vec![]), || {});
        assert!(catalog.get(&app(), Some("2")).is_none());
        let mut changed = app();
        changed.pid = 43;
        assert!(catalog.get(&changed, Some("1")).is_none());
        let mut changed = app();
        changed.instance_id = Some("two".into());
        assert!(catalog.get(&changed, Some("1")).is_none());
    }
}
