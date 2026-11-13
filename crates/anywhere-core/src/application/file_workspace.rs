//! Debounced async sources enrich only the query and policy that started them.
use super::Application;
use crate::core::search::candidate_source::AsyncCandidateSource;
use anyhow::{bail, Result};
use anywhere_protocol::{
    InputMode, LauncherEvent, LauncherState, Revision, SearchSourcePresentation, SearchSourceState,
    SessionId,
};
use std::path::PathBuf;

impl Application {
    /// Reconfiguration revokes cached file targets and running search publications.
    pub fn set_file_search(
        &self,
        enabled: bool,
        roots: Vec<PathBuf>,
        names: Vec<String>,
    ) -> Result<()> {
        self.configure_file_search(
            enabled,
            anywhere_platform::FileSearchOptions {
                roots,
                excluded_directory_names: names,
                ..Default::default()
            },
        )
    }

    pub fn configure_file_search(
        &self,
        enabled: bool,
        options: anywhere_platform::FileSearchOptions,
    ) -> Result<()> {
        let anywhere_platform::FileSearchOptions {
            roots,
            excluded_directory_names: names,
            engine,
            include_hidden,
            restrict_to_roots,
        } = options;
        if enabled && restrict_to_roots && roots.is_empty() {
            bail!("Authorize at least one search directory before enabling file search");
        }
        if roots.len() > anywhere_protocol::policy::MAX_SEARCH_ROOTS as usize
            || names.len() > anywhere_protocol::policy::MAX_SEARCH_EXCLUSIONS as usize
        {
            bail!("Use at most 8 directories and 32 exclusions");
        }
        let mut canonical = Vec::new();
        for root in roots {
            if !root.is_absolute() {
                bail!("Search directories must be absolute paths");
            }
            if root.as_os_str().len() > anywhere_protocol::policy::MAX_SEARCH_PATH_BYTES as usize
                || root.to_string_lossy().chars().any(char::is_control)
            {
                bail!("Search directory is invalid");
            }
            if !canonical.contains(&root) {
                canonical.push(root);
            }
        }
        if names.iter().any(|n| {
            n.trim().is_empty()
                || n.len() > anywhere_protocol::policy::MAX_SEARCH_EXCLUSION_BYTES as usize
                || n.contains(['/', '\\'])
                || n.chars().any(char::is_control)
                || n == "."
                || n == ".."
        }) {
            bail!("Exclusions must be directory names, one per line");
        }
        let mut names = names;
        names.sort();
        names.dedup();
        let options = anywhere_platform::FileSearchOptions {
            roots: canonical,
            excluded_directory_names: names,
            engine,
            include_hidden,
            restrict_to_roots,
        };
        let id = {
            let mut policy = self
                .file_search_policy
                .write()
                .map_err(|_| anyhow::anyhow!("Search settings unavailable"))?;
            if policy.enabled == enabled && policy.options == options {
                return Ok(());
            }
            self.file_search_source.clear();
            policy.enabled = enabled;
            policy.options = options;
            policy.generation = policy.generation.wrapping_add(1);
            let mut session = self.session()?;
            session.resolution.object_actions.clear();
            for rows in session.resolution.source_candidates.values_mut() {
                rows.retain(|c| !crate::features::filesystem::search::is_search_target(c));
            }
            session.resolution.file_search = None;
            session.resolution.file_results_frozen = false;
            std::sync::Arc::make_mut(&mut session.resolution.search_candidates)
                .retain(|c| !crate::features::filesystem::search::is_search_target(c));
            session
                .resolution
                .search_sources
                .retain(|s| s.id != "files");
            session
                .execution
                .candidates
                .retain(|_, c| !crate::features::filesystem::search::is_search_target(c));
            session.execution.file_candidate_indices.clear();
            session.note_snapshot_inputs();
            session.state.session_id
        };
        self.resolver.recompute_current(id);
        Ok(())
    }

    pub fn freeze_file_results(&self, session_id: SessionId, revision: Revision) -> Result<()> {
        let mut session = self.session()?;
        if !self.resolver.is_current(session_id, revision)
            || session.input_mode != InputMode::FileSearch
        {
            bail!("file search expired");
        }
        session.resolution.file_results_frozen = true;
        Ok(())
    }

    /// Read the page immediately after the current file-search window.
    pub async fn load_more_files(
        &self,
        session_id: SessionId,
        revision: Revision,
    ) -> Result<Option<LauncherState>> {
        let cursor = {
            let session = self.session()?;
            if !self.resolver.is_current(session_id, revision)
                || session.input_mode != InputMode::FileSearch
            {
                bail!("file search expired");
            }
            session
                .resolution
                .file_search
                .as_ref()
                .and_then(|progress| progress.next_cursor.clone())
        };
        let Some(cursor) = cursor else {
            return Ok(self.latest_state(session_id));
        };
        self.load_file_page(session_id, revision, cursor).await
    }

