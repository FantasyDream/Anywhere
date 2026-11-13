import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';

enum AppearanceBrightness { light, dark }

enum AppearanceContrast { standard, increased }

enum WindowSurfaceMode { opaque, systemManaged }

/// Static capabilities reported by the native appearance adapter.
@immutable
final class DesktopAppearanceCapabilities {
  const DesktopAppearanceCapabilities({
    this.protocolVersion = 0,
    this.systemSurface = false,
    this.brightness = false,
    this.contrast = false,
    this.reduceTransparency = false,
    this.reduceMotion = false,
    this.systemColors = false,
  });

  final int protocolVersion;
  final bool systemSurface;
  final bool brightness;
  final bool contrast;
  final bool reduceTransparency;
  final bool reduceMotion;
  final bool systemColors;

  static DesktopAppearanceCapabilities fromMap(Object? raw) {
    final map = _stringMap(raw);
    return DesktopAppearanceCapabilities(
      protocolVersion: _nonNegativeInt(map['protocolVersion']) ?? 0,
      systemSurface: map['systemSurface'] == true,
      brightness: map['brightness'] == true,
      contrast: map['contrast'] == true,
      reduceTransparency: map['reduceTransparency'] == true,
      reduceMotion: map['reduceMotion'] == true,
      systemColors: map['systemColors'] == true,
    );
  }
}

/// One complete appearance version for one native window binding.
///
/// A source ID identifies one native adapter lifetime. Revisions are comparable
/// only while both the source and the opaque window token stay the same.
@immutable
final class DesktopAppearanceSnapshot {
  const DesktopAppearanceSnapshot({
    required this.sourceId,
    required this.windowToken,
    required this.revision,
    required this.brightness,
    required this.contrast,
    required this.reduceTransparency,
    required this.reduceMotion,
    required this.surface,
    required this.capabilities,
    this.accentColor,
    this.systemColors = const {},
    this.fallbackReason,
  });

  final String sourceId;
  final String windowToken;
  final int revision;
  final AppearanceBrightness brightness;
  final AppearanceContrast contrast;
  final bool reduceTransparency;
  final bool reduceMotion;
  final WindowSurfaceMode surface;
  final DesktopAppearanceCapabilities capabilities;
  final int? accentColor;
  final Map<String, int> systemColors;
  final String? fallbackReason;

  bool get increasedContrast => contrast == AppearanceContrast.increased;

  bool get usesSystemSurface => surface == WindowSurfaceMode.systemManaged;

  bool sameVersion(DesktopAppearanceSnapshot other) =>
      sourceId == other.sourceId &&
      windowToken == other.windowToken &&
      revision == other.revision;

  static DesktopAppearanceSnapshot fallback({
    TargetPlatform? platform,
  }) {
    final effectivePlatform = platform ?? defaultTargetPlatform;
    return DesktopAppearanceSnapshot(
      sourceId: 'local-fallback',
      windowToken: '',
      revision: 0,
      brightness: AppearanceBrightness.dark,
      contrast: AppearanceContrast.standard,
      reduceTransparency: false,
      reduceMotion: false,
      surface: WindowSurfaceMode.opaque,
      capabilities: const DesktopAppearanceCapabilities(),
      fallbackReason: effectivePlatform == TargetPlatform.macOS
          ? 'appearanceHostUnavailable'
          : 'platformThemeHostUnavailable',
    );
  }

