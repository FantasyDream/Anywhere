import 'package:flutter/widgets.dart';

import 'app_language.dart';

/// Source-provided names stay separate from bundled UI translations and stable IDs.
extension LocalizedLabel on BuildContext {
  String localizedLabel(String original, Map<String, String> labels) {
    final language = l10n.localeName.split(RegExp('[-_]')).first;
    final label = labels[language];
    return label == null || label.trim().isEmpty ? original : label;
  }
}
