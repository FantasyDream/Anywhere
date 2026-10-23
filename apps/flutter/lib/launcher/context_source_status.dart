import 'dart:ui' show Locale;

import '../infrastructure/desktop_environment.dart';
import '../l10n/app_language.dart';
import '../src/rust/api.dart';

/// Presentation of safe capture metadata. No context values or system reads.
class ContextSourceStatus {
  const ContextSourceStatus(this.source, this.capture,
      {this.enabled = true, this.localizations});
  final ContextSourceDto source;
  final ContextCaptureDto? capture;
  final bool enabled;
  final AppLocalizations? localizations;
  AppLocalizations get _l10n =>
      localizations ?? lookupAppLocalizations(const Locale('en'));
  ContextSourceStatus withLocalizations(AppLocalizations labels) =>
      ContextSourceStatus(source, capture,
          enabled: enabled, localizations: labels);

  static const sources = [
    ContextSourceDto.finderSelection,
    ContextSourceDto.browserCurrentPage,
    ContextSourceDto.selectedText,
    ContextSourceDto.clipboard,
  ];

  String get label => switch (source) {
        ContextSourceDto.finderSelection =>
          DesktopEnvironment.current.usesMetaCommand
              ? _l10n.finderSelection
              : _l10n.fileSelection,
        ContextSourceDto.selectedText => _l10n.selectedText,
        ContextSourceDto.browserCurrentPage => _l10n.browserCurrentPage,
        ContextSourceDto.clipboard => _l10n.clipboard,
        _ => _l10n.context,
      };

  String get statusText {
    if (!enabled) return _l10n.captureOff;
    return switch (capture?.status) {
      null => _l10n.captureNotRead,
      ContextCaptureStatusDto.captured => _l10n.captureReady,
      ContextCaptureStatusDto.empty => switch (source) {
          ContextSourceDto.finderSelection => _l10n.noItemsSelected,
          ContextSourceDto.selectedText => _l10n.noTextSelected,
          ContextSourceDto.browserCurrentPage => _l10n.noPageAvailable,
          _ => _l10n.noTextCopied,
        },
      ContextCaptureStatusDto.permissionDenied => _l10n.permissionNeeded,
      ContextCaptureStatusDto.unsupported => _l10n.sourceUnsupported,
      ContextCaptureStatusDto.applicationChanged => _l10n.captureAppChanged,
      ContextCaptureStatusDto.timedOut => _l10n.timedOut,
      ContextCaptureStatusDto.tooLarge => _l10n.contentTooLong,
      ContextCaptureStatusDto.tooManyItems => _l10n.tooManyItems,
      ContextCaptureStatusDto.unavailable => _l10n.sourceUnavailable,
      ContextCaptureStatusDto.failed => _l10n.readFailed,
    };
  }

  String get detail => !enabled
      ? statusText
      : switch (capture?.status) {
          ContextCaptureStatusDto.permissionDenied => switch (source) {
              ContextSourceDto.finderSelection => _l10n.finderPermissionHelp,
              ContextSourceDto.selectedText => _l10n.selectedTextPermissionHelp,
              ContextSourceDto.browserCurrentPage =>
                _l10n.browserPermissionHelp,
              _ => _l10n.clipboardPermissionHelp,
            },
          ContextCaptureStatusDto.unsupported =>
            _l10n.sourceUnsupportedHelp(label),
          ContextCaptureStatusDto.applicationChanged => _l10n.appChangedHelp,
          ContextCaptureStatusDto.timedOut => _l10n.sourceTimeoutHelp(label),
          ContextCaptureStatusDto.tooLarge => _l10n.sourceTooLongHelp(label),
          ContextCaptureStatusDto.tooManyItems => _l10n.tooManyItemsHelp,
          ContextCaptureStatusDto.unavailable ||
          ContextCaptureStatusDto.failed =>
            _l10n.sourceFailedHelp(label),
          _ => statusText,
        };

  String? get settingsPane {
    if (!enabled ||
        capture?.status != ContextCaptureStatusDto.permissionDenied) {
      return null;
    }
    return switch (source) {
      ContextSourceDto.selectedText => 'accessibility',
      ContextSourceDto.finderSelection ||
      ContextSourceDto.browserCurrentPage =>
        'automation',
      ContextSourceDto.clipboard => 'clipboard',
      _ => null,
    };
  }

  /// Expected empty/unsupported states stay in Settings, keeping home quiet.
  int get attention => !enabled
      ? 0
      : switch (capture?.status) {
          ContextCaptureStatusDto.permissionDenied => 4,
          ContextCaptureStatusDto.tooLarge ||
          ContextCaptureStatusDto.tooManyItems =>
            3,
          ContextCaptureStatusDto.timedOut => 2,
          ContextCaptureStatusDto.failed ||
          ContextCaptureStatusDto.unavailable =>
            1,
          _ => 0,
        };

  static ContextSourceStatus? notice(Iterable<ContextCaptureDto> captures) {
    ContextSourceStatus? selected;
    for (final source in sources) {
      for (final capture in captures.where((item) => item.source == source)) {
        final status = ContextSourceStatus(source, capture);
        if (status.attention > (selected?.attention ?? 0)) selected = status;
      }
    }
    return selected;
  }
}