  static DesktopAppearanceSnapshot? tryParse(Object? raw) {
    final map = _stringMap(raw);
    final sourceId = _boundedString(map['sourceId']);
    final windowToken = _boundedString(map['windowToken']);
    final revision = _nonNegativeInt(map['revision']);
    final brightness = switch (map['brightness']) {
      'light' => AppearanceBrightness.light,
      'dark' => AppearanceBrightness.dark,
      _ => null,
    };
    final contrast = switch (map['contrast']) {
      'standard' => AppearanceContrast.standard,
      'increased' => AppearanceContrast.increased,
      _ => null,
    };
    final surface = switch (map['surface']) {
      'opaque' => WindowSurfaceMode.opaque,
      'systemManaged' => WindowSurfaceMode.systemManaged,
      _ => null,
    };
    if (sourceId == null ||
        windowToken == null ||
        revision == null ||
        brightness == null ||
        contrast == null ||
        surface == null ||
        map['reduceTransparency'] is! bool ||
        map['reduceMotion'] is! bool) {
      return null;
    }
    final colors = <String, int>{};
    final rawColors = map['systemColors'];
    if (rawColors is Map) {
      for (final entry in rawColors.entries) {
        final key = _boundedString(entry.key, maximum: 64);
        final value = _nonNegativeInt(entry.value);
        if (key == null || value == null || value > 0xffffffff) return null;
        colors[key] = value;
      }
    } else if (rawColors != null) {
      return null;
    }
    final accent = map['accentColor'];
    final accentColor = accent == null ? null : _nonNegativeInt(accent);
    if (accent != null && (accentColor == null || accentColor > 0xffffffff)) {
      return null;
    }
    final fallbackReason = map['fallbackReason'] == null
        ? null
        : _boundedString(map['fallbackReason'], maximum: 128);
    if (map['fallbackReason'] != null && fallbackReason == null) return null;
    return DesktopAppearanceSnapshot(
      sourceId: sourceId,
      windowToken: windowToken,
      revision: revision,
      brightness: brightness,
      contrast: contrast,
      reduceTransparency: map['reduceTransparency'] as bool,
      reduceMotion: map['reduceMotion'] as bool,
      surface: surface,
      capabilities: DesktopAppearanceCapabilities.fromMap(map['capabilities']),
      accentColor: accentColor,
      systemColors: Map.unmodifiable(colors),
      fallbackReason: fallbackReason,
    );
  }
}

/// Transport owned by one Flutter engine's native window.
abstract interface class DesktopAppearanceHost {
  Future<DesktopAppearanceSnapshot?> watch();

  Future<DesktopAppearanceSnapshot?> refresh(
    DesktopAppearanceSnapshot current,
  );

  void listen(Future<void> Function(DesktopAppearanceSnapshot) changed);

  void dispose();
}

class MethodChannelDesktopAppearanceHost implements DesktopAppearanceHost {
  MethodChannelDesktopAppearanceHost([
    this.channel = const MethodChannel('anywhere/appearance'),
  ]);

  final MethodChannel channel;
  Future<void> Function(DesktopAppearanceSnapshot)? _changed;

  @override
  void listen(Future<void> Function(DesktopAppearanceSnapshot) changed) {
    _changed = changed;
    channel.setMethodCallHandler((call) async {
      if (call.method != 'appearanceChanged') return null;
      final snapshot = DesktopAppearanceSnapshot.tryParse(call.arguments);
      if (snapshot != null) await _changed?.call(snapshot);
      return null;
    });
  }

  @override
  Future<DesktopAppearanceSnapshot?> watch() async {
    try {
      return DesktopAppearanceSnapshot.tryParse(
          await channel.invokeMethod<Object?>('watchAppearance'));
    } on MissingPluginException {
      return null;
    } on PlatformException {
      return null;
    }
  }

  @override
  Future<DesktopAppearanceSnapshot?> refresh(
      DesktopAppearanceSnapshot current) async {
    if (current.revision == 0 || current.sourceId == 'local-fallback') {
      return watch();
    }
    try {
      return DesktopAppearanceSnapshot.tryParse(
          await channel.invokeMethod<Object?>('refreshAppearance', {
        'windowToken': current.windowToken,
        'sourceId': current.sourceId,
        'afterRevision': current.revision,
      }));
    } on MissingPluginException {
      return null;
    } on PlatformException {
      return null;
    }
  }

