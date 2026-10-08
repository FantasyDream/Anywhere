//! Binding configuration uses business IDs; invocation resolves current execution rows.
use super::{session_model::LauncherSession, Application};
use anyhow::{bail, Result};
use anywhere_protocol::{CandidatePresentation, LauncherState, Revision, SessionId};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::atomic::Ordering,
};

#[derive(Debug, Clone)]
pub struct ShortcutAction {
    pub action_id: String,
    pub title: String,
    pub context_types: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct BindingResolution {
    pub state: LauncherState,
    pub candidates: Vec<CandidatePresentation>,
    pub requires_choice: bool,
}

#[derive(Debug, Clone)]
pub struct ShortcutRoute {
    pub action_id: String,
    pub app_bundle_id: Option<String>,
    pub context_type: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ShortcutApplication {
    pub bundle_id: String,
    pub title: String,
}

impl Application {
    /// Catalog eligibility is independent of the active project and availability.
    pub fn supported_shortcut_actions(&self, ids: &[String]) -> Result<Vec<String>> {
        if ids.len() > 2048 || ids.iter().any(|id| id.is_empty() || id.len() > 2048) {
            bail!("Invalid shortcut catalog request.");
        }
        let registry = self.resolver.base_registry();
        Ok(ids
            .iter()
            .filter(|id| registry.supports_persistent_binding(id))
            .cloned()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect())
    }

    /// Includes static actions and scripts from the last captured context. No object
    /// values or execution capabilities cross this configuration boundary.
    pub fn shortcut_actions(&self) -> Result<Vec<ShortcutAction>> {
        let session = self.session()?;
        let (actions, _) = self
            .resolver
            .base_registry()
            .for_contexts(&session.resolution.committed_contexts);
        let mut options: Vec<_> = actions
            .commands()
            .filter_map(|action| {
                let binding = actions.persistent_binding(&action.id).ok()?;
                Some(ShortcutAction {
                    action_id: binding.action_id.as_str().into(),
                    title: action.title.clone(),
                    context_types: binding.context_types,
                })
            })
            .collect();
        options.sort_by(|a, b| a.title.cmp(&b.title).then(a.action_id.cmp(&b.action_id)));
        Ok(options)
    }

    /// Configuration labels only; never send captured paths or process identities.
    pub fn shortcut_applications(&self) -> Result<Vec<ShortcutApplication>> {
        let mut apps: BTreeMap<String, String> = self
            .resolver
            .catalog
            .lock()
            .map_err(|_| anyhow::anyhow!("application catalog lock poisoned"))?
            .full
            .applications()
            .iter()
            .map(|a| (a.bundle_id.clone(), a.name.clone()))
            .collect();
        if let Some(app) = &self.session()?.identity.frontmost {
            apps.insert(app.bundle_id.clone(), app.name.clone());
        }
        let mut apps: Vec<_> = apps
            .into_iter()
            .map(|(bundle_id, title)| ShortcutApplication { bundle_id, title })
            .collect();
        apps.sort_by(|a, b| a.title.cmp(&b.title).then(a.bundle_id.cmp(&b.bundle_id)));
        Ok(apps)
    }

    /// Never resolve from saved Context/Candidate IDs or ranking's first result.
    /// The caller must let the user choose when several current objects match.
    pub fn resolve_action_binding(
        &self,
        session_id: SessionId,
        revision: Revision,
        action_id: &str,
    ) -> Result<Vec<CandidatePresentation>> {
        let session = self.session()?;
        self.binding_candidates(&session, session_id, revision, action_id)
    }

    /// A global invocation waits for this new session's captures, never the previous
    /// one. Snapshot and execution rows are returned under the same session lock.
    /// Supplemental completion does not always publish a snapshot, hence the bounded
    /// readiness timer alongside cancellation checks instead of waiting only on UI updates.
    pub async fn wait_action_binding(
        &self,
        session_id: SessionId,
        revision: Revision,
        action_id: &str,
    ) -> Result<BindingResolution> {
        self.wait_binding(session_id, revision, |session| {
            self.binding_candidates(session, session_id, revision, action_id)
                .map(|candidates| (candidates, false))
        })
        .await
    }

    pub fn resolve_shortcut_intent(
        &self,
        session_id: SessionId,
        revision: Revision,
        routes: &[ShortcutRoute],
    ) -> Result<BindingResolution> {
        validate_routes(routes)?;
        let session = self.session()?;
        let (candidates, requires_choice) =
            self.intent_candidates(&session, session_id, revision, routes)?;
        Ok(BindingResolution {
            state: session.state.clone(),
            candidates,
            requires_choice,
        })
    }

    pub async fn wait_shortcut_intent(
        &self,
        session_id: SessionId,
        revision: Revision,
        routes: &[ShortcutRoute],
    ) -> Result<BindingResolution> {
        validate_routes(routes)?;
        self.wait_binding(session_id, revision, |session| {
            self.intent_candidates(session, session_id, revision, routes)
        })
        .await
    }

    async fn wait_binding(
        &self,
        session_id: SessionId,
        revision: Revision,
        resolve: impl Fn(&LauncherSession) -> Result<(Vec<CandidatePresentation>, bool)>,
    ) -> Result<BindingResolution> {
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            {
                let session = self.session()?;
                if !self.resolver.execution(session_id, revision).is_current()
                    || session.state.session_id != session_id
                    || session.state.revision != revision
                {
                    bail!("Shortcut context expired. Reopen Anywhere and try again.");
                }
                if session.state.context_complete && !session.supplemental_pending {
                    let (candidates, requires_choice) = resolve(&session)?;
                    return Ok(BindingResolution {
                        state: session.state.clone(),
                        candidates,
                        requires_choice,
                    });
                }
            }
            if tokio::time::Instant::now() >= deadline {
                bail!("Context capture timed out. No action was executed. Check context permissions and try again.");
            }
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
    }