    /// Replace the retained window with an opaque, query-bound result page.
    ///
    /// The scan store remains the source of truth. Only this page becomes part
    /// of the execution table and Flutter snapshot, so visited pages cannot
    /// grow session memory without a bound.
    pub async fn load_file_page(
        &self,
        session_id: SessionId,
        revision: Revision,
        cursor: String,
    ) -> Result<Option<LauncherState>> {
        let generation = self
            .file_search_policy
            .read()
            .map_err(|_| anyhow::anyhow!("search settings unavailable"))?
            .generation;
        let query_id = {
            let session = self.session()?;
            if !self.resolver.is_current(session_id, revision)
                || session.input_mode != InputMode::FileSearch
            {
                bail!("file search expired");
            }
            session
                .resolution
                .file_search
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("file search not ready"))?
                .query_id
        };
        let ticket = self.resolver.execution(session_id, revision);
        let batch = self
            .file_search_source
            .page(query_id, cursor, ticket.clone())
            .await?;
        if !ticket.is_current() {
            return Ok(None);
        }
        commit_file_batch(
            &self.resolver,
            &self.file_search_policy,
            generation,
            session_id,
            revision,
            SourcePublication {
                id: "files",
                kind: FilePublication::Page,
            },
            batch,
        );
        Ok(self.latest_state(session_id))
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum FilePublication {
    LatestHead,
    Page,
}

struct SourcePublication<'a> {
    id: &'a str,
    kind: FilePublication,
}

