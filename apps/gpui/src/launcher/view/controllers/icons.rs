use super::*;

impl LauncherView {
    pub(super) fn request_application_icons(&mut self, cx: &mut Context<Self>) {
        if !self.presented || !matches!(self.screen, Screen::Launcher | Screen::FileSearch) {
            return;
        }
        let Some(live) = self.live.clone() else {
            return;
        };
        let visible_ids = self
            .model
            .rows
            .iter()
            .filter(|row| row.kind == CandidateKind::Application)
            .map(|row| {
                (
                    row.id.clone(),
                    row.icon_identity.clone().unwrap_or_else(|| row.id.clone()),
                )
            })
            .collect::<Vec<_>>();
        for (candidate_id, cache_key) in visible_ids {
            if self.application_icons.contains(&cache_key)
                || self.application_icons_requested.contains_key(&candidate_id)
                || self
                    .application_icons_requested
                    .values()
                    .any(|request| request.cache_key == cache_key)
                || self.application_icons_requested.len() >= MAX_APPLICATION_ICON_REQUESTS
            {
                continue;
            }
            self.application_icon_generation = self.application_icon_generation.wrapping_add(1);
            let ticket = self.application_icon_generation;
            self.application_icons_requested.insert(
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
        mut receiver: Receiver<LiveEvent>,
        candidate_id: String,
        cache_key: String,
        ticket: u64,
        cx: &mut Context<Self>,
    ) {
        cx.spawn(async move |this, cx| {
            let event = receiver.recv().await;
            this.update(cx, |this, cx| {
                if this
                    .application_icons_requested
                    .get(&candidate_id)
                    .map(|request| request.ticket)
                    != Some(ticket)
                {
                    return;
                }
                match event {
                    Some(LiveEvent::ApplicationIcon {
                        candidate_id: response_id,
                        bytes,
                    }) if response_id == candidate_id => {
                        this.application_icons_requested.remove(&candidate_id);
                        if let Some(bytes) = bytes {
                            this.application_icons.insert(
                                cache_key,
                                Arc::new(Image::from_bytes(ImageFormat::Png, bytes)),
                            );
                        } else {
                            this.application_icons.insert_missing(cache_key);
                        }
                    }
                    Some(_) | None => {
                        this.application_icons_requested.remove(&candidate_id);
                        this.application_icons.insert_missing(cache_key);
                    }
                }
                cx.notify();
                this.request_application_icons(cx);
            })
            .ok();
        })
        .detach();
    }
}
