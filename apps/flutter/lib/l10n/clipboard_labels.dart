import '../src/rust/api.dart';
import 'app_language.dart';

/// Shared by history, the floating shelf and settings. Rust supplies policy
/// states, never presentation wording or a language-dependent identity.
extension ClipboardNoticeLabel on ClipboardNoticeDto {
  String labelFor(AppLocalizations l10n) => switch (this) {
        ClipboardNoticeDto.featureDisabled => l10n.historyFeatureDisabled,
        ClipboardNoticeDto.storageUnavailable => l10n.historyStorageUnavailable,
        ClipboardNoticeDto.accessRequired => l10n.historyAccessRequired,
        ClipboardNoticeDto.itemTooLarge => l10n.historyItemTooLarge,
        ClipboardNoticeDto.readFailed => l10n.historyReadFailed,
        ClipboardNoticeDto.captureUnavailable => l10n.historyCaptureUnavailable,
        ClipboardNoticeDto.monitoringUnavailable =>
          l10n.historyMonitoringUnavailable,
      };
}
