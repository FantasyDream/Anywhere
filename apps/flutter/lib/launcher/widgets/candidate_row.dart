import '../../l10n/app_language.dart';
import 'package:flutter/material.dart';
import 'package:flutter/gestures.dart';
import '../../ui/app_components.dart';
import 'package:flutter/foundation.dart';
import '../../src/rust/api.dart';
import '../../l10n/localized_label.dart';
import '../../ui/app_style.dart';
import '../launcher_view_state.dart';
import 'candidate_icon.dart';

class CandidateRow extends StatelessWidget {
  const CandidateRow({
    super.key,
    required this.candidate,
    required this.selected,
    required this.onTap,
    this.iconBytes,
    this.sourceLabel,
    this.actionShortcut,
    this.primaryLabel,
    this.onPrimary,
    this.onActions,
    this.onDoubleTap,
    this.fileObject = true,
  });
  final CandidateDto candidate;
  final bool selected;
  final VoidCallback onTap;
  final Uint8List? iconBytes;
  final String? sourceLabel;
  final String? actionShortcut;
  final String? primaryLabel;
  final VoidCallback? onPrimary, onActions, onDoubleTap;
  final bool fileObject;

  @override
  Widget build(BuildContext context) {
    final disabled = candidate.availability != AvailabilityDto.enabled;
    final title =
        context.localizedLabel(candidate.title, candidate.localizedTitles);
    return Listener(
        // Select immediately, independently of the double-click recognizer. This
        // also lets row commands execute without waiting for a double-click timeout.
        onPointerDown: disabled
            ? null
            : (event) {
                if (event.buttons == kPrimaryMouseButton) onTap();
              },
        child: Semantics(
          selected: selected,
          button: true,
          enabled: !disabled,
          onTap: disabled ? null : onTap,
          child: Padding(
            padding: const EdgeInsets.symmetric(vertical: 2),
            child: AppSelectionSurface(
              selected: selected,
              child: SizedBox(
                height: LauncherViewState.rowHeight - 4,
                child: Padding(
                  padding: const EdgeInsets.symmetric(horizontal: 10),
                  child: Row(
                    children: [
                      Expanded(
                          // Only the object region recognizes double-clicks. A
                          // parent double-tap recognizer would delay command taps.
                          child: GestureDetector(
                              behavior: HitTestBehavior.opaque,
                              onDoubleTap: disabled ? null : onDoubleTap,
                              child: Row(children: [
                                CandidateIcon(
                                    candidate: candidate,
                                    bytes: iconBytes,
                                    size: 32,
                                    fileObject: fileObject),
                                const SizedBox(width: 12),
                                Expanded(
                                    child: _labels(context, disabled, title)),
                              ]))),
                      if (actionShortcut != null) ...[
                        const SizedBox(width: 8),
                        Tooltip(
                            message: context.l10n.anywhereActionShortcut,
                            child: _shortcut(actionShortcut!)),
                      ],
                      if (selected && !disabled && primaryLabel != null) ...[
                        const SizedBox(width: 6),
                        AppRowActions(
                            primaryLabel: primaryLabel!,
                            onPrimary: onPrimary,
                            onActions: onActions),
                      ],
                    ],
                  ),
                ),
              ),
            ),
          ),
        ));
  }

  Widget _labels(BuildContext context, bool disabled, String title) {
    final subtitle = _visibleSubtitle(context);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      mainAxisAlignment: MainAxisAlignment.center,
      children: [
        _title(disabled, title),
        if (subtitle != null) ...[
          const SizedBox(height: 2),
          Tooltip(
              message: subtitle,
              child: Text(
                subtitle,
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: TextStyle(
                  fontSize: 11.5,
                  fontWeight: FontWeight.w400,
                  shadows: AppStyle.textShadows,
                  height: 1.35,
                  color: disabled ? AppStyle.warning : AppStyle.secondary,
                ),
              )),
        ],
      ],
    );
  }

  Widget _title(bool disabled, String title) => Text(
        title,
        maxLines: 1,
        overflow: TextOverflow.ellipsis,
        style: TextStyle(
            fontSize: 15,
            height: 1.2,
            fontWeight: selected ? FontWeight.w600 : FontWeight.w500,
            shadows: AppStyle.textShadows,
            letterSpacing: 0,
            color: disabled ? AppStyle.muted : AppStyle.primary),
      );

  String? _visibleSubtitle(BuildContext context) {
    final disabled = candidate.availability != AvailabilityDto.enabled;
    if (disabled) {
      return candidate.availabilityReason ?? context.l10n.notAvailableRightNow;
    }
    final menu = candidate.menuPath.join(' › ');
    if (menu.isNotEmpty) {
      return sourceLabel == null ? menu : '$sourceLabel · $menu';
    }
    final subtitle = candidate.subtitle?.trim();
    if (subtitle == null ||
        subtitle.isEmpty ||
        _isBundleId(subtitle) ||
        candidate.title == subtitle) {
      return sourceLabel;
    }
    return sourceLabel == null ? subtitle : '$sourceLabel · $subtitle';
  }

  bool _isBundleId(String value) =>
      RegExp(r'^[A-Za-z][A-Za-z0-9-]*(\.[A-Za-z0-9-]+){2,}$').hasMatch(value);

  Widget _shortcut(String shortcut) => AppKeycap(shortcut);
}
