import '../infrastructure/desktop_environment.dart';
import '../src/rust/api.dart';

/// Presentation of safe capture metadata. No context values or system reads.
class ContextSourceStatus {
  const ContextSourceStatus(this.source, this.capture, {this.enabled = true});
  final ContextSourceDto source;
  final ContextCaptureDto? capture;
  final bool enabled;

  static const sources = [
    ContextSourceDto.finderSelection,
    ContextSourceDto.browserCurrentPage,
    ContextSourceDto.selectedText,
    ContextSourceDto.clipboard,
  ];

  String get label => switch (source) {
        ContextSourceDto.finderSelection =>
          DesktopEnvironment.current.fileSelectionLabel,
        ContextSourceDto.selectedText => 'Selected text',
        ContextSourceDto.browserCurrentPage => 'Browser current page',
        ContextSourceDto.clipboard => 'Clipboard',
        _ => 'Context',
      };

  String get statusText {
    if (!enabled) return 'Off · applies on the next wake';
    return switch (capture?.status) {
      null => 'Not captured in the last wake',
      ContextCaptureStatusDto.captured => 'Ready · last wake',
      ContextCaptureStatusDto.empty => switch (source) {
          ContextSourceDto.finderSelection => 'No items selected',
          ContextSourceDto.selectedText => 'No text selected',
          ContextSourceDto.browserCurrentPage => 'No web page available',
          _ => 'No text copied',
        },
      ContextCaptureStatusDto.permissionDenied => 'Permission needed',
      ContextCaptureStatusDto.unsupported => 'Not supported by this app',
      ContextCaptureStatusDto.applicationChanged =>
        'App changed during capture',
      ContextCaptureStatusDto.timedOut => 'Timed out',
      ContextCaptureStatusDto.tooLarge => 'Content too long',
      ContextCaptureStatusDto.tooManyItems => 'Too many selected items',
      ContextCaptureStatusDto.unavailable => 'Could not read this source',
      ContextCaptureStatusDto.failed => 'Read failed',
    };
  }

  String get detail => !enabled
      ? statusText
      : switch (capture?.status) {
          ContextCaptureStatusDto.permissionDenied => switch (source) {
              ContextSourceDto.finderSelection =>
                'Allow Finder Automation in System Settings, then reopen Anywhere.',
              ContextSourceDto.selectedText =>
                'Allow Accessibility in System Settings, then reopen Anywhere.',
              ContextSourceDto.browserCurrentPage =>
                'Allow Anywhere to access this browser in System Settings → Privacy & Security → Automation, then reopen Anywhere.',
              _ =>
                'Allow clipboard access in System Settings, then reopen Anywhere.',
            },
          ContextCaptureStatusDto.unsupported =>
            '$label is not exposed by this app. Other sources still work.',
          ContextCaptureStatusDto.applicationChanged =>
            'The original app exited or restarted. Reopen Anywhere to capture it again.',
          ContextCaptureStatusDto.timedOut =>
            '$label took too long to respond. Reopen Anywhere to try again.',
          ContextCaptureStatusDto.tooLarge =>
            '$label is too long to use. Use less content and reopen Anywhere.',
          ContextCaptureStatusDto.tooManyItems =>
            'Select up to 32 items in Finder, then reopen Anywhere.',
          ContextCaptureStatusDto.unavailable ||
          ContextCaptureStatusDto.failed =>
            '$label could not be read. Reopen Anywhere to try again; other sources still work.',
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

  static ContextSourceStatus? notice(List<ContextCaptureDto> captures) {
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
