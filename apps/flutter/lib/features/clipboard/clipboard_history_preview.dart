import 'package:flutter/material.dart';
import '../../infrastructure/desktop_environment.dart';
import '../../src/rust/api.dart';
import '../../ui/app_style.dart';
import '../../ui/file_type_appearance.dart';
import 'clipboard_history_presentation.dart';

/// Displays only the bounded Rust preview; never reads files or executes HTML.
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
    return Padding(
        padding: const EdgeInsets.fromLTRB(16, 14, 16, 12),
        child:
            Column(crossAxisAlignment: CrossAxisAlignment.stretch, children: [
          Row(children: [
            Icon(clipboardKindIcon(detail.item.kind),
                size: 16, color: AppStyle.secondary),
            const SizedBox(width: 7),
            Expanded(
                child: Text('${clipboardKindLabel(detail.item.kind)} preview',
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    style: AppStyle.paneTitle)),
          ]),
          const SizedBox(height: 6),
          Text(clipboardCopyDate(context, detail.item.copiedAtMs.toInt()),
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
              style: AppStyle.caption),
          const SizedBox(height: 12),
          Flexible(
              fit: FlexFit.loose,
              child: Container(
                  key: const Key('clipboard-preview-surface'),
                  clipBehavior: Clip.antiAlias,
                  decoration: BoxDecoration(
                      color: AppStyle.actionSurface,
                      borderRadius:
                          BorderRadius.circular(AppStyle.controlRadius)),
                  child: _content())),
          const SizedBox(height: 10),
          Row(children: [
            Expanded(
                child: Tooltip(
                    message: 'Size of the saved clipboard data',
                    child: Text(
                        '${clipboardDataSize(detail.item.byteCount)} saved',
                        style: AppStyle.caption))),
            if (detail.formats.isNotEmpty)
              Tooltip(
                  message: detail.formats.join('\n'),
                  child: Text(
                      '${detail.formats.length} ${detail.formats.length == 1 ? 'format' : 'formats'}',
                      style: AppStyle.caption)),
          ]),
        ]));
  }

  Widget _content() {
    final detail = widget.detail;
    if (detail.files.isNotEmpty) {
      return ListView.separated(
          key: const Key('clipboard-file-preview'),
          // Short selections fit their content; large ones keep a lazy viewport.
          shrinkWrap: detail.files.length <= 3,
          padding: const EdgeInsets.all(12),
          itemCount: detail.files.length,
          separatorBuilder: (_, __) => const Divider(height: 20),
          itemBuilder: (context, index) {
            final path = detail.files[index];
            final name = clipboardFileName(path);
            return Row(crossAxisAlignment: CrossAxisAlignment.start, children: [
              FileTypeIcon(
                  appearance: FileTypeAppearance.forName(name), size: 24),
              const SizedBox(width: 8),
              Expanded(
                  child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                    Text(name,
                        maxLines: 2,
                        overflow: TextOverflow.ellipsis,
                        style: AppStyle.body.copyWith(fontSize: 12)),
                    const SizedBox(height: 4),
                    SelectableText(path, style: AppStyle.caption),
                  ])),
            ]);
          });
    }
    if (_image != null) {
      return Column(children: [
        Expanded(
            flex: 2,
            child: Padding(
                padding: const EdgeInsets.all(12),
                child: Image(
                    image: _image!,
                    key: const Key('clipboard-image-preview'),
                    fit: BoxFit.contain,
                    width: double.infinity,
                    height: double.infinity,
                    errorBuilder: (_, error, stack) =>
                        _unavailable('Image preview unavailable')))),
        if (detail.text.isNotEmpty) Expanded(child: _text(detail.text)),
      ]);
    }
    if (detail.text.isNotEmpty) return _text(detail.text);
    return _unavailable(detail.item.kind == 'image'
        ? 'Preview unavailable. The original image can still be copied.'
        : 'Preview is not available for this format. Copy restores its saved data.');
  }

  Widget _text(String text) => SingleChildScrollView(
      padding: const EdgeInsets.all(12),
      child: SizedBox(
          width: double.infinity,
          child: SelectableText(text,
              key: const Key('clipboard-preview'),
              style: AppStyle.body.copyWith(
                  fontFamily: DesktopEnvironment.current.monoFontFamily,
                  fontSize: DesignTokens.previewCodeSize,
                  height: DesignTokens.previewCodeLineHeight))));

  Widget _unavailable(String message) => Center(
      child: Padding(
          padding: const EdgeInsets.all(16),
          child: Column(mainAxisSize: MainAxisSize.min, children: [
            Icon(clipboardKindIcon(widget.detail.item.kind),
                size: 28, color: AppStyle.secondary),
            const SizedBox(height: 10),
            Text(message, style: AppStyle.caption, textAlign: TextAlign.center),
          ])));
}
