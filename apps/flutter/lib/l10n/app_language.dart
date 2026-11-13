import 'dart:async';

import 'package:flutter/services.dart';
import 'package:material_ui/material_ui.dart';

import '../infrastructure/desktop_appearance.dart';
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
  // Design delegates follow the standalone packages used by the widget tree.
  // gen-l10n still emits the SDK design delegates in its convenience list.
  static const localizationsDelegates = <LocalizationsDelegate<dynamic>>[
    AppLocalizations.delegate,
    ...GlobalMaterialLocalizations.delegates,
  ];
  const LocalizedApp(
      {super.key,
      this.home,
      this.homeBuilder,
      this.title = 'Anywhere',
      this.theme,
      this.themeBuilder,
      this.builder,
      this.debugShowCheckedModeBanner = false})
      : assert(home != null || homeBuilder != null);
  final Widget? home;

  /// Reconstructs the presentation root for each live locale / appearance
  /// version. The same controller and widget keys preserve page state.
  final Widget Function()? homeBuilder;
  final String title;
  final ThemeData? theme;
  final ThemeData Function()? themeBuilder;
  final TransitionBuilder? builder;
  final bool debugShowCheckedModeBanner;
  @override
  Widget build(BuildContext context) => ListenableBuilder(
      listenable:
          Listenable.merge([AppLanguage.current, DesktopAppearance.current]),
      builder: (context, _) => MaterialApp(
          debugShowCheckedModeBanner: debugShowCheckedModeBanner,
          title: title,
          theme: themeBuilder?.call() ?? theme,
          builder: (context, child) {
            final media = MediaQuery.of(context);
            final content = MediaQuery(
                data: media.copyWith(
                    disableAnimations: media.disableAnimations ||
                        DesktopAppearance.current.snapshot.reduceMotion),
                child: child ?? const SizedBox.shrink());
            return builder?.call(context, content) ?? content;
          },
          locale: AppLanguage.current.locale,
          localizationsDelegates: localizationsDelegates,
          supportedLocales: AppLocalizations.supportedLocales,
          home: homeBuilder?.call() ?? home!));
}
