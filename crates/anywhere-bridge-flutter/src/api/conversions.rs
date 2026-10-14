//! Explicit protocol conversions keep Flutter types out of Core.
use super::*;

impl From<ShortcutRouteDto> for anywhere_core::application::ShortcutRoute {
    fn from(route: ShortcutRouteDto) -> Self {
        Self {
            action_id: route.action_id,
            app_bundle_id: route.app_bundle_id,
            context_type: route.context_type,
        }
    }
}

pub(super) fn binding_resolution_dto(
    resolved: anywhere_core::application::BindingResolution,
) -> BindingResolutionDto {
    BindingResolutionDto {
        state: resolved.state.into(),
        candidates: resolved.candidates.into_iter().map(candidate_dto).collect(),
        requires_choice: resolved.requires_choice,
    }
}

impl From<protocol::ExecutionSessionSummary> for ExecutionSessionSummaryDto {
    fn from(summary: protocol::ExecutionSessionSummary) -> Self {
        use protocol::ExecutionSessionStatus::*;
        Self {
            id: summary.id,
            title: summary.title,
            directory: summary.directory,
            command: summary.command,
            status: match summary.status {
                Running => ExecutionSessionStatusDto::Running,
                Stopping => ExecutionSessionStatusDto::Stopping,
                Succeeded => ExecutionSessionStatusDto::Succeeded,
                Failed => ExecutionSessionStatusDto::Failed,
                Stopped => ExecutionSessionStatusDto::Stopped,
            },
            exit_code: summary.exit_code,
            started_at_ms: summary.started_at_ms,
            finished_at_ms: summary.finished_at_ms,
            output_revision: summary.output_revision,
            output_truncated: summary.output_truncated,
            error: summary.error,
        }
    }
}

impl From<protocol::LauncherState> for LauncherStateDto {
    fn from(state: protocol::LauncherState) -> Self {
        Self {
            session_id: state.session_id,
            revision: state.revision,
            snapshot_seq: state.snapshot_seq,
            candidate_list_version: state.candidate_list_version,
            input: state.input,
            context_complete: state.context_complete,
            result_actions: state
                .result_actions
                .into_iter()
                .map(candidate_dto)
                .collect(),
            file_search: state.file_search.map(|p| FileSearchProgressDto {
                query_id: p.query_id,
                roots: p.roots,
                scanned_entries: p.scanned_entries,
                confirmed_matches: p.confirmed_matches,
                finished: p.finished,
                issue_count: p.issue_count,
                inaccessible_count: p.inaccessible_count,
                issue_examples: p.issue_examples,
                next_cursor: p.next_cursor,
                previous_cursor: p.previous_cursor,
                page: p.page,
                needs_refresh: p.needs_refresh,
            }),
            search_sources: state
                .search_sources
                .into_iter()
                .map(|s| SearchSourceDto {
                    id: s.id,
                    state: match s.state {
                        protocol::SearchSourceState::Loading => SearchSourceStateDto::Loading,
                        protocol::SearchSourceState::Ready => SearchSourceStateDto::Ready,
                        protocol::SearchSourceState::Partial => SearchSourceStateDto::Partial,
                        protocol::SearchSourceState::Unavailable => {
                            SearchSourceStateDto::Unavailable
                        }
                    },
                })
                .collect(),
            contexts: state
                .context_summary
                .into_iter()
                .map(|context| ContextDto {
                    id: context.id,
                    type_id: context.type_id,
                    label: context.label,
                    detail: context.detail,
                    source: context_source_dto(context.source),
                    key: context.key,
                    root_key: context.root_key,
                    icon_key: context.icon_key,
                    capabilities: context.capabilities,
                    candidate_ids: context.candidate_ids,
                    primary_candidate_id: context.primary_candidate_id,
                })
                .collect(),
            context_candidates: state
                .context_candidates
                .into_iter()
                .map(candidate_dto)
                .collect(),
            preferred_context_key: state.preferred_context_key,
            selected_scope: state.selected_scope,
            scopes: state
                .scopes
                .into_iter()
                .map(|scope| ScopeDto {
                    id: scope.id,
                    label: scope.label,
                    is_current_app: scope.is_current_app,
                    action_ids: scope.action_ids,
                    menu_ids: scope.menu_ids,
                })
                .collect(),
            context_captures: state
                .context_captures
                .into_iter()
                .map(|capture| ContextCaptureDto {
                    text: capture.text,
                    clipboard_age_ms: capture.clipboard_age_ms,
                    source: context_source_dto(capture.source),
                    status: context_capture_status_dto(capture.status),
                    captured_at_ms: capture.captured_at_ms,
                })
                .collect(),
            candidates: state
                .candidates
                .iter()
                .cloned()
                .map(candidate_dto)
                .collect(),
            diagnostics: state.diagnostics,
        }
    }
}

