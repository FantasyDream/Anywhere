import 'package:flutter/material.dart';
import 'app_icons.dart';
import 'app_style.dart';

/// Filename hints for presentation only. Never reads files or determines which
/// results match a search; Rust remains responsible for filtering and targets.
enum FileTypeAppearance {
  folder('Folder', AppIcons.folder, Color(0xff92c9f6)),
  pdf('PDF document', AppIcons.pdf, Color(0xfff0aaa8)),
  document('Document', AppIcons.document, Color(0xffa7caff)),
  text('Text file', AppIcons.document, Color(0xffd9e1ec)),
  spreadsheet('Spreadsheet', AppIcons.spreadsheet, Color(0xffa0ceb9)),
  presentation('Presentation', AppIcons.presentation, Color(0xffe9bd9c)),
  image('Image', AppIcons.image, Color(0xffb1cee9)),
  audio('Audio', AppIcons.audio, Color(0xffe7b4cf)),
  video('Video', AppIcons.video, Color(0xffc3b4e4)),
  archive('Archive', AppIcons.archive, Color(0xffdfc89f)),
  code('Source code', AppIcons.code, Color(0xffc3b4e4)),
  configuration('Configuration file', AppIcons.settings, Color(0xffbecbdc)),
  book('E-book', AppIcons.book, Color(0xffdfc89f)),
  font('Font', AppIcons.font, Color(0xffe7c3a6)),
  package('Package or binary', AppIcons.package, Color(0xffbecbdc)),
  file('File', AppIcons.file, Color(0xffd9e1ec));

  const FileTypeAppearance(this.label, this.glyph, this.tint);
  final String label;
  final IconData glyph;
  final Color tint;

  static FileTypeAppearance forName(String basename,
      {bool isDirectory = false}) {
    if (isDirectory) return folder;
    final name = basename.toLowerCase();
    if (_configNames.contains(name) || name.startsWith('.env.')) {
      return configuration;
    }
    if (_codeNames.contains(name)) return code;
    if (_textNames.contains(name)) return text;
    final dot = name.lastIndexOf('.');
    // A leading dot alone is a hidden basename, not a known extension.
    if (dot <= 0 || dot == name.length - 1) return file;
    return _extensions[name.substring(dot + 1)] ?? file;
  }

  static const _configNames = {
    '.env',
    '.gitignore',
    '.gitattributes',
    '.gitmodules',
    '.editorconfig',
    '.npmrc',
    '.nvmrc',
    '.prettierrc',
    '.eslintrc',
    '.bashrc',
    '.zshrc',
    '.bash_profile',
    '.zprofile',
    '.profile',
  };
  static const _codeNames = {
    'dockerfile',
    'makefile',
    'cmakelists.txt',
    'gemfile',
    'rakefile',
    'procfile',
    'justfile',
  };
  static const _textNames = {
    'readme',
    'license',
    'licence',
    'changelog',
    'authors'
  };
  static const _extensions = {
    'pdf': pdf,
    'doc': document,
    'docx': document,
    'odt': document,
    'rtf': document,
    'pages': document,
    'txt': text,
    'md': text,
    'markdown': text,
    'log': text,
    'rst': text,
    'tex': text,
    'xls': spreadsheet,
    'xlsx': spreadsheet,
    'ods': spreadsheet,
    'csv': spreadsheet,
    'tsv': spreadsheet,
    'numbers': spreadsheet,
    'ppt': presentation,
    'pptx': presentation,
    'odp': presentation,
    'key': presentation,
    'jpg': image,
    'jpeg': image,
    'png': image,
    'gif': image,
    'webp': image,
    'heic': image,
    'heif': image,
    'avif': image,
    'bmp': image,
    'tif': image,
    'tiff': image,
    'svg': image,
    'ico': image,
    'icns': image,
    'psd': image,
    'raw': image,
    'mp3': audio,
    'm4a': audio,
    'aac': audio,
    'wav': audio,
    'flac': audio,
    'ogg': audio,
    'opus': audio,
    'aiff': audio,
    'aif': audio,
    'wma': audio,
    'mp4': video,
    'mov': video,
    'mkv': video,
    'avi': video,
    'webm': video,
    'm4v': video,
    'wmv': video,
    'mpeg': video,
    'mpg': video,
    'zip': archive,
    '7z': archive,
    'rar': archive,
    'tar': archive,
    'gz': archive,
    'bz2': archive,
    'xz': archive,
    'tgz': archive,
    'zst': archive,
    'lz': archive,
    'lzma': archive,
    'tbz2': archive,
    'dmg': package,
    'iso': package,
    'pkg': package,
    'exe': package,
    'dll': package,
    'dylib': package,
    'so': package,
    'epub': book,
    'mobi': book,
    'azw': book,
    'azw3': book,
    'ttf': font,
    'otf': font,
    'woff': font,
    'woff2': font,
    'json': configuration,
    'yaml': configuration,
    'yml': configuration,
    'toml': configuration,
    'xml': configuration,
    'ini': configuration,
    'conf': configuration,
    'cfg': configuration,
    'properties': configuration,
    'lock': configuration,
    'dart': code,
    'swift': code,
    'rs': code,
    'java': code,
    'kt': code,
    'kts': code,
    'js': code,
    'jsx': code,
    'ts': code,
    'tsx': code,
    'py': code,
    'go': code,
    'c': code,
    'h': code,
    'cpp': code,
    'hpp': code,
    'cc': code,
    'cxx': code,
    'hxx': code,
    'cs': code,
    'sh': code,
    'bash': code,
    'zsh': code,
    'ps1': code,
    'bat': code,
    'cmd': code,
    'fish': code,
    'rb': code,
    'php': code,
    'lua': code,
    'sql': code,
    'html': code,
    'htm': code,
    'css': code,
    'scss': code,
    'sass': code,
    'less': code,
    'vue': code,
    'svelte': code,
    'ipynb': code,
    'ex': code,
    'exs': code,
    'erl': code,
    'clj': code,
    'cljs': code,
    'pl': code,
    'pm': code,
    'scala': code,
    'sc': code,
    'groovy': code,
    'r': code,
    'm': code,
    'mm': code,
    'cmake': code,
    'gradle': code,
    'hs': code,
  };
}

/// Transparent outline glyphs: no thumbnail decoding, native icon calls or
/// per-file raster cache. Rows and previews share the same presentation.
class FileTypeIcon extends StatelessWidget {
  const FileTypeIcon(
      {super.key,
      required this.appearance,
      this.size = 32,
      this.disabled = false});
  final FileTypeAppearance appearance;
  final double size;
  final bool disabled;

  @override
  Widget build(BuildContext context) => SizedBox.square(
      dimension: size,
      child: Icon(appearance.glyph,
          size: size * .66,
          color: disabled ? AppStyle.muted : appearance.tint,
          semanticLabel: appearance.label));
}
