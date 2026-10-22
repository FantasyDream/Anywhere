//! Metadata cache. Generation tickets reject obsolete discoveries and invalidations.
use anywhere_platform::{ApplicationInfo, DiscoveredMenuItem};
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
};

const MAX_CACHED_APPS: usize = 8;
const MAX_CACHE_BYTES: usize = 4 * 1024 * 1024;
const MAX_TRACKED_APPS: usize = 32;

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
    items: Arc<[DiscoveredMenuItem]>,
    bytes: usize,
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
    ) -> Option<Arc<[DiscoveredMenuItem]>> {
        let guard = self.inner.lock().ok()?;
        let cached = guard.get(&app.bundle_id)?.cached.as_ref()?;
        if cached.app != *app || cached.version.as_deref() != version {
            return None;
        }
        Some(Arc::clone(&cached.items))
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
        if entries.len() > MAX_TRACKED_APPS {
            let oldest = entries
                .iter()
                .filter(|(id, _)| *id != &ticket.app.bundle_id)
                .min_by_key(|(_, entry)| entry.generation)
                .map(|(id, _)| id.clone());
            if let Some(oldest) = oldest {
                entries.remove(&oldest);
            }
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
        {
            let Ok(mut entries) = self.inner.lock() else {
                return false;
            };
            let Some(entry) = entries.get_mut(&ticket.app.bundle_id) else {
                return false;
            };
            if entry.generation != ticket.generation {
                return false;
            }
            entry.cached = items.and_then(|items| {
                let bytes = metadata_bytes(&items);
                (bytes <= MAX_CACHE_BYTES).then(|| CachedApp {
                    app: ticket.app.clone(),
                    version,
                    items: items.into(),
                    bytes,
                })
            });
            // Only cache ownership is bounded here. An active Registry or reader
            // can retain its immutable snapshot until it finishes using it.
            while entries.values().filter(|e| e.cached.is_some()).count() > MAX_CACHED_APPS
                || entries
                    .values()
                    .filter_map(|e| e.cached.as_ref())
                    .map(|c| c.bytes)
                    .sum::<usize>()
                    > MAX_CACHE_BYTES
            {
                let oldest = entries
                    .iter()
                    .filter(|(_, e)| e.cached.is_some())
                    .min_by_key(|(_, e)| e.generation)
                    .map(|(id, _)| id.clone());
                let Some(oldest) = oldest else { break };
                entries.get_mut(&oldest).unwrap().cached = None;
            }
            publish();
        }
        true
    }
}

/// Conservative payload accounting, including vector capacities and owned string
/// capacities. Excludes allocator overhead and the small per-application header.
fn metadata_bytes(items: &[DiscoveredMenuItem]) -> usize {
    use std::mem::size_of;
    let mut bytes = items.len().saturating_mul(size_of::<DiscoveredMenuItem>());
    for item in items {
        bytes = bytes
            .saturating_add(item.title.capacity())
            .saturating_add(
                item.menu_path
                    .capacity()
                    .saturating_mul(size_of::<String>()),
            )
            .saturating_add(
                item.identifiers
                    .capacity()
                    .saturating_mul(size_of::<Option<String>>()),
            );
        for text in item
            .menu_path
            .iter()
            .chain(item.identifiers.iter().flatten())
            .chain(item.shortcut.iter())
        {
            bytes = bytes.saturating_add(text.capacity());
        }
    }
    bytes
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

    #[test]
    fn menu_metadata_keeps_only_recently_discovered_apps() {
        let catalog = AppActionCatalog::default();
        let apps: Vec<_> = (0..=MAX_CACHED_APPS)
            .map(|index| ApplicationInfo {
                bundle_id: format!("app-{index}"),
                ..app()
            })
            .collect();
        for app in &apps {
            let ticket = catalog.ticket(app.clone());
            assert!(catalog.begin(&ticket, false));
            assert!(catalog.finish(&ticket, None, Some(vec![]), || {}));
        }
        assert!(catalog.get(&apps[0], None).is_none());
        for app in &apps[1..] {
            assert!(catalog.get(app, None).is_some());
        }
    }
    #[test]
    fn readers_share_metadata_and_oversize_payloads_still_publish() {
        let catalog = AppActionCatalog::default();
        let ticket = catalog.ticket(app());
        catalog.begin(&ticket, false);
        catalog.finish(&ticket, None, Some(vec![]), || {});
        let first = catalog.get(&app(), None).unwrap();
        assert!(Arc::ptr_eq(&first, &catalog.get(&app(), None).unwrap()));
        let ticket = catalog.ticket(app());
        catalog.begin(&ticket, false);
        let item = DiscoveredMenuItem {
            title: "x".repeat(MAX_CACHE_BYTES),
            menu_path: vec![],
            identifiers: vec![],
            shortcut: None,
            enabled: true,
        };
        let mut published = false;
        assert!(catalog.finish(&ticket, None, Some(vec![item]), || published = true));
        assert!(published && catalog.get(&app(), None).is_none());
    }
    #[test]
    fn byte_budget_evicts_old_payloads_and_generation_tombstones_are_bounded() {
        let catalog = AppActionCatalog::default();
        for index in 0..MAX_TRACKED_APPS + 3 {
            let app = ApplicationInfo {
                bundle_id: format!("app-{index}"),
                ..app()
            };
            let ticket = catalog.ticket(app);
            catalog.begin(&ticket, false);
            let item = DiscoveredMenuItem {
                title: "x".repeat(MAX_CACHE_BYTES / 3),
                menu_path: vec![],
                identifiers: vec![],
                shortcut: None,
                enabled: true,
            };
            catalog.finish(&ticket, None, Some(vec![item]), || {});
        }
        let entries = catalog.inner.lock().unwrap();
        assert!(entries.len() <= MAX_TRACKED_APPS);
        assert_eq!(entries.values().filter(|e| e.cached.is_some()).count(), 2);
        assert!(
            entries
                .values()
                .filter_map(|e| e.cached.as_ref())
                .map(|c| c.bytes)
                .sum::<usize>()
                <= MAX_CACHE_BYTES
        );
    }
}
