import 'dart:async';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'generated/app_localizations.dart';
import 'language_preference.dart';

export 'generated/app_localizations.dart';

/// Presentation preference only. Business identity, matching and execution never
/// depend on a UI locale. Each Flutter engine has its own listener on the Host.
class AppLanguage extends ChangeNotifier with WidgetsBindingObserver {
  AppLanguage({MethodChannel channel = const MethodChannel('anywhere/locale')})
      : _channel = channel;
  static final current = AppLanguage();
  final MethodChannel _channel;
  String _preference = 'system';
  bool _attached = false;
  String get preference => _preference;
  static const preferences = LanguagePreference.values;
  static String normalize(Object? value) => LanguagePreference.normalize(value);

  Locale get locale =>
      resolve(_preference, WidgetsBinding.instance.platformDispatcher.locales);

  static Locale resolve(String preference, List<Locale> system) {
    if (preference == 'zh' || preference == 'en') return Locale(preference);
    // Match the user's preferred language before applying the English fallback.
    for (final locale in system) {
      if (locale.languageCode == 'zh') return const Locale('zh');
      if (locale.languageCode == 'en') return const Locale('en');
    }
    return const Locale('en');
  }

  void apply(Object? value) {
    final next = normalize(value);
    if (_preference == next) return;
    _preference = next;
    notifyListeners();
  }

  Future<void> attach() async {
    if (_attached) return;
    _attached = true;
    WidgetsBinding.instance.addObserver(this);
    var revision = 0;
    _channel.setMethodCallHandler((call) async {
      if (call.method == 'changed') {
        ++revision;
        apply(call.arguments);
      }
    });
    try {
      final value = await _channel
          .invokeMethod<String>('read')
          .timeout(const Duration(seconds: 1));
      // An earlier read must not overwrite a later save notification.
      if (_attached && revision == 0) apply(value);
    } on MissingPluginException {
      // Older or non-native hosts retain the system locale.
    } on PlatformException {
      // Presentation-language failure must not prevent basic app startup.
    } on TimeoutException {
      // Keep startup bounded even when a Host fails to answer; notifications remain active.
    }
  }

  @override
  void didChangeLocales(List<Locale>? locales) {
    if (_preference == 'system') notifyListeners();
  }

  @override
  void dispose() {
    if (_attached) {
      _channel.setMethodCallHandler(null);
      WidgetsBinding.instance.removeObserver(this);
      _attached = false;
    }
    super.dispose();
  }
}

extension AppLocalizationContext on BuildContext {
  AppLocalizations get l10n =>
      Localizations.of<AppLocalizations>(this, AppLocalizations) ??
      lookupAppLocalizations(const Locale('en'));
}

/// Shared delegates and live language updates for Launcher and auxiliary engines.
class LocalizedApp extends StatelessWidget {
  const LocalizedApp(
      {super.key,
      required this.home,
      this.title = 'Anywhere',
      this.theme,
      this.builder,
      this.debugShowCheckedModeBanner = false});
  final Widget home;
  final String title;
  final ThemeData? theme;
  final TransitionBuilder? builder;
  final bool debugShowCheckedModeBanner;
  @override
  Widget build(BuildContext context) => ListenableBuilder(
      listenable: AppLanguage.current,
      builder: (context, _) => MaterialApp(
          debugShowCheckedModeBanner: debugShowCheckedModeBanner,
          title: title,
          theme: theme,
          builder: builder,
          locale: AppLanguage.current.locale,
          localizationsDelegates: AppLocalizations.localizationsDelegates,
          supportedLocales: AppLocalizations.supportedLocales,
          home: home));
}
