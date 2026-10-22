import 'package:flutter/services.dart';

/// Packaging paths belong to the host. Dart never guesses a framework layout,
/// dynamic-library suffix, or user-data location from the executable path.
class RuntimePaths {
  const RuntimePaths(this.library, this.values);
  final String library;
  final Map<String, String> values;
  String requiredPath(String key) {
    final value = values[key];
    if (value == null || value.isEmpty) {
      throw StateError('Host path "$key" is missing.');
    }
    return value;
  }

  static Future<RuntimePaths> read(MethodChannel channel) async {
    final paths = await channel.invokeMapMethod<String, String>('paths');
    final library = paths?['library'];
    if (library == null || library.isEmpty) {
      throw StateError('Host library path is missing.');
    }
    return RuntimePaths(library, Map.unmodifiable(paths!));
  }
}