  @override
  void dispose() => channel.setMethodCallHandler(null);
}

/// Owns one engine's latest native appearance and rejects stale publications.
class DesktopAppearanceController extends ChangeNotifier {
  DesktopAppearanceController({
    DesktopAppearanceHost? host,
    TargetPlatform? platform,
  })  : _host = host ?? MethodChannelDesktopAppearanceHost(),
        _snapshot = DesktopAppearanceSnapshot.fallback(platform: platform);

  final DesktopAppearanceHost _host;
  DesktopAppearanceSnapshot _snapshot;
  final Set<String> _retiredBindings = {};
  int _generation = 0;
  bool _binding = false;
  bool _disposed = false;

  DesktopAppearanceSnapshot get snapshot => _snapshot;

  bool get binding => _binding;

  /// Installs the event handler before asking for the initial native snapshot.
  Future<void> bind() async {
    if (_disposed) return;
    final generation = ++_generation;
    _binding = true;
    _host.listen((incoming) async {
      if (_disposed || generation != _generation) return;
      _accept(incoming);
    });
    final initial = await _host.watch();
    if (_disposed || generation != _generation) return;
    if (initial != null) _accept(initial);
    _binding = false;
    notifyListeners();
  }

  /// Re-reads native state after activation or a detected event gap.
  Future<void> refresh() async {
    if (_disposed || _binding) return;
    final generation = _generation;
    final refreshed = await _host.refresh(_snapshot);
    if (_disposed || generation != _generation) return;
    if (refreshed != null) _accept(refreshed);
  }

  void _accept(DesktopAppearanceSnapshot incoming) {
    final current = _snapshot;
    final incomingBinding = '${incoming.sourceId}\u0000${incoming.windowToken}';
    final sameBinding = incoming.sourceId == current.sourceId &&
        incoming.windowToken == current.windowToken;
    if (sameBinding && incoming.revision <= current.revision) return;
    if (!sameBinding && _retiredBindings.contains(incomingBinding)) return;
    if (!sameBinding &&
        current.sourceId != 'local-fallback' &&
        incoming.revision == 0) {
      return;
    }
    final changed = !sameBinding ||
        incoming.revision != current.revision ||
        incoming.brightness != current.brightness ||
        incoming.contrast != current.contrast ||
        incoming.reduceTransparency != current.reduceTransparency ||
        incoming.reduceMotion != current.reduceMotion ||
        incoming.surface != current.surface ||
        incoming.accentColor != current.accentColor ||
        !mapEquals(incoming.systemColors, current.systemColors) ||
        incoming.fallbackReason != current.fallbackReason ||
        incoming.capabilities.protocolVersion !=
            current.capabilities.protocolVersion;
    if (!sameBinding && current.sourceId != 'local-fallback') {
      _retiredBindings.add('${current.sourceId}\u0000${current.windowToken}');
    }
    _snapshot = incoming;
    if (changed) notifyListeners();
  }

  @override
  void dispose() {
    if (_disposed) return;
    _disposed = true;
    ++_generation;
    _host.dispose();
    super.dispose();
  }
}

/// One appearance controller per Flutter engine.
abstract final class DesktopAppearance {
  static DesktopAppearanceController _current = DesktopAppearanceController();

  static DesktopAppearanceController get current => _current;

  static void configure(DesktopAppearanceController controller) {
    _current = controller;
  }

  static void reset() {
    _current.dispose();
    _current = DesktopAppearanceController();
  }
}

Map<Object?, Object?> _stringMap(Object? value) =>
    value is Map ? Map<Object?, Object?>.from(value) : const {};

String? _boundedString(Object? value, {int maximum = 256}) {
  if (value is! String || value.isEmpty || value.length > maximum) return null;
  if (value.runes.any((rune) => rune < 0x20 || rune == 0x7f)) return null;
  return value;
}

int? _nonNegativeInt(Object? value) =>
    value is int && value >= 0 ? value : null;
