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
            session
                .resolution
                .search_candidates
                .retain(|c| !crate::features::filesystem::search::is_search_target(c));
            session
                .resolution
                .search_sources
                .retain(|s| s.id != "files");
            session
                .execution
                .candidates
                .retain(|_, c| !crate::features::filesystem::search::is_search_target(c));
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

    /// Append unseen targets from query-local storage without replacing displayed rows.
    pub async fn load_more_files(
        &self,
        session_id: SessionId,
        revision: Revision,
    ) -> Result<Option<LauncherState>> {
        let generation = self
            .file_search_policy
            .read()
            .map_err(|_| anyhow::anyhow!("search settings unavailable"))?
            .generation;
        let (query_id, displayed) = {
            let mut session = self.session()?;
            if !self.resolver.is_current(session_id, revision)
                || session.input_mode != InputMode::FileSearch
            {
                bail!("file search expired");
            }
            let query_id = session
                .resolution
                .file_search
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("file search not ready"))?
                .query_id;
            session.resolution.file_results_frozen = true;
            let displayed = session
                .resolution
                .search_candidates
                .iter()
                .filter_map(|candidate| candidate.context()?.value.path().map(PathBuf::from))
                .collect();
            (query_id, displayed)
        };
        let ticket = self.resolver.execution(session_id, revision);
        let batch = self
            .file_search_source
            .more(query_id, displayed, ticket.clone())
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
                append_results: true,
            },
            batch,
        );
        Ok(self.latest_state(session_id))
    }
}

struct SourcePublication<'a> {
    id: &'a str,
    append_results: bool,
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
    let SourcePublication {
        id: source,
        append_results,
    } = publication;
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
        // Appends and their publication share the policy/session lock.
        // A scan update arriving during disk reading must not undo user navigation.
        if append_results {
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
                batch.candidates = session.resolution.search_candidates.clone();
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
        let prior_rows = session.resolution.search_candidates.len();
        if append_results {
            let mut displayed: std::collections::HashSet<_> = session
                .resolution
                .search_candidates
                .iter()
                .filter_map(|c| c.context()?.value.path().map(PathBuf::from))
                .collect();
            session
                .resolution
                .search_candidates
                .extend(batch.candidates.into_iter().filter(|c| {
                    c.context()
                        .and_then(|context| context.value.path())
                        .is_some_and(|path| displayed.insert(path.to_owned()))
                }));
        } else if frozen {
            if let (Some(old), Some(new)) = (&session.resolution.file_search, &mut file_search) {
                new.next_cursor = old.next_cursor.clone();
                new.previous_cursor = old.previous_cursor.clone();
                new.page = old.page;
                new.needs_refresh = old.needs_refresh
                    || new.confirmed_matches != old.confirmed_matches
                    || new.finished != old.finished;
            }
        } else {
            // Once visible, rows keep their position for this revision. Initial
            // batches may improve ranking, but only unseen targets are appended.
            // Automatic head growth remains bounded; more() reaches all matches.
            let mut displayed: std::collections::HashSet<_> = session
                .resolution
                .search_candidates
                .iter()
                .filter_map(|c| c.context()?.value.path().map(PathBuf::from))
                .collect();
            for candidate in batch.candidates {
                if session.resolution.search_candidates.len()
                    >= crate::features::filesystem::scan::PAGE_SIZE
                {
                    break;
                }
                if candidate
                    .context()
                    .and_then(|c| c.value.path())
                    .is_some_and(|path| displayed.insert(path.to_owned()))
                {
                    session.resolution.search_candidates.push(candidate);
                }
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
        if append_results {
            session.resolution.file_results_frozen = true;
        }
        session.note_snapshot_inputs();
        let committed = session.resolution.committed_revision == Some(revision);
        if committed
            && (frozen || append_results)
            && session.execution.action_catalog_version
                == Some(
                    resolver
                        .action_catalog_version
                        .load(std::sync::atomic::Ordering::Acquire),
                )
        {
            if append_results {
                // Revalidate only the new offers; existing rows retain the same
                // definition/Revision credentials. Catalog changes take the full path.
                let registry = session.actions.clone();
                let facts = session.facts();
                let catalog = resolver
                    .action_catalog_version
                    .load(std::sync::atomic::Ordering::Acquire);
                let mut rows = session.resolution.search_candidates[prior_rows..].to_vec();
                super::snapshot::validate_ready_candidates(&mut rows, &registry, &facts);
                for row in &mut rows {
                    if catalog > 1 {
                        row.presentation.id.push_str(&format!(":catalog:{catalog}"));
                    }
                }
                std::sync::Arc::make_mut(&mut session.state.candidates)
                    .extend(rows.iter().map(|c| c.presentation.clone()));
                session.execution.candidates.extend(
                    rows.into_iter()
                        .map(|c| (c.presentation.id.clone().into(), c)),
                );
            }
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
                            append_results: false,
                        },
                        batch,
                    )
                }),
            );
        }
    }
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
                        append_results: false,
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