fn commit_file_batch(
    resolver: &super::session_runtime::Resolver,
    policy: &std::sync::RwLock<crate::features::filesystem::search::FileSearchPolicy>,
    generation: u64,
    session_id: SessionId,
    revision: Revision,
    publication: SourcePublication<'_>,
    mut batch: crate::core::search::candidate_source::AsyncCandidateBatch,
) {
    let SourcePublication { id: source, kind } = publication;
    let mut file_search = batch
        .metadata::<anywhere_protocol::FileSearchProgress>()
        .cloned();
    let mut fast_published = false;
    let committed = {
        let Ok(guard) = policy.read() else {
            return;
        };
        if (guard.generation != generation || !guard.enabled)
            || !resolver.is_current(session_id, revision)
        {
            return;
        }
        let Ok(mut session) = resolver.session.lock() else {
            return;
        };
        if session.state.session_id != session_id
            || session.state.revision != revision
            || !resolver.is_current(session_id, revision)
        {
            return;
        }
        // Page reads and their publication share the policy/session lock.
        // A scan progress update arriving during disk reading must not undo
        // user navigation or claim older coverage.
        if let (Some(old), Some(new)) = (&session.resolution.file_search, &mut file_search) {
            if old.query_id != new.query_id {
                return;
            }
            if old.scanned_entries > new.scanned_entries
                || old.confirmed_matches > new.confirmed_matches
                || (old.finished && !new.finished)
            {
                new.needs_refresh = true;
                new.scanned_entries = old.scanned_entries;
                new.confirmed_matches = old.confirmed_matches;
                new.finished = old.finished;
                new.issue_count = old.issue_count;
                new.inaccessible_count = old.inaccessible_count;
                new.issue_examples = old.issue_examples.clone();
                batch.state = if !new.finished {
                    SearchSourceState::Loading
                } else if new.issue_count > 0 {
                    SearchSourceState::Partial
                } else {
                    SearchSourceState::Ready
                };
            }
        }
        if batch.state == SearchSourceState::Unavailable && file_search.is_none() {
            if let Some(old) = &session.resolution.file_search {
                let mut progress = old.clone();
                progress.finished = true;
                progress.issue_count += 1;
                if progress.issue_examples.len() < 8 {
                    progress
                        .issue_examples
                        .push("File search interrupted. Retry to search the full range.".into());
                }
                file_search = Some(progress);
                batch.state = SearchSourceState::Partial;
                batch.candidates = session.resolution.search_candidates.to_vec();
            }
        }
        if let (Some(old), Some(new)) = (&session.resolution.file_search, &file_search) {
            if old.query_id != new.query_id
                || new.scanned_entries < old.scanned_entries
                || (old.finished && !new.finished)
            {
                return;
            }
        }
        let frozen = session.resolution.file_results_frozen;
        let committed = session.resolution.committed_revision == Some(revision);
        let replace_window = kind == FilePublication::Page || !frozen;
        if !replace_window {
            if let (Some(old), Some(new)) = (&session.resolution.file_search, &mut file_search) {
                new.next_cursor = old.next_cursor.clone();
                new.previous_cursor = old.previous_cursor.clone();
                new.page = old.page;
                new.needs_refresh = old.needs_refresh
                    || new.confirmed_matches != old.confirmed_matches
                    || new.finished != old.finished;
            }
        }
        if file_search.is_some() {
            session.resolution.file_search = file_search;
        }
        if let Some(s) = session
            .resolution
            .search_sources
            .iter_mut()
            .find(|s| s.id == source)
        {
            s.state = batch.state;
        }
        if replace_window {
            let mut rows = unseen_file_candidates(
                &[],
                std::mem::take(&mut batch.candidates),
                crate::features::filesystem::scan::PAGE_SIZE,
            );
            if committed
                && session.execution.action_catalog_version == Some(resolver.catalog.version())
            {
                let registry = session.actions.clone();
                let facts = session.facts();
                let catalog = resolver.catalog.version();
                super::snapshot::validate_ready_candidates(&mut rows, &registry, &facts);
                for row in &mut rows {
                    if catalog > 1 {
                        row.presentation.id.push_str(&format!(":catalog:{catalog}"));
                    }
                }
            }
            let rows = std::sync::Arc::new(rows);
            let presentations = std::sync::Arc::new(
                rows.iter()
                    .map(|candidate| candidate.presentation.clone())
                    .collect::<Vec<_>>(),
            );
            let rows_changed = *session.state.candidates != *presentations;
            session.resolution.search_candidates = rows.clone();
            session.execution.file_candidate_indices = rows
                .iter()
                .enumerate()
                .map(|(index, candidate)| (candidate.presentation.id.clone().into(), index))
                .collect();
            if rows_changed {
                session.state.candidates = presentations;
                session.state.candidate_list_version =
                    session.state.candidate_list_version.saturating_add(1);
            }
        }
        if kind == FilePublication::Page {
            session.resolution.file_results_frozen = true;
        }
        session.note_snapshot_inputs();
        if committed
            && (frozen || kind == FilePublication::Page)
            && session.execution.action_catalog_version == Some(resolver.catalog.version())
        {
            // Metadata updates retain the immutable rows and execution map.
            session.resolution.snapshot_seq = session.resolution.snapshot_seq.saturating_add(1);
            session.state.snapshot_seq = session.resolution.snapshot_seq;
            session.state.file_search = session.resolution.file_search.clone().map(Box::new);
            session.state.search_sources = session.resolution.search_sources.clone();
            let state = session.state.clone();
            // Publish while holding the session/policy guards, just like a full
            // commit. Unlocking first could publish past a new query or session.
            resolver
                .latest_tx
                .send_replace(Some(std::sync::Arc::new(state.clone())));
            let _ = resolver
                .events
                .send(LauncherEvent::ActionsUpdated { state });
            fast_published = true;
        }
        committed && !fast_published
    };
    if committed {
        resolver.recompute_current(session_id);
    }
}

impl Application {
    pub(super) fn spawn_file_search_sources(
        &self,
        session_id: SessionId,
        revision: Revision,
        input: &str,
        sources: Vec<
            std::sync::Arc<dyn crate::core::search::candidate_source::AsyncCandidateSource>,
        >,
    ) {
        let resolver = self.resolver.clone();
        let policy = self.file_search_policy.clone();
        let Ok(guard) = policy.read() else {
            return;
        };
        if !guard.enabled {
            return;
        }
        let generation = guard.generation;
        drop(guard);
        if sources.is_empty() {
            if crate::features::filesystem::search::eligible_query(input) {
                super::search_sources::mark_sources_loading(
                    &resolver,
                    session_id,
                    revision,
                    &[SearchSourcePresentation {
                        id: self.file_search_source.id().into(),
                        state: SearchSourceState::Unavailable,
                    }],
                );
            }
            return;
        }
        let filter = resolver
            .session
            .lock()
            .map(|s| s.file_filter)
            .unwrap_or_default();
        super::search_sources::mark_sources_loading(
            &resolver,
            session_id,
            revision,
            &sources
                .iter()
                .map(|s| SearchSourcePresentation {
                    id: s.id().into(),
                    state: SearchSourceState::Loading,
                })
                .collect::<Vec<_>>(),
        );
        for source in sources {
            let id = source.id();
            let r = resolver.clone();
            let p = policy.clone();
            let valid_policy = policy.clone();
            super::source_runtime::spawn_source(
                source,
                crate::core::search::candidate_source::AsyncSearchRequest {
                    text: input.into(),
                    options: Some(std::sync::Arc::new(
                        crate::features::filesystem::search::FileSearchRequest { filter },
                    )),
                },
                resolver.execution(session_id, revision),
                resolver.source_workers.clone(),
                std::sync::Arc::new(move || {
                    valid_policy
                        .read()
                        .is_ok_and(|p| p.generation == generation && p.enabled)
                }),
                std::sync::Arc::new(move |batch| {
                    commit_file_batch(
                        &r,
                        &p,
                        generation,
                        session_id,
                        revision,
                        SourcePublication {
                            id,
                            kind: FilePublication::LatestHead,
                        },
                        batch,
                    )
                }),
            );
        }
    }
}

