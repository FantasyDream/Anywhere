use super::{
    directory_access::DirectoryAccess,
    dto::{
        AppHistoryObjectAction, AppHistorySource, ClipboardItem, ClipboardSnapshot,
        ExecutionDetail, ExecutionRow, PreviewFile, WebDocument, WebPackage,
    },
    HostPreferences,
};
use crate::settings::{FeatureSetting, SettingsSnapshot};
use anyhow::Result;
use anywhere_core::application::frontend_runtime::{ApplicationRuntime, WebDocumentView};
use anywhere_protocol::{
    CandidatePresentation, ExecutionSessionDetail, ExecutionSessionSummary, LauncherState,
};
use std::path::PathBuf;

pub(super) fn reconcile_access_at_startup(
    directory_access: &DirectoryAccess,
    preferences: &HostPreferences,
) -> (Vec<PathBuf>, Option<String>) {
    if !preferences.file_search_enabled {
        directory_access.clear();
        return (Vec::new(), None);
    }
    if preferences.file_search_roots.is_empty() {
        return (
            Vec::new(),
            Some("File search needs at least one authorized directory.".to_owned()),
        );
    }
    match directory_access.reconcile(&preferences.file_search_roots) {
        Ok(roots) => (roots, None),
        Err(error) => (
            Vec::new(),
            Some(format!("File search is disabled: {error:#}")),
        ),
    }
}

pub(super) fn apply_preferences(
    application: &ApplicationRuntime,
    preferences: &HostPreferences,
    authorized_roots: &[PathBuf],
) -> Result<()> {
    preferences.validate()?;
    application.application().set_capture_sources(
        preferences.capture_finder_selection,
        preferences.capture_selected_text,
        preferences.capture_browser_current_page,
        preferences.capture_clipboard,
    );
    application
        .application()
        .set_excluded_apps(preferences.excluded_apps.clone());
    let enabled = preferences.file_search_enabled && !authorized_roots.is_empty();
    application.application().configure_file_search(
        enabled,
        anywhere_platform::FileSearchOptions {
            roots: authorized_roots.to_vec(),
            excluded_directory_names: preferences.file_search_excluded_names.clone(),
            engine: preferences.file_search_engine(),
            include_hidden: preferences.file_search_include_hidden,
            restrict_to_roots: true,
        },
    )?;
    Ok(())
}

pub(super) fn snapshot_cursor(state: &LauncherState) -> u64 {
    (u64::from(state.revision) << 32) | u64::from(state.snapshot_seq)
}

pub(super) fn settings_snapshot_from(application: &ApplicationRuntime) -> Result<SettingsSnapshot> {
    let settings = application.settings().map_err(anyhow::Error::new)?;
    let clipboard_recording = settings
        .clipboard()
        .status()
        .map(|status| status.recording_enabled)
        .unwrap_or(false);
    let features = settings
        .statuses()
        .into_iter()
        .filter(|status| status.first_party && status.configurable)
        .map(|status| FeatureSetting {
            key: status.id,
            name: status.name,
            enabled: status.enabled,
        })
        .collect();
    Ok(SettingsSnapshot {
        clipboard_recording,
        features,
    })
}

pub(super) fn clipboard_snapshot_from(
    application: &ApplicationRuntime,
    query: Option<&str>,
) -> Result<ClipboardSnapshot> {
    let settings = application.settings().map_err(anyhow::Error::new)?;
    let history = settings.clipboard();
    let (status, items, storage_usage) = if let Some(query) = query {
        let snapshot = history
            .snapshot(query, None)?
            .ok_or_else(|| anyhow::anyhow!("clipboard snapshot unavailable"))?;
        let items = snapshot
            .items
            .into_iter()
            .map(|item| ClipboardItem {
                id: item.id,
                summary: item.summary,
                kind: item.kind,
                copied_at_ms: item.copied_at_ms,
                byte_count: item.byte_count,
            })
            .collect();
        (snapshot.status, items, None)
    } else {
        (history.status()?, Vec::new(), Some(history.usage()?))
    };
    Ok(ClipboardSnapshot {
        recording: status.recording,
        recording_enabled: status.recording_enabled,
        available: status.available,
        paused_until_ms: status.policy.paused_until_ms,
        retention_days: status.policy.retention_days,
        storage_usage,
        notice: status.notice,
        items,
    })
}

pub(super) fn app_history_sources_from(
    application: &ApplicationRuntime,
) -> Result<Vec<AppHistorySource>> {
    let settings = application.settings().map_err(anyhow::Error::new)?;
    let service = settings.application_history()?;
    service.refresh()?;
    Ok(service
        .sources()
        .into_iter()
        .map(|status| AppHistorySource {
            id: status.descriptor.id,
            title: status.descriptor.title,
            file_hint: status.descriptor.file_hint,
            action_enabled: status.action_enabled,
            path: status.path,
            mode: status.mode,
        })
        .collect())
}

