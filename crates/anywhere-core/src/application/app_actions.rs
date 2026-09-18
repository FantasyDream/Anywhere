//! Discovery lifecycle: one request per activation, generation-guarded cache and publication.
use super::launcher_service::Application;
use crate::features::app_actions::convert;
use anywhere_platform::ApplicationInfo;
use anywhere_protocol::SessionId;
use std::{sync::Arc, time::Instant};

impl Application {
    pub(super) async fn prepare_app_actions(
        &self,
        session_id: SessionId,
        app: Option<&ApplicationInfo>,
        prompt: bool,
    ) {
        let Some(app) = app.cloned() else {
            return;
        };
        let platform = self.platform.clone();
        let bundle = app.bundle_id.clone();
        let (trusted, version) = tokio::task::spawn_blocking(move || {
            let trusted = platform.accessibility_trusted(prompt);
            (
                trusted,
                if trusted {
                    platform.application_version(&bundle)
                } else {
                    None
                },
            )
        })
        .await
        .unwrap_or((false, None));
        if !self.resolver.is_session_active(session_id) {
            return;
        }
        let cached = trusted
            .then(|| self.app_actions.get(&app, version.as_deref()))
            .flatten();
        let cache_hit = cached.is_some();
        let actions =
            cached.and_then(|items| convert::registry(&self.resolver.registry, &app, items).ok());
        {
            let Ok(mut session) = self.session() else {
                return;
            };
            if session.state.session_id != session_id {
                return;
            }
            session.menu_trusted = trusted;
            if let Some(actions) = actions {
                session.actions = Arc::new(actions);
            }
        }
        tracing::info!(trusted, prompt, "accessibility_probe");
        if trusted {
            self.spawn_menu_discovery(session_id, version, cache_hit, false, None);
        }
    }

    pub fn prompt_accessibility(&self) -> anyhow::Result<bool> {
        let trusted = self.platform.accessibility_trusted(true);
        tracing::info!(trusted, prompt = true, "accessibility_prompt");
        let (session_id, changed) = {
            let mut session = self.session()?;
            let changed = session.menu_trusted != trusted;
            session.menu_trusted = trusted;
            (session.state.session_id, changed)
        };
        // Showing the window calls this too. An already granted permission must not
        // start a second scan, race the first scan, or block the Flutter UI on version IO.
        if changed {
            self.refresh_app_actions(session_id, None);
        }
        Ok(trusted)
    }

    pub(super) fn refresh_app_actions(
        &self,
        session_id: SessionId,
        expected_generation: Option<u64>,
    ) {
        self.spawn_menu_discovery(session_id, None, false, true, expected_generation);
    }

    fn spawn_menu_discovery(
        &self,
        session_id: SessionId,
        version: Option<String>,
        cache_hit: bool,
        invalidate: bool,
        expected_generation: Option<u64>,
    ) {
        let (ticket, trusted) = {
            let Ok(mut session) = self.session() else {
                return;
            };
            if session.state.session_id != session_id
                || !self.resolver.is_session_active(session_id)
            {
                return;
            }
            if expected_generation.is_some_and(|expected| expected != session.menu_generation) {
                return;
            }
            let Some(app) = session.frontmost.clone() else {
                return;
            };
            // Issued under the session lock, without taking the catalog lock.
            let ticket = self.app_actions.ticket(app);
            session.menu_generation = ticket.generation;
            if invalidate {
                session.actions = self.resolver.registry.clone();
            }
            (ticket, session.menu_trusted)
        };
        if !self.app_actions.begin(&ticket, invalidate) {
            return;
        }
        if invalidate {
            self.resolver.recompute_current(session_id);
        }
        let platform = self.platform.clone();
        let catalog = self.app_actions.clone();
        let resolver = self.resolver.clone();
        let result = std::thread::Builder::new()
            .name("app-action-discovery".into())
            .spawn(move || {
                let started = Instant::now();
                let version = version.or_else(|| {
                    if trusted {
                        platform.application_version(&ticket.app.bundle_id)
                    } else {
                        None
                    }
                });
                let result = if trusted {
                    platform.discover_menu_actions(&ticket.app)
                } else {
                    Ok(Vec::new())
                };
                let items = match result {
                    Ok(items) => Some(items),
                    Err(error) => {
                        tracing::warn!(error = %error, "app_action_discovery_failed");
                        None
                    }
                };
                let menu_item_count = items.as_ref().map_or(0, Vec::len);
                let visible_action_count = items
                    .as_ref()
                    .map_or(0, |items| items.iter().filter(|item| item.enabled).count());
                let actions = match convert::registry(
                    &resolver.registry,
                    &ticket.app,
                    items.clone().unwrap_or_default(),
                ) {
                    Ok(actions) => actions,
                    Err(error) => {
                        tracing::warn!(error = %error, "app_action_registration_failed");
                        return;
                    }
                };
                tracing::info!(
                    app_action_discovery_duration = started.elapsed().as_micros() as u64,
                    menu_item_count,
                    visible_action_count,
                    cache_hit,
                    "app_action_discovery"
                );
                let mut published = false;
                catalog.finish(&ticket, version, items, || {
                    let Ok(mut session) = resolver.session.lock() else {
                        return;
                    };
                    if session.state.session_id != session_id
                        || session.menu_generation != ticket.generation
                        || !resolver.is_session_active(session_id)
                    {
                        return;
                    }
                    session.actions = Arc::new(actions);
                    published = true;
                });
                if published {
                    resolver.recompute_current(session_id);
                }
            });
        if let Err(error) = result {
            tracing::warn!(error = %error, "app_action_discovery_spawn_failed");
        }
    }
}
