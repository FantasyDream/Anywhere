import 'package:flutter/material.dart';
import '../../ui/app_components.dart';
import 'package:flutter/foundation.dart';
import '../../src/rust/api.dart';
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
    this.fileObject = true,
  });
  final CandidateDto candidate;
  final bool selected;
  final VoidCallback onTap;
  final Uint8List? iconBytes;
  final String? sourceLabel;
  final String? actionShortcut;
  final bool fileObject;

  @override
  Widget build(BuildContext context) {
    final disabled = candidate.availability != AvailabilityDto.enabled;
    return Semantics(
      selected: selected,
      button: true,
      enabled: !disabled,
      child: Padding(
        padding: const EdgeInsets.symmetric(vertical: 2),
        child: Material(
          color: selected ? AppStyle.selectedSurface : Colors.transparent,
          shape: RoundedRectangleBorder(
            borderRadius: BorderRadius.circular(AppStyle.resultRadius),
            side: BorderSide(
                color:
                    selected ? AppStyle.selectionOutline : Colors.transparent,
                width: .5),
          ),
          child: InkWell(
            borderRadius: BorderRadius.circular(AppStyle.resultRadius),
            onTap: disabled ? null : onTap,
            child: SizedBox(
              height: LauncherViewState.rowHeight - 4,
              child: Padding(
                padding: const EdgeInsets.symmetric(horizontal: 10),
                child: Row(
                  children: [
                    CandidateIcon(
                        candidate: candidate,
                        bytes: iconBytes,
                        size: 32,
                        fileObject: fileObject),
                    const SizedBox(width: 12),
                    Expanded(child: _labels(disabled)),
                    if (actionShortcut != null ||
                        candidate.shortcut != null) ...[
                      const SizedBox(width: 8),
                      Tooltip(
                          message: actionShortcut != null
                              ? 'Anywhere action shortcut'
                              : 'Shortcut in the current app',
                          child:
                              _shortcut(actionShortcut ?? candidate.shortcut!)),
                    ],
                  ],
                ),
              ),
            ),
          ),
        ),
      ),
    );
  }

  Widget _labels(bool disabled) {
    final subtitle = _visibleSubtitle();
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      mainAxisAlignment: MainAxisAlignment.center,
      children: [
        _title(disabled),
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

  Widget _title(bool disabled) => Text(
        candidate.title,
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

  String? _visibleSubtitle() {
    final disabled = candidate.availability != AvailabilityDto.enabled;
    if (disabled) {
      return candidate.availabilityReason ?? 'Not available right now';
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
