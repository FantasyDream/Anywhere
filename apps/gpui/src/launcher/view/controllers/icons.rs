use super::*;
use crate::launcher::icon_cache::raw_icon_to_render_image;

impl LauncherView {
    pub(super) fn drop_render_images(
        &self,
        images: Vec<Arc<gpui_kit::RenderImage>>,
        cx: &mut Context<Self>,
    ) {
        if images.is_empty() {
            return;
        }
        let window_handle = self.window_handle;
        cx.defer(move |cx| {
            let _ = window_handle.update(cx, |_, window, _| {
                for image in images {
                    let _ = window.drop_image(image);
                }
            });
        });
    }

    pub(super) fn request_application_icons(&mut self, cx: &mut Context<Self>) {
        if !self.presence.presented || !matches!(self.screen, Screen::Launcher | Screen::FileSearch)
        {
            return;
        }
        let Some(live) = self.live.clone() else {
            return;
        };
        let visible_ids = self
            .model
            .rows
            .projected()
            .iter()
            .filter(|row| row.kind == CandidateKind::Application)
            .map(|row| {
                (
                    row.id.to_string(),
                    row.icon_identity
                        .clone()
                        .unwrap_or_else(|| row.id.to_string()),
                )
            })
            .collect::<Vec<_>>();
        for (candidate_id, cache_key) in visible_ids {
            if self.icons.cache.contains(&cache_key)
                || self.icons.requested.contains_key(&candidate_id)
                || self
                    .icons
                    .requested
                    .values()
                    .any(|request| request.cache_key == cache_key)
                || self.icons.requested.len() >= MAX_APPLICATION_ICON_REQUESTS
            {
                continue;
            }
            self.icons.generation = self.icons.generation.wrapping_add(1);
            let ticket = self.icons.generation;
            self.icons.requested.insert(
                candidate_id.clone(),
                ApplicationIconRequest {
                    ticket,
                    cache_key: cache_key.clone(),
                },
            );
            let receiver = live.application_icon(candidate_id.clone());
            self.poll_application_icon(receiver, candidate_id, cache_key, ticket, cx);
        }
    }

    pub(super) fn poll_application_icon(
        &mut self,
        receiver: oneshot::Receiver<Option<anywhere_platform::RawIconImage>>,
        candidate_id: String,
        cache_key: String,
        ticket: u64,
        cx: &mut Context<Self>,
    ) {
        cx.spawn(async move |this, cx| {
            let raw = receiver.await.ok().flatten();
            let decoded = raw.and_then(raw_icon_to_render_image);
            this.update(cx, |this, cx| {
                if this
                    .icons
                    .requested
                    .get(&candidate_id)
                    .map(|request| request.ticket)
                    != Some(ticket)
                {
                    if let Some(image) = decoded {
                        this.drop_render_images(vec![image], cx);
                    }
                    return;
                }
                this.icons.requested.remove(&candidate_id);
                let evicted = if let Some(image) = decoded {
                    this.icons.cache.insert(cache_key, image)
                } else {
                    this.icons.cache.insert_missing(cache_key)
                };
                this.drop_render_images(evicted, cx);
                cx.notify();
                this.request_application_icons(cx);
            })
            .ok();
        })
        .detach();
    }
}
