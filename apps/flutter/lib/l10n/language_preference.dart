/// Persisted preference, independent of widgets, the platform, and search.
abstract final class LanguagePreference {
  static const values = ['system', 'zh', 'en'];
  static String normalize(Object? value) =>
      values.contains(value) ? value as String : 'system';
}
