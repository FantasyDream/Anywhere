import 'package:flutter/material.dart';
import 'package:flutter/foundation.dart';
import '../../src/rust/api.dart';

/// A result row: presentation only, with selection and icon data supplied by its parent.
class CandidateRow extends StatelessWidget {
  const CandidateRow({
    super.key,
    required this.candidate,
    required this.selected,
    required this.onTap,
    this.iconBytes,
  });
  final CandidateDto candidate;
  final bool selected;
  final VoidCallback onTap;
  final Uint8List? iconBytes;

  @override
  Widget build(BuildContext context) {
    final disabled = candidate.availability != 'enabled';
    return Semantics(
      selected: selected,
      button: true,
      child: Material(
        color: selected ? const Color(0x665c6070) : Colors.transparent,
        borderRadius: BorderRadius.circular(7),
        child: InkWell(
          borderRadius: BorderRadius.circular(7),
          onTap: onTap,
          child: Padding(
            padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 6),
            child: Row(
              children: [
                Container(
                  width: 30,
                  height: 30,
                  decoration: BoxDecoration(
                    // Only the fallback glyph needs a plate: a real icon is opaque.
                    color: iconBytes != null
                        ? Colors.transparent
                        : const Color(0x4d454955),
                    borderRadius: BorderRadius.circular(7),
                  ),
                  clipBehavior: Clip.antiAlias,
                  child: _iconFor(candidate, disabled),
                ),
                const SizedBox(width: 12),
                Expanded(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    mainAxisAlignment: MainAxisAlignment.center,
                    children: [
                      Text(
                        candidate.title,
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                        style: TextStyle(
                          fontSize: 13,
                          fontWeight: FontWeight.w500,
                          color: disabled
                              ? const Color(0xff9aa0ad)
                              : const Color(0xffebebef),
                        ),
                      ),
                      if (candidate.subtitle != null || disabled)
                        Text(
                          disabled
                              ? (candidate.availabilityReason ??
                                  'Not available right now')
                              : candidate.subtitle!,
                          maxLines: 1,
                          overflow: TextOverflow.ellipsis,
                          style: TextStyle(
                            fontSize: 11,
                            height: 1.5,
                            color: disabled
                                ? const Color(0xffc9a26b)
                                : const Color(0xffd0d6e1),
                          ),
                        ),
                    ],
                  ),
                ),
                if (candidate.shortcut != null) ...[
                  const SizedBox(width: 12),
                  Text(candidate.shortcut!,
                      style: const TextStyle(
                          fontSize: 12, color: Color(0xffa8abb7))),
                ],
                if (selected && !disabled) ...[
                  const SizedBox(width: 12),
                  const Icon(
                    Icons.keyboard_return_rounded,
                    size: 16,
                    color: Color(0xffa8abb7),
                  ),
                ],
              ],
            ),
          ),
        ),
      ),
    );
  }

  /// Real application icon when available, semantic glyph otherwise.
  Widget _iconFor(CandidateDto candidate, bool disabled) {
    final bytes = iconBytes;
    if (bytes != null) {
      // `cover` fills the slot the way the system's own icon renderer does. `contain`
      // letterboxes, which showed up as a transparent frame around the artwork.
      return Image.memory(
        bytes,
        width: 30,
        height: 30,
        fit: BoxFit.cover,
        gaplessPlayback: true,
        errorBuilder: (_, __, ___) => _glyph(candidate, disabled),
      );
    }
    return _glyph(candidate, disabled);
  }

  Widget _glyph(CandidateDto candidate, bool disabled) => Icon(
        _icon(candidate),
        size: 17,
        color: disabled ? const Color(0xff8a8f9c) : const Color(0xffcad0de),
      );

  IconData _icon(CandidateDto candidate) {
    if (candidate.kind == 'application') return Icons.apps_rounded;
    return switch (candidate.iconKey) {
      'terminal' => Icons.terminal_rounded,
      'folder' => Icons.folder_open_rounded,
      'file' => Icons.insert_drive_file_outlined,
      'copy' => Icons.content_copy_rounded,
      'globe' => Icons.language_rounded,
      'code' => Icons.data_object_rounded,
      'search' => Icons.search_rounded,
      'git' => Icons.account_tree_outlined,
      _ => Icons.bolt_rounded,
    };
  }
}