/// Deduplicate against borrowed paths first. This avoids allocating a PathBuf
/// copy for every already displayed candidate on each progress publication.
fn unseen_file_candidates(
    displayed: &[crate::core::search::candidate::Candidate],
    incoming: Vec<crate::core::search::candidate::Candidate>,
    maximum_rows: usize,
) -> Vec<crate::core::search::candidate::Candidate> {
    use std::path::Path;
    let mut paths: std::collections::HashSet<&Path> = displayed
        .iter()
        .filter_map(|candidate| candidate.context()?.value.path())
        .collect();
    let mut keep = Vec::with_capacity(incoming.len());
    let mut accepted = 0usize;
    for candidate in &incoming {
        if displayed.len() + accepted >= maximum_rows {
            break;
        }
        let unseen = candidate
            .context()
            .and_then(|context| context.value.path())
            .is_some_and(|path| paths.insert(path));
        keep.push(unseen);
        if unseen {
            accepted += 1;
        }
    }
    drop(paths);
    incoming
        .into_iter()
        .zip(keep)
        .filter_map(|(candidate, keep)| keep.then_some(candidate))
        .collect()
}

#[cfg(test)]
mod progress_tests {
    use super::*;
    use crate::application::session_runtime::Resolver;
    use crate::core::{action::descriptor::ActionRegistry, ranking::usage::MemoryUsage};
    use std::sync::{Arc, RwLock};
    #[test]
    fn progress_commits_reuse_large_row_arrays_and_do_not_rebuild_execution() {
        for count in [1000, 10000] {
            let resolver =
                Resolver::new(ActionRegistry::default(), Box::new(MemoryUsage::default()));
            resolver.reset_session(1);
            let sample: anywhere_protocol::CandidatePresentation =
                serde_json::from_value(serde_json::json!({
                    "id":"fixture", "kind":"file", "title":"report.txt", "icon":{"key":"file"},
                    "score":0.0,"group_id":"files","availability":{"state":"enabled"}
                }))
                .unwrap();
            let rows = Arc::new(vec![sample; count]);
            {
                let mut session = resolver.session.lock().unwrap();
                session.state.session_id = 1;
                session.input_mode = InputMode::FileSearch;
                session.state.candidates = rows.clone();
                session.state.candidate_list_version = 1;
                session.resolution.committed_revision = Some(0);
                session.resolution.file_results_frozen = true;
                session.resolution.file_search = Some(anywhere_protocol::FileSearchProgress {
                    query_id: 77,
                    ..Default::default()
                });
                session.execution.action_catalog_version = Some(1);
            }
            let policy =
                RwLock::new(crate::features::filesystem::search::FileSearchPolicy::default());
            let start = std::time::Instant::now();
            for scanned in 1..=100 {
                commit_file_batch(
                    &resolver,
                    &policy,
                    0,
                    1,
                    0,
                    SourcePublication {
                        id: "files",
                        kind: FilePublication::LatestHead,
                    },
                    crate::core::search::candidate_source::AsyncCandidateBatch {
                        update: crate::core::search::candidate_source::BatchUpdate::Replace,
                        candidates: vec![],
                        state: SearchSourceState::Loading,
                        metadata: Some(Arc::new(anywhere_protocol::FileSearchProgress {
                            query_id: 77,
                            scanned_entries: scanned,
                            ..Default::default()
                        })),
                    },
                );
            }
            println!(
                "{count} retained rows / 100 progress commits: {}us",
                start.elapsed().as_micros()
            );
            let state = resolver.latest_tx.borrow().clone().unwrap();
            assert!(Arc::ptr_eq(&state.candidates, &rows));
            assert_eq!(state.snapshot_seq, 100);
            assert!(resolver
                .session
                .lock()
                .unwrap()
                .execution
                .candidates
                .is_empty());
            assert_eq!(state.file_search.as_ref().unwrap().scanned_entries, 100);
        }
    }
}
