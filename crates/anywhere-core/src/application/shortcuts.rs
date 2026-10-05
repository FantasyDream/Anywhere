//! Binding configuration uses business IDs; invocation resolves current execution rows.
use super::Application;
use anyhow::{bail, Result};
use anywhere_protocol::{CandidatePresentation, Revision, SessionId};
use std::sync::atomic::Ordering;

#[derive(Debug, Clone)]
pub struct ShortcutAction {
    pub action_id: String,
    pub title: String,
    pub context_types: Vec<String>,
}

impl Application {
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

    /// Never resolve from saved Context/Candidate IDs or ranking's first result.
    /// The caller must let the user choose when several current objects match.
    pub fn resolve_action_binding(
        &self,
        session_id: SessionId,
        revision: Revision,
        action_id: &str,
    ) -> Result<Vec<CandidatePresentation>> {
        let session = self.session()?;
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
