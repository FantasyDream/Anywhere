import 'package:flutter/material.dart';
import '../../src/rust/api.dart';
import '../../ui/app_icons.dart';
import '../../ui/app_style.dart';

IconData clipboardKindIcon(String kind) => switch (kind) {
      'image' => AppIcons.image,
      'files' => AppIcons.folder,
      'rich_text' => AppIcons.document,
      'binary' => AppIcons.file,
      _ => AppIcons.clipboard,
    };
String clipboardKindLabel(String kind) => switch (kind) {
      'image' => 'Image',
      'files' => 'Files',
      'rich_text' => 'Rich text',
      'binary' => 'Data',
      _ => 'Text',
    };

/// Displays only a bounded Rust-generated preview, never executes HTML or opens files.
class ClipboardHistoryPreview extends StatefulWidget {
  const ClipboardHistoryPreview({super.key, required this.detail});
  final ClipboardDetailDto detail;
  @override
  State<ClipboardHistoryPreview> createState() =>
      _ClipboardHistoryPreviewState();
}

class _ClipboardHistoryPreviewState extends State<ClipboardHistoryPreview> {
  MemoryImage? _image;
  @override
  void initState() {
    super.initState();
    _setImage();
  }

  void _setImage() {
    _image = widget.detail.imagePng.isEmpty
        ? null
        : MemoryImage(widget.detail.imagePng);
  }

  @override
  void didUpdateWidget(ClipboardHistoryPreview oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (!identical(oldWidget.detail.imagePng, widget.detail.imagePng)) {
      _image?.evict();
      _setImage();
    }
  }

  @override
  void dispose() {
    _image?.evict();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final detail = widget.detail;
    final formats = Text(detail.formats.join(' · '), style: AppStyle.caption);
    if (detail.files.isNotEmpty) {
      return ListView.builder(
          key: const Key('clipboard-file-preview'),
          itemCount: detail.files.length + 1,
          itemBuilder: (context, index) => index == detail.files.length
              ? Padding(padding: const EdgeInsets.only(top: 16), child: formats)
              : Padding(
                  padding: const EdgeInsets.only(bottom: 14),
                  child: Row(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        const Icon(AppIcons.file,
                            size: 18, color: AppStyle.secondary),
                        const SizedBox(width: 8),
                        Expanded(
                            child: SelectableText(detail.files[index],
                                style: AppStyle.body.copyWith(fontSize: 12))),
                      ])));
    }
    return SingleChildScrollView(
        child: Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
      if (_image != null) ...[
        Image(
            image: _image!,
            key: const Key('clipboard-image-preview'),
            fit: BoxFit.contain,
            errorBuilder: (_, error, stack) => const Text(
                'Image preview unavailable',
                style: AppStyle.caption)),
        const SizedBox(height: 16),
      ] else if (detail.item.kind == 'image') ...[
        const Icon(AppIcons.image, size: 48, color: AppStyle.secondary),
        const SizedBox(height: 12),
        const Text(
            'Preview unavailable. The original image can still be copied.',
            style: AppStyle.caption),
        const SizedBox(height: 16),
      ],
      if (detail.text.isNotEmpty) ...[
        SelectableText(detail.text,
            key: const Key('clipboard-preview'),
            style: AppStyle.body
                .copyWith(fontFamily: 'monospace', fontSize: 12, height: 1.5)),
        const SizedBox(height: 16),
      ],
      if (detail.text.isEmpty &&
          _image == null &&
          detail.item.kind != 'image') ...[
        Icon(clipboardKindIcon(detail.item.kind),
            size: 40, color: AppStyle.secondary),
        const SizedBox(height: 12),
        const Text(
            'Preview is not available for this format. Copy restores its saved data.',
            style: AppStyle.caption),
        const SizedBox(height: 16),
      ],
      formats,
    ]));
  }
}
