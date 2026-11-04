import 'dart:ui' show Locale;

import 'package:anywhere/l10n/app_language.dart';
import 'package:anywhere/launcher/launcher_presentation.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  final labels = lookupAppLocalizations(const Locale('en'));

  test('file range presentation localizes empty and multiple roots', () {
    expect(FileRangePresentation(const []).label(labels), 'File Search');
    expect(FileRangePresentation(['/Users/me', '/Volumes/work']).label(labels),
        'File Search · 2 locations');
  });

  test('file range presentation uses the final non-empty path component', () {
    final presentation = FileRangePresentation(['/Users/me/Workspace/']);

    expect(presentation.label(labels), 'File Search · Workspace');
    expect(presentation.detail, '/Users/me/Workspace/');
  });
}