pub(super) fn candidate_dto(candidate: protocol::CandidatePresentation) -> CandidateDto {
    CandidateDto {
        id: candidate.id,
        kind: kind_dto(candidate.kind),
        is_menu: candidate.is_menu,
        file_kind: candidate.file_kind.map(|k| match k {
            protocol::FileObjectKind::File => FileObjectKindDto::File,
            protocol::FileObjectKind::Directory => FileObjectKindDto::Directory,
        }),
        title: candidate.title,
        menu_path: candidate.menu_path,
        context_keys: candidate.context_keys,
        execution_context_id: candidate.execution_context_id,
        execution_target_label: candidate.execution_target_label,
        home_eligible: candidate.home_eligible,
        primary_action: candidate.primary_action,
        section_label: candidate.section_label,
        subtitle: candidate.subtitle,
        shortcut: candidate.shortcut,
        icon_key: candidate.icon.key,
        score: candidate.score,
        group_id: candidate.group_id,
        availability: availability_dto(candidate.availability.state),
        availability_reason: candidate.availability.reason,
        command_id: candidate.command_id,
        bundle_id: candidate.bundle_id,
    }
}

pub(super) fn kind_dto(kind: protocol::CandidateKind) -> CandidateKindDto {
    match kind {
        protocol::CandidateKind::Application => CandidateKindDto::Application,
        protocol::CandidateKind::Action => CandidateKindDto::Action,
        protocol::CandidateKind::File => CandidateKindDto::File,
    }
}

pub(super) fn context_source_dto(source: protocol::ContextSource) -> ContextSourceDto {
    match source {
        protocol::ContextSource::UserInput => ContextSourceDto::UserInput,
        protocol::ContextSource::ForegroundApplication => ContextSourceDto::ForegroundApplication,
        protocol::ContextSource::ApplicationContext => ContextSourceDto::ApplicationContext,
        protocol::ContextSource::FinderSelection => ContextSourceDto::FinderSelection,
        protocol::ContextSource::SelectedText => ContextSourceDto::SelectedText,
        protocol::ContextSource::BrowserCurrentPage => ContextSourceDto::BrowserCurrentPage,
        protocol::ContextSource::Clipboard => ContextSourceDto::Clipboard,
    }
}

pub(super) fn context_capture_status_dto(
    status: protocol::ContextCaptureStatus,
) -> ContextCaptureStatusDto {
    match status {
        protocol::ContextCaptureStatus::Captured => ContextCaptureStatusDto::Captured,
        protocol::ContextCaptureStatus::Empty => ContextCaptureStatusDto::Empty,
        protocol::ContextCaptureStatus::PermissionDenied => {
            ContextCaptureStatusDto::PermissionDenied
        }
        protocol::ContextCaptureStatus::Unavailable => ContextCaptureStatusDto::Unavailable,
        protocol::ContextCaptureStatus::Unsupported => ContextCaptureStatusDto::Unsupported,
        protocol::ContextCaptureStatus::ApplicationChanged => {
            ContextCaptureStatusDto::ApplicationChanged
        }
        protocol::ContextCaptureStatus::TimedOut => ContextCaptureStatusDto::TimedOut,
        protocol::ContextCaptureStatus::TooLarge => ContextCaptureStatusDto::TooLarge,
        protocol::ContextCaptureStatus::TooManyItems => ContextCaptureStatusDto::TooManyItems,
        protocol::ContextCaptureStatus::Failed => ContextCaptureStatusDto::Failed,
    }
}

pub(super) fn availability_dto(state: protocol::AvailabilityState) -> AvailabilityDto {
    match state {
        protocol::AvailabilityState::Enabled => AvailabilityDto::Enabled,
        protocol::AvailabilityState::Disabled => AvailabilityDto::Disabled,
        protocol::AvailabilityState::Hidden => AvailabilityDto::Hidden,
    }
}

impl From<protocol::ActionResult> for ActionResultDto {
    fn from(result: protocol::ActionResult) -> Self {
        let (view, plugin_detail) = match result.view {
            Some(protocol::HostView::FileSearch) => (Some(HostViewDto::FileSearch), None),
            Some(protocol::HostView::PluginDetail { detail }) => (
                Some(HostViewDto::PluginDetail),
                Some(PluginDetailDto {
                    schema_version: detail.schema_version,
                    title: detail.title,
                    body: detail.body,
                    rows: detail
                        .rows
                        .into_iter()
                        .map(|r| PluginKeyValueDto {
                            label: r.label,
                            value: r.value,
                        })
                        .collect(),
                }),
            ),
            None => (None, None),
        };
        Self {
            session_id: result.session_id,
            revision: result.revision,
            status: match result.status {
                protocol::ActionStatus::Succeeded => ActionStatusDto::Succeeded,
                protocol::ActionStatus::Failed => ActionStatusDto::Failed,
            },
            error: result.error,
            warning: result.warning,
            output_text: result.output_text,
            view,
            plugin_detail,
            close_window: result.close_window,
            execution_session_id: result.execution_session_id,
        }
    }
}
