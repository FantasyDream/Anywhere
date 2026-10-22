import 'package:anywhere/src/rust/api.dart';

ContextDto contextFixture(
        {required String id,
        required String label,
        required List<String> candidateIds,
        ContextSourceDto source = ContextSourceDto.applicationContext,
        String typeId = 'core.directory',
        String detail = '/project/build',
        List<String> capabilities = const []}) =>
    ContextDto(
      id: id,
      key: id,
      rootKey: id,
      label: label,
      detail: detail,
      typeId: typeId,
      source: source,
      iconKey: typeId == 'core.application' ? 'application' : 'folder',
      capabilities: capabilities,
      candidateIds: candidateIds,
    );

CandidateDto withRoots(CandidateDto candidate, List<ContextDto> roots) =>
    CandidateDto(
      id: candidate.id,
      kind: candidate.kind,
      isMenu: candidate.isMenu,
      fileKind: candidate.fileKind,
      title: candidate.title,
      localizedTitles: candidate.localizedTitles,
      menuPath: candidate.menuPath,
      contextKeys: {
        ...candidate.contextKeys,
        for (final root in roots)
          if (root.candidateIds.contains(candidate.id)) root.key
      }.toList(),
      executionContextId: candidate.executionContextId,
      executionTargetLabel: candidate.executionTargetLabel,
      homeEligible: candidate.homeEligible,
      primaryAction: candidate.primaryAction,
      sectionLabel: candidate.sectionLabel,
      subtitle: candidate.subtitle,
      shortcut: candidate.shortcut,
      iconKey: candidate.iconKey,
      score: candidate.score,
      groupId: candidate.groupId,
      availability: candidate.availability,
      availabilityReason: candidate.availabilityReason,
      commandId: candidate.commandId,
      bundleId: candidate.bundleId,
    );

LauncherStateDto launcherSnapshot(
    {required int sessionId,
    required int revision,
    required int snapshotSeq,
    int candidateListVersion = 0,
    required String input,
    required bool contextComplete,
    List<SearchSourceDto> searchSources = const [],
    FileSearchProgressDto? fileSearch,
    List<CandidateDto> resultActions = const [],
    List<ContextDto> contexts = const [],
    List<ContextDto> roots = const [],
    required List<CandidateDto> candidates,
    List<CandidateDto>? contextCandidates,
    List<ContextCaptureDto> contextCaptures = const [],
    required List<String> diagnostics,
    String? preferredContextKey,
    List<ScopeDto>? scopes,
    String? selectedScope}) {
  final allRoots = [...roots, ...contexts];
  final offers = contextCandidates ?? candidates;
  return LauncherStateDto(
      sessionId: sessionId,
      revision: revision,
      snapshotSeq: snapshotSeq,
      candidateListVersion: candidateListVersion,
      input: input,
      contextComplete: contextComplete,
      searchSources: searchSources,
      fileSearch: fileSearch,
      resultActions: resultActions,
      selectedScope: selectedScope,
      scopes: (scopes ??
              [
                for (final group in {
                  for (final offer in offers)
                    if (offer.kind == CandidateKindDto.action &&
                        offer.availability != AvailabilityDto.hidden)
                      offer.groupId
                })
                  ScopeDto(
                      id: group,
                      label: group == 'folder'
                          ? 'Folder'
                          : group == 'git'
                              ? 'Git'
                              : group.startsWith('app.')
                                  ? 'Editor'
                                  : group,
                      isCurrentApp: group.startsWith('app.')),
              ])
          .map((scope) => ScopeDto(
                  id: scope.id,
                  label: scope.label,
                  isCurrentApp: scope.isCurrentApp,
                  actionIds: [
                    for (final c in offers)
                      if (c.kind == CandidateKindDto.action &&
                          c.availability != AvailabilityDto.hidden &&
                          (scope.isCurrentApp || c.groupId == scope.id))
                        c.id
                  ],
                  menuIds: [
                    for (final c in offers)
                      if (c.isMenu &&
                          c.groupId == scope.id &&
                          c.availability != AvailabilityDto.hidden)
                        c.id
                  ]))
          .toList(),
      contexts: allRoots,
      preferredContextKey:
          preferredContextKey ?? (allRoots.isEmpty ? null : allRoots.first.key),
      contextCaptures: contextCaptures,
      candidates: [
        for (final candidate in candidates) withRoots(candidate, allRoots)
      ],
      contextCandidates: [
        for (final candidate in offers) withRoots(candidate, allRoots)
      ],
      diagnostics: diagnostics);
}

// Test Host supplies an explicit fake Rust recommendation. Eligibility/expiry
// boundary tests live in Rust input_policy; this is only for Widget flow tests.
ActivationInputDto? fixtureInputSuggestion(
    List<ContextCaptureDto> captures, int nowMs,
    {required bool selectedEnabled,
    required bool clipboardEnabled,
    required bool allowPendingSelection}) {
  final selected = captures
      .where((c) => c.source == ContextSourceDto.selectedText)
      .firstOrNull;
  if (selectedEnabled && selected == null && !allowPendingSelection) {
    return null;
  }
  final value =
      (selectedEnabled && selected?.status == ContextCaptureStatusDto.captured)
          ? selected
          : (clipboardEnabled
              ? captures
                  .where((c) =>
                      c.source == ContextSourceDto.clipboard &&
                      c.status == ContextCaptureStatusDto.captured)
                  .firstOrNull
              : null);
  return value?.text == null
      ? null
      : ActivationInputDto(source: value!.source, text: value.text!);
}
