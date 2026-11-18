use gpui_kit::Image;
use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};

const MAX_APPLICATION_ICON_CACHE: usize = 128;
const MISSING_ICON_RETRY_DELAY: Duration = Duration::from_secs(30);

struct CachedApplicationIcon {
    image: Option<Arc<Image>>,
    retry_after: Option<Instant>,
    last_used: u64,
}

#[derive(Default)]
pub(super) struct ApplicationIconCache {
    entries: HashMap<String, CachedApplicationIcon>,
    clock: u64,
}

impl ApplicationIconCache {
    pub(super) fn contains(&self, key: &str) -> bool {
        self.contains_at(key, Instant::now())
    }

    fn contains_at(&self, key: &str, now: Instant) -> bool {
        self.entries
            .get(key)
            .is_some_and(|entry| entry.retry_after.is_none_or(|retry| now < retry))
    }

    pub(super) fn get(&mut self, key: &str) -> Option<Arc<Image>> {
        self.clock = self.clock.wrapping_add(1);
        let clock = self.clock;
        let entry = self.entries.get_mut(key)?;
        entry.last_used = clock;
        entry.image.clone()
    }

    pub(super) fn insert(&mut self, key: String, image: Arc<Image>) {
        self.store(key, Some(image), None);
    }

    pub(super) fn insert_missing(&mut self, key: String) {
        self.insert_missing_at(key, Instant::now());
    }

    fn insert_missing_at(&mut self, key: String, now: Instant) {
        self.store(key, None, Some(now + MISSING_ICON_RETRY_DELAY));
    }

    fn store(&mut self, key: String, image: Option<Arc<Image>>, retry_after: Option<Instant>) {
        self.clock = self.clock.wrapping_add(1);
        self.entries.insert(
            key,
            CachedApplicationIcon {
                image,
                retry_after,
                last_used: self.clock,
            },
        );
        while self.entries.len() > MAX_APPLICATION_ICON_CACHE {
            let Some(oldest) = self
                .entries
                .iter()
                .min_by_key(|(_, entry)| entry.last_used)
                .map(|(key, _)| key.clone())
            else {
                break;
            };
            self.entries.remove(&oldest);
        }
    }

    #[cfg(test)]
    pub(super) fn clear(&mut self) {
        self.entries.clear();
        self.clock = self.clock.wrapping_add(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_icons_retry_after_the_delay_and_success_replaces_the_miss() {
        let mut cache = ApplicationIconCache::default();
        let now = Instant::now();
        cache.insert_missing_at("app".into(), now);
        assert!(cache.contains_at("app", now));
        assert!(cache.contains_at("app", now + MISSING_ICON_RETRY_DELAY / 2));
        assert!(!cache.contains_at("app", now + MISSING_ICON_RETRY_DELAY));
        assert!(cache.get("app").is_none());

        let image = Arc::new(Image::from_bytes(gpui_kit::ImageFormat::Png, Vec::new()));
        cache.insert("app".into(), image.clone());
        assert!(cache.contains_at("app", now + MISSING_ICON_RETRY_DELAY * 2));
        assert!(Arc::ptr_eq(&cache.get("app").unwrap(), &image));
    }

    #[test]
    fn missing_icons_share_the_bounded_cache_and_clear_with_it() {
        let mut cache = ApplicationIconCache::default();
        let now = Instant::now();
        for index in 0..=MAX_APPLICATION_ICON_CACHE {
            cache.insert_missing_at(index.to_string(), now);
        }
        assert_eq!(cache.entries.len(), MAX_APPLICATION_ICON_CACHE);
        assert!(!cache.contains_at("0", now));
        cache.clear();
        assert!(cache.entries.is_empty());
    }
}
