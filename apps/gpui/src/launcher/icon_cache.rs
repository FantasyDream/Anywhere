use anywhere_platform::RawIconImage;
use gpui_kit::RenderImage;
use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};

const MAX_APPLICATION_ICON_CACHE: usize = 96;
const IDLE_RETAINED_ICON_CACHE: usize = 32;
const MISSING_ICON_RETRY_DELAY: Duration = Duration::from_secs(30);

struct CachedApplicationIcon {
    image: Option<Arc<RenderImage>>,
    retry_after: Option<Instant>,
    last_used: u64,
}

#[derive(Default)]
pub(super) struct ApplicationIconCache {
    entries: HashMap<String, CachedApplicationIcon>,
    clock: u64,
}

/// Convert native premultiplied BGRA8 into the straight-alpha BGRA8 that GPUI blends.
pub(super) fn raw_icon_to_render_image(mut raw: RawIconImage) -> Option<Arc<RenderImage>> {
    if raw.width == 0 || raw.height == 0 {
        return None;
    }
    for [blue, green, red, alpha] in raw.bgra.as_chunks_mut::<4>().0 {
        let alpha = u16::from(*alpha);
        if alpha == 0 {
            [blue, green, red]
                .into_iter()
                .for_each(|channel| *channel = 0);
        } else if alpha < 255 {
            for channel in [blue, green, red] {
                let straight = (u16::from(*channel) * 255 + alpha / 2) / alpha;
                *channel = u8::try_from(straight).unwrap_or(u8::MAX);
            }
        }
    }
    let buffer = image::RgbaImage::from_raw(raw.width, raw.height, raw.bgra)?;
    let frame = image::Frame::new(buffer);
    Some(Arc::new(RenderImage::new(smallvec::smallvec![frame])))
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

    pub(super) fn get(&mut self, key: &str) -> Option<Arc<RenderImage>> {
        self.clock = self.clock.wrapping_add(1);
        let clock = self.clock;
        let entry = self.entries.get_mut(key)?;
        entry.last_used = clock;
        entry.image.clone()
    }

    pub(super) fn insert(&mut self, key: String, image: Arc<RenderImage>) -> Vec<Arc<RenderImage>> {
        self.store(key, Some(image), None)
    }

    pub(super) fn insert_missing(&mut self, key: String) -> Vec<Arc<RenderImage>> {
        self.insert_missing_at(key, Instant::now())
    }

    fn insert_missing_at(&mut self, key: String, now: Instant) -> Vec<Arc<RenderImage>> {
        self.store(key, None, Some(now + MISSING_ICON_RETRY_DELAY))
    }

    /// Trim the cache when the launcher becomes idle, returning evicted GPU textures
    /// so the caller can explicitly release them via `Window::drop_image`.
    pub(super) fn trim_for_idle(&mut self) -> Vec<Arc<RenderImage>> {
        self.evict_down_to(IDLE_RETAINED_ICON_CACHE)
    }

    fn store(
        &mut self,
        key: String,
        image: Option<Arc<RenderImage>>,
        retry_after: Option<Instant>,
    ) -> Vec<Arc<RenderImage>> {
        self.clock = self.clock.wrapping_add(1);
        let mut evicted = Vec::new();
        if let Some(previous) = self.entries.insert(
            key,
            CachedApplicationIcon {
                image,
                retry_after,
                last_used: self.clock,
            },
        ) {
            if let Some(previous_image) = previous.image {
                evicted.push(previous_image);
            }
        }
        evicted.extend(self.evict_down_to(MAX_APPLICATION_ICON_CACHE));
        evicted
    }

    fn evict_down_to(&mut self, limit: usize) -> Vec<Arc<RenderImage>> {
        let mut evicted = Vec::new();
        while self.entries.len() > limit {
            let Some(oldest) = self
                .entries
                .iter()
                .min_by_key(|(_, entry)| entry.last_used)
                .map(|(key, _)| key.clone())
            else {
                break;
            };
            if let Some(removed) = self.entries.remove(&oldest) {
                if let Some(image) = removed.image {
                    evicted.push(image);
                }
            }
        }
        evicted
    }

    #[cfg(test)]
    pub(super) fn clear(&mut self) -> Vec<Arc<RenderImage>> {
        self.clock = self.clock.wrapping_add(1);
        self.entries
            .drain()
            .filter_map(|(_, entry)| entry.image)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn synthetic_render_image() -> Arc<RenderImage> {
        raw_icon_to_render_image(RawIconImage {
            width: 2,
            height: 2,
            bgra: vec![255; 16],
        })
        .unwrap()
    }

    #[test]
    fn native_icon_bytes_keep_color_channels_in_straight_alpha_bgra_order() {
        let image = raw_icon_to_render_image(RawIconImage {
            width: 3,
            height: 1,
            bgra: vec![32, 64, 128, 128, 17, 34, 51, 255, 0, 0, 0, 0],
        })
        .unwrap();
        assert_eq!(
            image.as_bytes(0).unwrap(),
            &[64, 128, 255, 128, 17, 34, 51, 255, 0, 0, 0, 0]
        );
    }

    #[test]
    fn missing_icons_retry_after_the_delay_and_success_replaces_the_miss() {
        let mut cache = ApplicationIconCache::default();
        let now = Instant::now();
        cache.insert_missing_at("app".into(), now);
        assert!(cache.contains_at("app", now));
        assert!(cache.contains_at("app", now + MISSING_ICON_RETRY_DELAY / 2));
        assert!(!cache.contains_at("app", now + MISSING_ICON_RETRY_DELAY));
        assert!(cache.get("app").is_none());

        let image = synthetic_render_image();
        let evicted = cache.insert("app".into(), image.clone());
        assert!(evicted.is_empty());
        assert!(cache.contains_at("app", now + MISSING_ICON_RETRY_DELAY * 2));
        assert!(Arc::ptr_eq(&cache.get("app").unwrap(), &image));
    }

    #[test]
    fn eviction_and_idle_trim_return_dropped_render_images_for_gpu_release() {
        let mut cache = ApplicationIconCache::default();
        let mut evicted_count = 0;
        for index in 0..=MAX_APPLICATION_ICON_CACHE {
            evicted_count += cache
                .insert(index.to_string(), synthetic_render_image())
                .len();
        }
        assert_eq!(cache.entries.len(), MAX_APPLICATION_ICON_CACHE);
        assert_eq!(evicted_count, 1);

        let trimmed = cache.trim_for_idle();
        assert_eq!(cache.entries.len(), IDLE_RETAINED_ICON_CACHE);
        assert_eq!(
            trimmed.len(),
            MAX_APPLICATION_ICON_CACHE - IDLE_RETAINED_ICON_CACHE
        );
        let cleared = cache.clear();
        assert_eq!(cleared.len(), IDLE_RETAINED_ICON_CACHE);
        assert!(cache.entries.is_empty());
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
        let _ = cache.clear();
        assert!(cache.entries.is_empty());
    }
}
