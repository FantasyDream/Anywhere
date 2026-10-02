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
      title: candidate.title,
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
    required String input,
    required bool contextComplete,
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
      input: input,
      contextComplete: contextComplete,
      selectedScope: selectedScope,
      scopes: scopes ??
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
          ],
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
