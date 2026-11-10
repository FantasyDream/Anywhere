use anywhere_protocol::{
    canonical_context_type, is_json_context, ActionId, ActionResult, ActionStatus, Availability,
    AvailabilityState, CandidateId, CandidateKind, CandidatePresentation,
    ContextCapturePresentation, ContextCaptureStatus, ContextDescriptor, ContextId, ContextKey,
    ContextMatcher, ContextPresentation, ContextSource, ContextTypeId, DeclarativeAction,
    ExecutionSessionDetail, ExecutionSessionId, ExecutionSessionStatus, ExecutionSessionSummary,
    FileObjectKind, FileSearchFilter, FileSearchProgress, GroupId, HostView, IconDescriptor,
    InputMode, LauncherEvent, LauncherState, PluginCommand, PluginManifest, ProviderId, Revision,
    Scope, ScopeEntry, SearchSourcePresentation, SearchSourceState, SessionId, APPLICATION,
    DIRECTORY, FILE, FILE_SELECTION, GIT_REPOSITORY, JAVA_SPRING_BOOT, JSON_DOCUMENT, JSON_LEGACY,
    LAUNCHER_ENTRY_CONTEXT, PROJECT_GRADLE, PROJECT_MAVEN, PROJECT_NODE, PROJECT_RUST, TEXT, URL,
};

#[test]
fn root_paths_keep_the_existing_protocol_surface() {
    let sizes = [
        std::mem::size_of::<ActionId>(),
        std::mem::size_of::<ActionResult>(),
        std::mem::size_of::<ActionStatus>(),
        std::mem::size_of::<Availability>(),
        std::mem::size_of::<AvailabilityState>(),
        std::mem::size_of::<CandidateId>(),
        std::mem::size_of::<CandidateKind>(),
        std::mem::size_of::<CandidatePresentation>(),
        std::mem::size_of::<ContextCapturePresentation>(),
        std::mem::size_of::<ContextCaptureStatus>(),
        std::mem::size_of::<ContextDescriptor>(),
        std::mem::size_of::<ContextId>(),
        std::mem::size_of::<ContextKey>(),
        std::mem::size_of::<ContextMatcher>(),
        std::mem::size_of::<ContextPresentation>(),
        std::mem::size_of::<ContextSource>(),
        std::mem::size_of::<ContextTypeId>(),
        std::mem::size_of::<DeclarativeAction>(),
        std::mem::size_of::<ExecutionSessionDetail>(),
        std::mem::size_of::<ExecutionSessionId>(),
        std::mem::size_of::<ExecutionSessionStatus>(),
        std::mem::size_of::<ExecutionSessionSummary>(),
        std::mem::size_of::<FileObjectKind>(),
        std::mem::size_of::<FileSearchFilter>(),
        std::mem::size_of::<FileSearchProgress>(),
        std::mem::size_of::<GroupId>(),
        std::mem::size_of::<HostView>(),
        std::mem::size_of::<IconDescriptor>(),
        std::mem::size_of::<InputMode>(),
        std::mem::size_of::<LauncherEvent>(),
        std::mem::size_of::<LauncherState>(),
        std::mem::size_of::<PluginCommand>(),
        std::mem::size_of::<PluginManifest>(),
        std::mem::size_of::<ProviderId>(),
        std::mem::size_of::<Revision>(),
        std::mem::size_of::<Scope>(),
        std::mem::size_of::<ScopeEntry>(),
        std::mem::size_of::<SearchSourcePresentation>(),
        std::mem::size_of::<SearchSourceState>(),
        std::mem::size_of::<SessionId>(),
    ];
    assert!(sizes.iter().all(|size| *size > 0));

    assert_eq!(LAUNCHER_ENTRY_CONTEXT, "core.launcher");
    assert_eq!(TEXT, "core.text");
    assert_eq!(URL, "core.url");
    assert_eq!(JSON_DOCUMENT, "json.document");
    assert_eq!(JSON_LEGACY, "core.json");
    assert_eq!(FILE, "core.file");
    assert_eq!(FILE_SELECTION, "core.file_selection");
    assert_eq!(DIRECTORY, "core.directory");
    assert_eq!(APPLICATION, "core.application");
    assert_eq!(GIT_REPOSITORY, "git.repository");
    assert_eq!(PROJECT_RUST, "project.rust");
    assert_eq!(PROJECT_MAVEN, "project.maven");
    assert_eq!(PROJECT_GRADLE, "project.gradle");
    assert_eq!(JAVA_SPRING_BOOT, "java.spring_boot");
    assert_eq!(PROJECT_NODE, "project.node");
    assert!(is_json_context(JSON_LEGACY));
    assert_eq!(canonical_context_type(JSON_LEGACY), JSON_DOCUMENT);
}
