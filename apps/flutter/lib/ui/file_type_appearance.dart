import '../l10n/app_language.dart';
import 'package:flutter/material.dart';
import 'app_icons.dart';
import 'app_style.dart';
import 'semantic_icon.dart';

/// Filename hints for presentation only. Never reads files or determines which
/// results match a search; Rust remains responsible for filtering and targets.
enum FileTypeAppearance {
  folder(AppIcons.folder, DesignTokens.iconBlue),
  pdf(AppIcons.pdf, DesignTokens.iconCoral, stamp: 'PDF'),
  document(AppIcons.document, DesignTokens.iconBlue, stamp: 'DOC'),
  text(AppIcons.text, DesignTokens.iconNeutral, stamp: 'TXT'),
  spreadsheet(AppIcons.spreadsheet, DesignTokens.iconGreen),
  presentation(AppIcons.presentation, DesignTokens.iconAmber),
  image(AppIcons.image, DesignTokens.iconBlue),
  audio(AppIcons.audio, DesignTokens.iconPink),
  video(AppIcons.video, DesignTokens.iconPurple),
  archive(AppIcons.archive, DesignTokens.iconAmber),
  code(AppIcons.code, DesignTokens.iconPurple),
  configuration(AppIcons.configuration, DesignTokens.iconAmber),
  book(AppIcons.book, DesignTokens.iconGreen),
  font(AppIcons.font, DesignTokens.iconPink),
  package(AppIcons.package, DesignTokens.iconNeutral),
  file(AppIcons.file, DesignTokens.iconNeutral);

  const FileTypeAppearance(this.glyph, this.tint, {this.stamp});
  String labelFor(AppLocalizations l10n) => switch (this) {
        FileTypeAppearance.folder => l10n.folder,
        FileTypeAppearance.pdf => l10n.pdfDocument,
        FileTypeAppearance.document => l10n.document,
        FileTypeAppearance.text => l10n.textFile,
        FileTypeAppearance.spreadsheet => l10n.spreadsheet,
        FileTypeAppearance.presentation => l10n.presentation,
        FileTypeAppearance.image => l10n.image,
        FileTypeAppearance.audio => l10n.audio,
        FileTypeAppearance.video => l10n.video,
        FileTypeAppearance.archive => l10n.archive,
        FileTypeAppearance.code => l10n.sourceCode,
        FileTypeAppearance.configuration => l10n.configurationFile,
        FileTypeAppearance.book => l10n.ebook,
        FileTypeAppearance.font => l10n.font,
        FileTypeAppearance.package => l10n.packageOrBinary,
        FileTypeAppearance.file => l10n.file,
      };

  final IconData glyph;
  final Color tint;
  final String? stamp;

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

/// Transparent semantic glyphs: no thumbnail decoding, native icon calls or
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
  Widget build(BuildContext context) => Semantics(
      image: true,
      label: appearance.labelFor(context.l10n),
      child: SemanticIcon(
          glyph: appearance.glyph,
          tint: appearance.tint,
          size: size,
          stamp: appearance.stamp,
          disabled: disabled));
}
