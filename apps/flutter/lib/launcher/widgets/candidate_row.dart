import 'package:flutter/material.dart';
import 'package:flutter/foundation.dart';
import '../../src/rust/api.dart';
import '../launcher_style.dart';

class CandidateRow extends StatelessWidget {
  const CandidateRow({
    super.key,
    required this.candidate,
    required this.selected,
    required this.onTap,
    this.iconBytes,
    this.sourceLabel,
  });
  final CandidateDto candidate;
  final bool selected;
  final VoidCallback onTap;
  final Uint8List? iconBytes;
  final String? sourceLabel;

  @override
  Widget build(BuildContext context) {
    final disabled = candidate.availability != AvailabilityDto.enabled;
    return Semantics(
      selected: selected,
      button: true,
      enabled: !disabled,
      child: Padding(
        padding: const EdgeInsets.symmetric(horizontal: 2, vertical: 1),
        child: Material(
          color: selected ? const Color(0x44778fe0) : Colors.transparent,
          shape: RoundedRectangleBorder(
            borderRadius: BorderRadius.circular(11),
            side: BorderSide(
              color: selected ? const Color(0x779eb4ff) : Colors.transparent,
              width: 0.8,
            ),
          ),
          child: InkWell(
            borderRadius: BorderRadius.circular(11),
            onTap: disabled ? null : onTap,
            child: SizedBox(
              height: 50,
              child: Padding(
                padding: const EdgeInsets.symmetric(horizontal: 8),
                child: Row(
                  children: [
                    if (selected)
                      Container(
                        width: 3,
                        height: 22,
                        margin: const EdgeInsets.only(right: 8),
                        decoration: BoxDecoration(
                          color: LauncherStyle.accent,
                          borderRadius: BorderRadius.circular(99),
                        ),
                      )
                    else
                      const SizedBox(width: 11),
                    _iconPlate(disabled),
                    const SizedBox(width: 12),
                    Expanded(child: _labels(disabled)),
                    if (candidate.shortcut != null) ...[
                      const SizedBox(width: 8),
                      _shortcut(candidate.shortcut!),
                    ],
                    if (selected && !disabled) ...[
                      const SizedBox(width: 8),
                      const LauncherKeycap('↵'),
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

  Widget _iconPlate(bool disabled) {
    final bytes = iconBytes;
    return Container(
      width: 28,
      height: 28,
      decoration: BoxDecoration(
        gradient: bytes != null
            ? null
            : const LinearGradient(
                begin: Alignment.topLeft,
                end: Alignment.bottomRight,
                colors: [Color(0x59444a5c), Color(0x33282c38)],
              ),
        color: bytes != null ? Colors.transparent : null,
        borderRadius: BorderRadius.circular(8),
        border: bytes != null
            ? null
            : Border.all(color: const Color(0x22ffffff), width: 0.5),
      ),
      clipBehavior: Clip.antiAlias,
      child: bytes != null
          ? Image.memory(
              bytes,
              width: 28,
              height: 28,
              fit: BoxFit.cover,
              gaplessPlayback: true,
              errorBuilder: (_, __, ___) => _glyph(disabled),
            )
          : _glyph(disabled),
    );
  }

  Widget _labels(bool disabled) {
    final subtitle = _visibleSubtitle();
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      mainAxisAlignment: MainAxisAlignment.center,
      children: [
        _title(disabled),
        if (subtitle != null)
          Text(
            subtitle,
            maxLines: 1,
            overflow: TextOverflow.ellipsis,
            style: TextStyle(
              fontSize: 11,
              height: 1.35,
              color:
                  disabled ? const Color(0xffc9a26b) : const Color(0x99c5cddd),
            ),
          ),
      ],
    );
  }

  Widget _title(bool disabled) => Text(
        candidate.title,
        maxLines: 1,
        overflow: TextOverflow.ellipsis,
        style: TextStyle(
            fontSize: 13.5,
            height: 1.2,
            fontWeight: FontWeight.w600,
            letterSpacing: -0.2,
            color: disabled ? const Color(0xff8b90a0) : LauncherStyle.primary),
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

  Widget _shortcut(String shortcut) => Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          for (final part in _shortcutParts(shortcut)) ...[
            const SizedBox(width: 3),
            LauncherKeycap(part),
          ],
        ],
      );

  List<String> _shortcutParts(String shortcut) {
    if (shortcut.startsWith('^') && shortcut.length > 1) {
      return ['⌃', shortcut.substring(1)];
    }
    final parts = <String>[];
    final rest = StringBuffer();
    for (final rune in shortcut.runes) {
      final unit = String.fromCharCode(rune);
      if ('⌃⌘⇧⌥'.contains(unit)) {
        if (rest.isNotEmpty) {
          parts.add(rest.toString());
          rest.clear();
        }
        parts.add(unit);
      } else {
        rest.write(unit);
      }
    }
    if (rest.isNotEmpty) parts.add(rest.toString());
    return parts.isEmpty ? [shortcut] : parts;
  }

  Widget _glyph(bool disabled) => Icon(
        _icon(candidate),
        size: 15,
        color: disabled ? const Color(0xff7c8293) : const Color(0xffd5dced),
      );

  IconData _icon(CandidateDto candidate) {
    if (candidate.kind == CandidateKindDto.application) {
      return Icons.apps_rounded;
    }
    switch (candidate.iconKey) {
      case 'terminal':
        return Icons.terminal_rounded;
      case 'folder':
        return Icons.folder_open_rounded;
      case 'file':
        return Icons.insert_drive_file_outlined;
      case 'copy':
        return Icons.content_copy_rounded;
      case 'globe':
        return Icons.language_rounded;
      case 'code':
        return Icons.data_object_rounded;
      case 'search':
        return Icons.search_rounded;
      case 'git':
        return Icons.account_tree_outlined;
    }
    if (candidate.iconKey == 'app') return Icons.north_east_rounded;
    return Icons.bolt_rounded;
  }
}

class LauncherKeycap extends StatelessWidget {
  const LauncherKeycap(this.label, {super.key});
  final String label;

  @override
  Widget build(BuildContext context) => Container(
        constraints: const BoxConstraints(minWidth: 18),
        padding: const EdgeInsets.symmetric(horizontal: 5, vertical: 3),
        decoration: BoxDecoration(
          color: const Color(0x33222630),
          borderRadius: BorderRadius.circular(5),
          border: Border.all(color: const Color(0x33ffffff), width: 0.5),
        ),
        child: Text(
          label,
          textAlign: TextAlign.center,
          style: const TextStyle(
            fontSize: 10,
            height: 1.1,
            fontWeight: FontWeight.w500,
            color: Color(0xffcdd3e0),
          ),
        ),
      );
}
