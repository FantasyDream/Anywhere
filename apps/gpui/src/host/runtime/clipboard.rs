use super::*;

impl LiveLauncher {
    pub fn clipboard_snapshot(&self, query: String) -> Receiver<LiveEvent> {
        let application = self.application.clone();
        self.spawn_blocking(move || {
            tracing::debug!(query_length = query.len(), "clipboard_snapshot_started");
            let snapshot = clipboard_snapshot_from(&application, Some(&query))?;
            tracing::debug!(
                item_count = snapshot.items.len(),
                "clipboard_snapshot_completed"
            );
            Ok(LiveEvent::Clipboard(snapshot))
        })
    }

    pub fn copy_clipboard(&self, id: i64, query: String) -> Receiver<LiveEvent> {
        let application = self.application.clone();
        self.spawn_blocking(move || {
            {
                let settings = application.settings().map_err(anyhow::Error::new)?;
                settings
                    .clipboard()
                    .copy(id, application.application().platform())?;
            }
            Ok(LiveEvent::Clipboard(clipboard_snapshot_from(
                &application,
                Some(&query),
            )?))
        })
    }

    pub fn delete_clipboard(&self, id: i64, query: String) -> Receiver<LiveEvent> {
        let application = self.application.clone();
        self.spawn_blocking(move || {
            {
                let settings = application.settings().map_err(anyhow::Error::new)?;
                settings.clipboard().delete(id)?;
            }
            Ok(LiveEvent::Clipboard(clipboard_snapshot_from(
                &application,
                Some(&query),
            )?))
        })
    }

    pub fn clipboard_settings_snapshot(&self) -> Receiver<LiveEvent> {
        let application = self.application.clone();
        self.spawn_blocking(move || {
            Ok(LiveEvent::Clipboard(clipboard_snapshot_from(
                &application,
                None,
            )?))
        })
    }

    pub fn clear_clipboard(&self) -> Receiver<LiveEvent> {
        let application = self.application.clone();
        self.spawn_blocking(move || {
            {
                let settings = application.settings().map_err(anyhow::Error::new)?;
                settings.clipboard().clear()?;
            }
            Ok(LiveEvent::Clipboard(clipboard_snapshot_from(
                &application,
                None,
            )?))
        })
    }

    pub fn set_clipboard_pause(&self, minutes: u32) -> Receiver<LiveEvent> {
        let application = self.application.clone();
        self.spawn_blocking(move || {
            {
                let settings = application.settings().map_err(anyhow::Error::new)?;
                settings
                    .clipboard()
                    .pause(minutes, application.application().platform())?;
            }
            Ok(LiveEvent::Clipboard(clipboard_snapshot_from(
                &application,
                None,
            )?))
        })
    }

    pub fn set_clipboard_retention(&self, days: u32) -> Receiver<LiveEvent> {
        let application = self.application.clone();
        self.spawn_blocking(move || {
            {
                let settings = application.settings().map_err(anyhow::Error::new)?;
                settings.clipboard().set_retention_days(days)?;
            }
            Ok(LiveEvent::Clipboard(clipboard_snapshot_from(
                &application,
                None,
            )?))
        })
    }
}
