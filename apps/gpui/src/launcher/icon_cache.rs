use gpui_kit::Image;
use std::{collections::HashMap, sync::Arc};

const MAX_APPLICATION_ICON_CACHE: usize = 128;

struct CachedApplicationIcon {
    image: Arc<Image>,
    last_used: u64,
}

#[derive(Default)]
pub(super) struct ApplicationIconCache {
    entries: HashMap<String, CachedApplicationIcon>,
    clock: u64,
}

impl ApplicationIconCache {
    pub(super) fn contains(&self, key: &str) -> bool {
        self.entries.contains_key(key)
    }

    pub(super) fn get(&mut self, key: &str) -> Option<Arc<Image>> {
        self.clock = self.clock.wrapping_add(1);
        let clock = self.clock;
        let entry = self.entries.get_mut(key)?;
        entry.last_used = clock;
        Some(entry.image.clone())
    }

    pub(super) fn insert(&mut self, key: String, image: Arc<Image>) {
        self.clock = self.clock.wrapping_add(1);
        self.entries.insert(
            key,
            CachedApplicationIcon {
                image,
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

    pub(super) fn clear(&mut self) {
        self.entries.clear();
        self.clock = self.clock.wrapping_add(1);
    }
}