pub(super) fn app_history_object_action(
    candidate: CandidatePresentation,
) -> Option<AppHistoryObjectAction> {
    candidate
        .availability
        .is_visible()
        .then(|| AppHistoryObjectAction {
            id: candidate.id,
            title: candidate
                .localized_titles
                .get("en")
                .cloned()
                .unwrap_or(candidate.title),
            localized_titles: candidate.localized_titles,
            enabled: candidate.availability.is_executable(),
        })
}

pub(super) fn execution_row(value: ExecutionSessionSummary) -> ExecutionRow {
    ExecutionRow {
        id: value.id.to_string(),
        title: value.title,
        directory: value.directory,
        command: format_execution_command(&value.command),
        status: value.status,
        output_revision: value.output_revision,
        exit_code: value.exit_code,
        started_at_ms: value.started_at_ms,
        finished_at_ms: value.finished_at_ms,
        output_truncated: value.output_truncated,
        error: value.error,
    }
}

fn format_execution_command(arguments: &[String]) -> String {
    arguments
        .iter()
        .map(|argument| {
            if is_plain_execution_argument(argument) {
                argument.clone()
            } else {
                format!("'{}'", argument.replace('\'', "'\"'\"'"))
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn is_plain_execution_argument(argument: &str) -> bool {
    !argument.is_empty()
        && argument.chars().all(|character| {
            character.is_ascii_alphanumeric()
                || matches!(character, '_' | '.' | '/' | ':' | '=' | '@' | '+' | '-')
        })
}

pub(super) fn execution_detail(value: ExecutionSessionDetail) -> ExecutionDetail {
    ExecutionDetail {
        row: execution_row(value.summary),
        stdout: value.stdout.into(),
        stderr: value.stderr.into(),
    }
}

pub(super) fn preview(value: anywhere_core::features::preview::PreviewResource) -> PreviewFile {
    PreviewFile {
        id: value.id,
        paths: value.paths,
        title: value.title,
    }
}

pub(super) fn web_packages_from(application: &ApplicationRuntime) -> Result<Vec<WebPackage>> {
    Ok(application
        .web_package_statuses()?
        .into_iter()
        .map(|status| WebPackage {
            id: status.id,
            name: status.name,
            version: status.version,
            digest: status.digest,
            entry: status.entry,
            enabled: status.enabled,
            available: status.available,
            rollback: status.rollback,
            permission_count: status.permissions.len(),
        })
        .collect())
}

pub(super) fn web_document(document: WebDocumentView) -> WebDocument {
    WebDocument {
        handle: document.binding.runtime_id().to_owned(),
        origin: document.origin,
        entry: document.entry,
        title: document.title,
    }
}

pub(super) fn web_permission_label(
    permission: &anywhere_protocol::web_plugin::WebPermission,
) -> String {
    use anywhere_protocol::web_plugin::WebPermission;
    match permission {
        WebPermission::ContextRead { types } => format!("context.read:{}", types.join(",")),
        WebPermission::CurrentApp {} => "current-app".to_owned(),
        WebPermission::ClipboardReadText {} => "clipboard.read-text".to_owned(),
        WebPermission::ClipboardWriteText {} => "clipboard.write-text".to_owned(),
        WebPermission::LocalStorage {} => "local-storage".to_owned(),
        WebPermission::InputRead {} => "input.read".to_owned(),
        WebPermission::InputWrite {} => "input.write".to_owned(),
        WebPermission::FrontmostApp {} => "frontmost-app".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::{clipboard_snapshot_from, format_execution_command, ApplicationRuntime};
    use anywhere_core::application::frontend_runtime::RuntimePaths;

    #[test]
    fn clipboard_result_and_settings_reads_request_different_data() {
        let directory = tempfile::tempdir().unwrap();
        let application = ApplicationRuntime::open(RuntimePaths::new(
            directory.path().join("data"),
            directory.path().join("plugins"),
        ))
        .unwrap();
        let results = clipboard_snapshot_from(&application, Some("")).unwrap();
        let settings = clipboard_snapshot_from(&application, None).unwrap();
        assert_eq!(results.storage_usage, None);
        assert_eq!(settings.storage_usage, Some((0, 0)));
        assert_eq!(results.retention_days, settings.retention_days);
        assert_eq!(results.recording_enabled, settings.recording_enabled);
        assert!(settings.items.is_empty());
    }

    #[test]
    fn execution_commands_quote_arguments_like_the_flutter_view() {
        assert_eq!(
            format_execution_command(&[
                "npm".to_owned(),
                "run".to_owned(),
                "build:prod".to_owned(),
                "hello world".to_owned(),
                "it's".to_owned(),
                String::new(),
            ]),
            "npm run build:prod 'hello world' 'it'\"'\"'s' ''"
        );
        assert_eq!(format_execution_command(&[]), "");
        assert_eq!(
            format_execution_command(&[
                "/tmp/project".to_owned(),
                "@scope/pkg".to_owned(),
                "a+b-c_d=e".to_owned(),
            ]),
            "/tmp/project @scope/pkg a+b-c_d=e"
        );
    }
}
