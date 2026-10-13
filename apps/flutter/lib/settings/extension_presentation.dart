import '../src/rust/api.dart';

enum ExtensionPhase { enabled, disabled, needsPermission, failed, unavailable }

extension ExtensionPresentation on ExtensionStatusDto {
  ExtensionPhase get phase {
    if (active) return ExtensionPhase.enabled;
    if (!configurable) return ExtensionPhase.unavailable;
    if (!enabled) return ExtensionPhase.disabled;
    if (needsApproval) return ExtensionPhase.needsPermission;
    return ExtensionPhase.failed;
  }

  String get stateLabel => switch (phase) {
        ExtensionPhase.enabled => 'Enabled',
        ExtensionPhase.disabled => 'Disabled',
        ExtensionPhase.needsPermission => 'Awaiting permission',
        ExtensionPhase.failed => 'Could not start',
        ExtensionPhase.unavailable => 'Unavailable',
      };
  bool get needsAttention =>
      phase == ExtensionPhase.failed || phase == ExtensionPhase.unavailable;
}

// Only for callers still using the v1 plugin-status facade.
List<ExtensionStatusDto> legacyExtensionStatuses(
        List<PluginStatusDto> plugins) =>
    [
      for (final p in plugins)
        ExtensionStatusDto(
            key: 'plugin:${p.id}',
            id: p.id,
            name: p.id,
            version: 'v1',
            firstParty: false,
            legacy: true,
            enabled: p.ok,
            active: p.ok,
            needsApproval: false,
            configurable: false,
            capabilities: const [],
            approvalToken: '',
            detail: p.detail),
    ];