    fn intent_candidates(
        &self,
        session: &LauncherSession,
        session_id: SessionId,
        revision: Revision,
        routes: &[ShortcutRoute],
    ) -> Result<(Vec<CandidatePresentation>, bool)> {
        self.check_binding_session(session, session_id, revision)?;
        // Match conditions before testing availability. A disabled specific rule must
        // never silently select the generic command instead.
        let matching: Vec<_> = routes
            .iter()
            .filter(|route| {
                route.app_bundle_id.as_ref().is_none_or(|id| {
                    session
                        .identity
                        .frontmost
                        .as_ref()
                        .is_some_and(|app| &app.bundle_id == id)
                }) && route.context_type.as_ref().is_none_or(|ty| {
                    session
                        .resolution
                        .committed_contexts
                        .iter()
                        .any(|ctx| &ctx.type_id == ty)
                })
            })
            .collect();
        let priority = matching
            .iter()
            .map(|r| r.app_bundle_id.is_some() as u8 + r.context_type.is_some() as u8)
            .max()
            .ok_or_else(|| {
                anyhow::anyhow!("No shortcut rule matches the current app and context.")
            })?;
        let selected: Vec<_> = matching
            .into_iter()
            .filter(|r| {
                r.app_bundle_id.is_some() as u8 + r.context_type.is_some() as u8 == priority
            })
            .collect();
        let mut candidates = BTreeMap::new();
        for route in &selected {
            // Invalid or removed actions fail closed, including in an ambiguous rule set.
            let offers =
                self.binding_candidates(session, session_id, revision, &route.action_id)?;
            let offers: Vec<_> = offers
                .into_iter()
                .filter(|offer| {
                    route.context_type.as_ref().is_none_or(|ty| {
                        session
                            .execution
                            .candidates
                            .values()
                            .find(|c| c.presentation.id == offer.id)
                            .and_then(|c| c.context())
                            .is_some_and(|ctx| &ctx.type_id == ty)
                    })
                })
                .collect();
            if offers.is_empty() {
                bail!("The matching shortcut action has no available target of the required context type.");
            }
            for offer in offers {
                candidates.insert(offer.id.clone(), offer);
            }
        }
        let mut candidates: Vec<_> = candidates.into_values().collect();
        candidates.sort_by(|a, b| b.score.total_cmp(&a.score).then(a.id.cmp(&b.id)));
        Ok((candidates, selected.len() > 1))
    }

    fn check_binding_session(
        &self,
        session: &LauncherSession,
        session_id: SessionId,
        revision: Revision,
    ) -> Result<()> {
        if !self.resolver.execution(session_id, revision).is_current()
            || session.state.session_id != session_id
            || session.execution.revision != Some(revision)
            || session.execution.action_catalog_version
                != Some(self.resolver.action_catalog_version.load(Ordering::Acquire))
        {
            bail!("Shortcut context expired. Reopen Anywhere and try again.");
        }
        if !session.state.context_complete || session.supplemental_pending {
            bail!("Context is still loading. Try the shortcut again when it is ready.");
        }
        Ok(())
    }

    fn binding_candidates(
        &self,
        session: &LauncherSession,
        session_id: SessionId,
        revision: Revision,
        action_id: &str,
    ) -> Result<Vec<CandidatePresentation>> {
        self.check_binding_session(session, session_id, revision)?;
        let (actions, _) = session
            .actions
            .for_contexts(&session.resolution.committed_contexts);
        actions.persistent_binding(action_id)?;
        let mut candidates: Vec<_> = session
            .execution
            .candidates
            .values()
            .filter(|c| {
                c.presentation.command_id.as_deref() == Some(action_id)
                    && c.presentation.availability.is_executable()
            })
            .map(|c| c.presentation.clone())
            .collect();
        candidates.sort_by(|a, b| b.score.total_cmp(&a.score).then(a.id.cmp(&b.id)));
        if candidates.is_empty() {
            let reason = session.execution.candidates.values().find_map(|c| {
                (c.presentation.command_id.as_deref() == Some(action_id))
                    .then(|| c.presentation.availability.reason.clone())
                    .flatten()
            });
            bail!(
                "{}",
                reason
                    .as_deref()
                    .unwrap_or("This shortcut has no available action for the current context.")
            );
        }
        Ok(candidates)
    }
}

fn validate_routes(routes: &[ShortcutRoute]) -> Result<()> {
    if routes.is_empty() || routes.len() > 16 {
        bail!("An intent needs between 1 and 16 rules.");
    }
    let mut conditions = BTreeSet::new();
    for route in routes {
        if route.action_id.is_empty()
            || route.action_id.len() > 2048
            || route
                .app_bundle_id
                .as_ref()
                .is_some_and(|id| !valid_condition(id, 256))
            || route
                .context_type
                .as_ref()
                .is_some_and(|id| !valid_condition(id, 128))
        {
            bail!("Invalid shortcut rule identity or condition.");
        }
        if !conditions.insert((&route.app_bundle_id, &route.context_type)) {
            bail!("Shortcut rules must use distinct app and context conditions.");
        }
    }
    Ok(())
}
fn valid_condition(id: &str, max: usize) -> bool {
    !id.is_empty()
        && id.len() <= max
        && id
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"._-".contains(&c))
}
