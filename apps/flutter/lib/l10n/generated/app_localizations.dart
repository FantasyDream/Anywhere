import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_localizations/flutter_localizations.dart';
import 'package:intl/intl.dart' as intl;

import 'app_localizations_en.dart';
import 'app_localizations_zh.dart';

// ignore_for_file: type=lint

/// Callers can lookup localized strings with an instance of AppLocalizations
/// returned by `AppLocalizations.of(context)`.
///
/// Applications need to include `AppLocalizations.delegate()` in their app's
/// `localizationDelegates` list, and the locales they support in the app's
/// `supportedLocales` list. For example:
///
/// ```dart
/// import 'generated/app_localizations.dart';
///
/// return MaterialApp(
///   localizationsDelegates: AppLocalizations.localizationsDelegates,
///   supportedLocales: AppLocalizations.supportedLocales,
///   home: MyApplicationHome(),
/// );
/// ```
///
/// ## Update pubspec.yaml
///
/// Please make sure to update your pubspec.yaml to include the following
/// packages:
///
/// ```yaml
/// dependencies:
///   # Internationalization support.
///   flutter_localizations:
///     sdk: flutter
///   intl: any # Use the pinned version from flutter_localizations
///
///   # Rest of dependencies
/// ```
///
/// ## iOS Applications
///
/// iOS applications define key application metadata, including supported
/// locales, in an Info.plist file that is built into the application bundle.
/// To configure the locales supported by your app, you’ll need to edit this
/// file.
///
/// First, open your project’s ios/Runner.xcworkspace Xcode workspace file.
/// Then, in the Project Navigator, open the Info.plist file under the Runner
/// project’s Runner folder.
///
/// Next, select the Information Property List item, select Add Item from the
/// Editor menu, then select Localizations from the pop-up menu.
///
/// Select and expand the newly-created Localizations item then, for each
/// locale your application supports, add a new item and select the locale
/// you wish to add from the pop-up menu in the Value field. This list should
/// be consistent with the languages listed in the AppLocalizations.supportedLocales
/// property.
abstract class AppLocalizations {
  AppLocalizations(String locale) : localeName = intl.Intl.canonicalizedLocale(locale.toString());

  final String localeName;

  static AppLocalizations of(BuildContext context) {
    return Localizations.of<AppLocalizations>(context, AppLocalizations)!;
  }

  static const LocalizationsDelegate<AppLocalizations> delegate = _AppLocalizationsDelegate();

  /// A list of this localizations delegate along with the default localizations
  /// delegates.
  ///
  /// Returns a list of localizations delegates containing this delegate along with
  /// GlobalMaterialLocalizations.delegate, GlobalCupertinoLocalizations.delegate,
  /// and GlobalWidgetsLocalizations.delegate.
  ///
  /// Additional delegates can be added by appending to this list in
  /// MaterialApp. This list does not have to be used at all if a custom list
  /// of delegates is preferred or required.
  static const List<LocalizationsDelegate<dynamic>> localizationsDelegates = <LocalizationsDelegate<dynamic>>[
    delegate,
    GlobalMaterialLocalizations.delegate,
    GlobalCupertinoLocalizations.delegate,
    GlobalWidgetsLocalizations.delegate,
  ];

  /// A list of this localizations delegate's supported locales.
  static const List<Locale> supportedLocales = <Locale>[
    Locale('en'),
    Locale('zh')
  ];

  /// No description provided for @settings.
  ///
  /// In en, this message translates to:
  /// **'Settings'**
  String get settings;

  /// No description provided for @general.
  ///
  /// In en, this message translates to:
  /// **'General'**
  String get general;

  /// No description provided for @shortcuts.
  ///
  /// In en, this message translates to:
  /// **'Shortcuts'**
  String get shortcuts;

  /// No description provided for @context.
  ///
  /// In en, this message translates to:
  /// **'Context'**
  String get context;

  /// No description provided for @search.
  ///
  /// In en, this message translates to:
  /// **'Search'**
  String get search;

  /// No description provided for @clipboard.
  ///
  /// In en, this message translates to:
  /// **'Clipboard'**
  String get clipboard;

  /// No description provided for @features.
  ///
  /// In en, this message translates to:
  /// **'Features'**
  String get features;

  /// No description provided for @language.
  ///
  /// In en, this message translates to:
  /// **'Language'**
  String get language;

  /// No description provided for @languageDescription.
  ///
  /// In en, this message translates to:
  /// **'Choose the language used throughout Anywhere'**
  String get languageDescription;

  /// No description provided for @languageSystem.
  ///
  /// In en, this message translates to:
  /// **'System default'**
  String get languageSystem;

  /// No description provided for @languageEnglish.
  ///
  /// In en, this message translates to:
  /// **'English'**
  String get languageEnglish;

  /// No description provided for @languageChinese.
  ///
  /// In en, this message translates to:
  /// **'简体中文'**
  String get languageChinese;

  /// No description provided for @done.
  ///
  /// In en, this message translates to:
  /// **'Done'**
  String get done;

  /// No description provided for @saving.
  ///
  /// In en, this message translates to:
  /// **'Saving…'**
  String get saving;

  /// No description provided for @openAtLogin.
  ///
  /// In en, this message translates to:
  /// **'Open at login'**
  String get openAtLogin;

  /// No description provided for @openAtLoginDescription.
  ///
  /// In en, this message translates to:
  /// **'Start quietly when you sign in'**
  String get openAtLoginDescription;

  /// No description provided for @currentApp.
  ///
  /// In en, this message translates to:
  /// **'Current App'**
  String get currentApp;

  /// No description provided for @globalSearch.
  ///
  /// In en, this message translates to:
  /// **'Global Search'**
  String get globalSearch;

  /// No description provided for @universalSearch.
  ///
  /// In en, this message translates to:
  /// **'Universal Search'**
  String get universalSearch;

  /// No description provided for @navigationModes.
  ///
  /// In en, this message translates to:
  /// **'Global Search / Current App'**
  String get navigationModes;

  /// No description provided for @backToResults.
  ///
  /// In en, this message translates to:
  /// **'Back to results'**
  String get backToResults;

  /// No description provided for @back.
  ///
  /// In en, this message translates to:
  /// **'Back'**
  String get back;

  /// No description provided for @run.
  ///
  /// In en, this message translates to:
  /// **'Run'**
  String get run;

  /// No description provided for @open.
  ///
  /// In en, this message translates to:
  /// **'Open'**
  String get open;

  /// No description provided for @copy.
  ///
  /// In en, this message translates to:
  /// **'Copy'**
  String get copy;

  /// No description provided for @actions.
  ///
  /// In en, this message translates to:
  /// **'Actions'**
  String get actions;

  /// No description provided for @retry.
  ///
  /// In en, this message translates to:
  /// **'Retry'**
  String get retry;

  /// No description provided for @cancel.
  ///
  /// In en, this message translates to:
  /// **'Cancel'**
  String get cancel;

  /// No description provided for @close.
  ///
  /// In en, this message translates to:
  /// **'Close'**
  String get close;

  /// No description provided for @clear.
  ///
  /// In en, this message translates to:
  /// **'Clear'**
  String get clear;

  /// No description provided for @remove.
  ///
  /// In en, this message translates to:
  /// **'Remove'**
  String get remove;

  /// No description provided for @delete.
  ///
  /// In en, this message translates to:
  /// **'Delete'**
  String get delete;

  /// No description provided for @preview.
  ///
  /// In en, this message translates to:
  /// **'Preview'**
  String get preview;

  /// No description provided for @kind.
  ///
  /// In en, this message translates to:
  /// **'Kind'**
  String get kind;

  /// No description provided for @size.
  ///
  /// In en, this message translates to:
  /// **'Size'**
  String get size;

  /// No description provided for @modified.
  ///
  /// In en, this message translates to:
  /// **'Modified'**
  String get modified;

  /// No description provided for @location.
  ///
  /// In en, this message translates to:
  /// **'Location'**
  String get location;

  /// No description provided for @allTypes.
  ///
  /// In en, this message translates to:
  /// **'All types'**
  String get allTypes;

  /// No description provided for @files.
  ///
  /// In en, this message translates to:
  /// **'Files'**
  String get files;

  /// No description provided for @folders.
  ///
  /// In en, this message translates to:
  /// **'Folders'**
  String get folders;

  /// No description provided for @documents.
  ///
  /// In en, this message translates to:
  /// **'Documents'**
  String get documents;

  /// No description provided for @pdf.
  ///
  /// In en, this message translates to:
  /// **'PDF'**
  String get pdf;

  /// No description provided for @images.
  ///
  /// In en, this message translates to:
  /// **'Images'**
  String get images;

  /// No description provided for @audio.
  ///
  /// In en, this message translates to:
  /// **'Audio'**
  String get audio;

  /// No description provided for @video.
  ///
  /// In en, this message translates to:
  /// **'Video'**
  String get video;

  /// No description provided for @archives.
  ///
  /// In en, this message translates to:
  /// **'Archives'**
  String get archives;

  /// No description provided for @code.
  ///
  /// In en, this message translates to:
  /// **'Code'**
  String get code;

  /// No description provided for @fileSearch.
  ///
  /// In en, this message translates to:
  /// **'File Search'**
  String get fileSearch;

  /// No description provided for @clipboardHistory.
  ///
  /// In en, this message translates to:
  /// **'Clipboard History'**
  String get clipboardHistory;

  /// No description provided for @clipboardShelf.
  ///
  /// In en, this message translates to:
  /// **'Clipboard Shelf'**
  String get clipboardShelf;

  /// No description provided for @runningTasks.
  ///
  /// In en, this message translates to:
  /// **'Running Tasks'**
  String get runningTasks;

  /// No description provided for @searchAppsActions.
  ///
  /// In en, this message translates to:
  /// **'Search apps and actions…'**
  String get searchAppsActions;

  /// No description provided for @searchFilesFolders.
  ///
  /// In en, this message translates to:
  /// **'Search files and folders…'**
  String get searchFilesFolders;

  /// No description provided for @searchHistory.
  ///
  /// In en, this message translates to:
  /// **'Search clipboard history…'**
  String get searchHistory;

  /// No description provided for @findFilesFolders.
  ///
  /// In en, this message translates to:
  /// **'Find files and folders'**
  String get findFilesFolders;

  /// No description provided for @noResults.
  ///
  /// In en, this message translates to:
  /// **'No results'**
  String get noResults;

  /// No description provided for @copied.
  ///
  /// In en, this message translates to:
  /// **'Copied'**
  String get copied;

  /// No description provided for @copiedPaste.
  ///
  /// In en, this message translates to:
  /// **'Copied. Paste in the destination app.'**
  String get copiedPaste;

  /// No description provided for @copyFailed.
  ///
  /// In en, this message translates to:
  /// **'Could not copy. Try again.'**
  String get copyFailed;

  /// No description provided for @loading.
  ///
  /// In en, this message translates to:
  /// **'Loading…'**
  String get loading;

  /// No description provided for @searching.
  ///
  /// In en, this message translates to:
  /// **'Searching…'**
  String get searching;

  /// No description provided for @suggested.
  ///
  /// In en, this message translates to:
  /// **'Suggested'**
  String get suggested;

  /// No description provided for @menus.
  ///
  /// In en, this message translates to:
  /// **'Menus'**
  String get menus;

  /// No description provided for @browseMenus.
  ///
  /// In en, this message translates to:
  /// **'Browse Menus…'**
  String get browseMenus;

  /// No description provided for @contextSources.
  ///
  /// In en, this message translates to:
  /// **'CONTEXT SOURCES'**
  String get contextSources;

  /// No description provided for @contextDescription.
  ///
  /// In en, this message translates to:
  /// **'Choose what Anywhere can use when you open it.'**
  String get contextDescription;

  /// No description provided for @finderSelection.
  ///
  /// In en, this message translates to:
  /// **'Finder selection'**
  String get finderSelection;

  /// No description provided for @fileSelection.
  ///
  /// In en, this message translates to:
  /// **'File selection'**
  String get fileSelection;

  /// No description provided for @selectedFilesDescription.
  ///
  /// In en, this message translates to:
  /// **'Selected files and folders'**
  String get selectedFilesDescription;

  /// No description provided for @selectedText.
  ///
  /// In en, this message translates to:
  /// **'Selected text'**
  String get selectedText;

  /// No description provided for @selectedTextDescription.
  ///
  /// In en, this message translates to:
  /// **'Text selected in the current app'**
  String get selectedTextDescription;

  /// No description provided for @browserCurrentPage.
  ///
  /// In en, this message translates to:
  /// **'Browser current page'**
  String get browserCurrentPage;

  /// No description provided for @clipboardText.
  ///
  /// In en, this message translates to:
  /// **'Clipboard text'**
  String get clipboardText;

  /// No description provided for @openSettings.
  ///
  /// In en, this message translates to:
  /// **'Open Settings'**
  String get openSettings;

  /// No description provided for @keepEditing.
  ///
  /// In en, this message translates to:
  /// **'Keep editing'**
  String get keepEditing;

  /// No description provided for @closeWithoutSaving.
  ///
  /// In en, this message translates to:
  /// **'Close without saving'**
  String get closeWithoutSaving;

  /// No description provided for @saveChanges.
  ///
  /// In en, this message translates to:
  /// **'Save changes'**
  String get saveChanges;

  /// No description provided for @saveBeforeClose.
  ///
  /// In en, this message translates to:
  /// **'Save changes before closing?'**
  String get saveBeforeClose;

  /// No description provided for @changesNotSaved.
  ///
  /// In en, this message translates to:
  /// **'Some changes were not saved'**
  String get changesNotSaved;

  /// No description provided for @searchDrafts.
  ///
  /// In en, this message translates to:
  /// **'Search settings have unapplied changes.'**
  String get searchDrafts;

  /// No description provided for @settingsSaveFailed.
  ///
  /// In en, this message translates to:
  /// **'Settings could not be saved. Try again.'**
  String get settingsSaveFailed;

  /// No description provided for @shortcutHelp.
  ///
  /// In en, this message translates to:
  /// **'Choose a shortcut, then press the new key combination. Esc cancels.'**
  String get shortcutHelp;

  /// No description provided for @workspace.
  ///
  /// In en, this message translates to:
  /// **'Workspace'**
  String get workspace;

  /// No description provided for @image.
  ///
  /// In en, this message translates to:
  /// **'Image'**
  String get image;

  /// No description provided for @richText.
  ///
  /// In en, this message translates to:
  /// **'Rich text'**
  String get richText;

  /// No description provided for @data.
  ///
  /// In en, this message translates to:
  /// **'Data'**
  String get data;

  /// No description provided for @text.
  ///
  /// In en, this message translates to:
  /// **'Text'**
  String get text;

  /// No description provided for @searchAppActions.
  ///
  /// In en, this message translates to:
  /// **'Search {app} actions…'**
  String searchAppActions(String app);

  /// No description provided for @contentEscapeHint.
  ///
  /// In en, this message translates to:
  /// **'{source} · Esc → Current App'**
  String contentEscapeHint(String source);

  /// No description provided for @fileRange.
  ///
  /// In en, this message translates to:
  /// **'File Search · {range}'**
  String fileRange(String range);

  /// No description provided for @editCapturedContent.
  ///
  /// In en, this message translates to:
  /// **'Edit captured content…'**
  String get editCapturedContent;

  /// No description provided for @fileLocations.
  ///
  /// In en, this message translates to:
  /// **'File Search · {count} locations'**
  String fileLocations(int count);

  /// No description provided for @actionShortcuts.
  ///
  /// In en, this message translates to:
  /// **'Action shortcuts'**
  String get actionShortcuts;

  /// No description provided for @bindAKeyToAnActionOr.
  ///
  /// In en, this message translates to:
  /// **'Bind a key to an action, or choose actions by context.'**
  String get bindAKeyToAnActionOr;

  /// No description provided for @globalWorksFromOtherApps.
  ///
  /// In en, this message translates to:
  /// **'Global · works from other apps'**
  String get globalWorksFromOtherApps;

  /// No description provided for @whileAnywhereIsOpen.
  ///
  /// In en, this message translates to:
  /// **'While Anywhere is open'**
  String get whileAnywhereIsOpen;

  /// No description provided for @actionUnavailableShortcutPaused.
  ///
  /// In en, this message translates to:
  /// **'Action unavailable · shortcut paused'**
  String get actionUnavailableShortcutPaused;

  /// No description provided for @actionsUnavailableShortcutPaused.
  ///
  /// In en, this message translates to:
  /// **'Actions unavailable · shortcut paused'**
  String get actionsUnavailableShortcutPaused;

  /// No description provided for @someRulesAreUnavailable.
  ///
  /// In en, this message translates to:
  /// **'Some rules are unavailable'**
  String get someRulesAreUnavailable;

  /// No description provided for @notDetectedInTheLastContext.
  ///
  /// In en, this message translates to:
  /// **'Not detected in the last context'**
  String get notDetectedInTheLastContext;

  /// No description provided for @addActionShortcut.
  ///
  /// In en, this message translates to:
  /// **'Add action shortcut'**
  String get addActionShortcut;

  /// No description provided for @addContextShortcut.
  ///
  /// In en, this message translates to:
  /// **'Add context shortcut'**
  String get addContextShortcut;

  /// No description provided for @toolsSuchAsFileSearchAreAlways.
  ///
  /// In en, this message translates to:
  /// **'Tools such as File Search are always available to bind. Project actions appear when their project is active.'**
  String get toolsSuchAsFileSearchAreAlways;

  /// No description provided for @actionShortcut.
  ///
  /// In en, this message translates to:
  /// **'Action shortcut'**
  String get actionShortcut;

  /// No description provided for @findAnAction.
  ///
  /// In en, this message translates to:
  /// **'Find an action…'**
  String get findAnAction;

  /// No description provided for @notDetectedInTheLastContextThe.
  ///
  /// In en, this message translates to:
  /// **'Not detected in the last context. The saved action is retained.'**
  String get notDetectedInTheLastContextThe;

  /// No description provided for @shortcutRecordingFailedTryAgain.
  ///
  /// In en, this message translates to:
  /// **'Shortcut recording failed. Try again.'**
  String get shortcutRecordingFailedTryAgain;

  /// No description provided for @pressAShortcut.
  ///
  /// In en, this message translates to:
  /// **'Press a shortcut…'**
  String get pressAShortcut;

  /// No description provided for @recordShortcut.
  ///
  /// In en, this message translates to:
  /// **'Record shortcut'**
  String get recordShortcut;

  /// No description provided for @useCommandOptionOrControlWithA.
  ///
  /// In en, this message translates to:
  /// **'Use Command, Option or Control with a letter. Esc cancels recording.'**
  String get useCommandOptionOrControlWithA;

  /// No description provided for @globalShortcut.
  ///
  /// In en, this message translates to:
  /// **'Global shortcut'**
  String get globalShortcut;

  /// No description provided for @worksFromOtherAppsUsesFreshContext.
  ///
  /// In en, this message translates to:
  /// **'Works from other apps. Uses fresh context when needed.'**
  String get worksFromOtherAppsUsesFreshContext;

  /// No description provided for @chooseAnotherAction.
  ///
  /// In en, this message translates to:
  /// **'Choose another action'**
  String get chooseAnotherAction;

  /// No description provided for @saveShortcut.
  ///
  /// In en, this message translates to:
  /// **'Save shortcut'**
  String get saveShortcut;

  /// No description provided for @clearClipboardHistory.
  ///
  /// In en, this message translates to:
  /// **'Clear clipboard history?'**
  String get clearClipboardHistory;

  /// No description provided for @allSavedEntriesWillBePermanentlyRemoved.
  ///
  /// In en, this message translates to:
  /// **'All saved entries will be permanently removed. The current system clipboard is unchanged.'**
  String get allSavedEntriesWillBePermanentlyRemoved;

  /// No description provided for @clearHistory.
  ///
  /// In en, this message translates to:
  /// **'Clear History'**
  String get clearHistory;

  /// No description provided for @clipboardHistorySetting.
  ///
  /// In en, this message translates to:
  /// **'Clipboard history'**
  String get clipboardHistorySetting;

  /// No description provided for @recordClipboardHistory.
  ///
  /// In en, this message translates to:
  /// **'Record clipboard history'**
  String get recordClipboardHistory;

  /// No description provided for @saveFutureCopiesWhileAnywhereIsRunning.
  ///
  /// In en, this message translates to:
  /// **'Save future copies while Anywhere is running. Existing clipboard contents are not imported.'**
  String get saveFutureCopiesWhileAnywhereIsRunning;

  /// No description provided for @textImagesRichTextAndFileReferences.
  ///
  /// In en, this message translates to:
  /// **'Text, images, rich text and file references are saved locally. Pausing keeps existing history.'**
  String get textImagesRichTextAndFileReferences;

  /// No description provided for @tryAgain.
  ///
  /// In en, this message translates to:
  /// **'Try Again'**
  String get tryAgain;

  /// No description provided for @whenOpeningLauncher.
  ///
  /// In en, this message translates to:
  /// **'When opening Launcher'**
  String get whenOpeningLauncher;

  /// No description provided for @autoFillSupportedContentCopiedWithinThe.
  ///
  /// In en, this message translates to:
  /// **'Auto-fill supported content copied within the last 15 seconds. Independent of history recording.'**
  String get autoFillSupportedContentCopiedWithinThe;

  /// No description provided for @historyStorage.
  ///
  /// In en, this message translates to:
  /// **'History storage'**
  String get historyStorage;

  /// No description provided for @removeAllSavedEntriesThisDoesNot.
  ///
  /// In en, this message translates to:
  /// **'Remove all saved entries. This does not change recording or the current system clipboard.'**
  String get removeAllSavedEntriesThisDoesNot;

  /// No description provided for @clearHistoryAction.
  ///
  /// In en, this message translates to:
  /// **'Clear History…'**
  String get clearHistoryAction;

  /// No description provided for @enabled.
  ///
  /// In en, this message translates to:
  /// **'Enabled'**
  String get enabled;

  /// No description provided for @disabled.
  ///
  /// In en, this message translates to:
  /// **'Disabled'**
  String get disabled;

  /// No description provided for @awaitingPermission.
  ///
  /// In en, this message translates to:
  /// **'Awaiting permission'**
  String get awaitingPermission;

  /// No description provided for @couldNotStart.
  ///
  /// In en, this message translates to:
  /// **'Could not start'**
  String get couldNotStart;

  /// No description provided for @unavailable.
  ///
  /// In en, this message translates to:
  /// **'Unavailable'**
  String get unavailable;

  /// No description provided for @restartRequired.
  ///
  /// In en, this message translates to:
  /// **'Restart required'**
  String get restartRequired;

  /// No description provided for @updating.
  ///
  /// In en, this message translates to:
  /// **'Updating…'**
  String get updating;

  /// No description provided for @refresh.
  ///
  /// In en, this message translates to:
  /// **'Refresh'**
  String get refresh;

  /// No description provided for @chooseTheToolsAvailableInAnywhere.
  ///
  /// In en, this message translates to:
  /// **'Choose the tools available in Anywhere.'**
  String get chooseTheToolsAvailableInAnywhere;

  /// No description provided for @builtIn.
  ///
  /// In en, this message translates to:
  /// **'Built in'**
  String get builtIn;

  /// No description provided for @installedPlugins.
  ///
  /// In en, this message translates to:
  /// **'Installed plugins'**
  String get installedPlugins;

  /// No description provided for @legacyV1PermissionsCannotBeRevokedIndividually.
  ///
  /// In en, this message translates to:
  /// **'Legacy v1 permissions cannot be revoked individually. Disable to stop its actions.'**
  String get legacyV1PermissionsCannotBeRevokedIndividually;

  /// No description provided for @reviewPermissions.
  ///
  /// In en, this message translates to:
  /// **'Review permissions…'**
  String get reviewPermissions;

  /// No description provided for @revokePermissions.
  ///
  /// In en, this message translates to:
  /// **'Revoke permissions'**
  String get revokePermissions;

  /// No description provided for @thisExtensionRequestsPermissionTo.
  ///
  /// In en, this message translates to:
  /// **'This extension requests permission to:'**
  String get thisExtensionRequestsPermissionTo;

  /// No description provided for @noHostPermissionsRequested.
  ///
  /// In en, this message translates to:
  /// **'No host permissions requested.'**
  String get noHostPermissionsRequested;

  /// No description provided for @approvalAppliesToThisInspectedDefinitionRestart.
  ///
  /// In en, this message translates to:
  /// **'Approval applies to this inspected definition. Restart Anywhere to activate a newly approved plugin. Revoking permission takes effect immediately.'**
  String get approvalAppliesToThisInspectedDefinitionRestart;

  /// No description provided for @allowForNextLaunch.
  ///
  /// In en, this message translates to:
  /// **'Allow for next launch'**
  String get allowForNextLaunch;

  /// No description provided for @showAPluginViewInAnywhere.
  ///
  /// In en, this message translates to:
  /// **'Show a plugin view in Anywhere'**
  String get showAPluginViewInAnywhere;

  /// No description provided for @writeToTheClipboard.
  ///
  /// In en, this message translates to:
  /// **'Write to the clipboard'**
  String get writeToTheClipboard;

  /// No description provided for @openWebAddresses.
  ///
  /// In en, this message translates to:
  /// **'Open web addresses'**
  String get openWebAddresses;

  /// No description provided for @openFiles.
  ///
  /// In en, this message translates to:
  /// **'Open files'**
  String get openFiles;

  /// No description provided for @readRepositoryInformation.
  ///
  /// In en, this message translates to:
  /// **'Read repository information'**
  String get readRepositoryInformation;

  /// No description provided for @runProjectTasks.
  ///
  /// In en, this message translates to:
  /// **'Run project tasks'**
  String get runProjectTasks;

  /// No description provided for @saveExtensionData.
  ///
  /// In en, this message translates to:
  /// **'Save extension data'**
  String get saveExtensionData;

  /// No description provided for @fileSearchSetting.
  ///
  /// In en, this message translates to:
  /// **'File search'**
  String get fileSearchSetting;

  /// No description provided for @searchFileNamesInTheFileSearch.
  ///
  /// In en, this message translates to:
  /// **'Search file names in the File Search action'**
  String get searchFileNamesInTheFileSearch;

  /// No description provided for @systemSearchShowsEarlyResultsFilenameScanning.
  ///
  /// In en, this message translates to:
  /// **'System search shows early results. Filename scanning uses accessible, authorized directories.'**
  String get systemSearchShowsEarlyResultsFilenameScanning;

  /// No description provided for @includeHiddenFiles.
  ///
  /// In en, this message translates to:
  /// **'Include hidden files'**
  String get includeHiddenFiles;

  /// No description provided for @includeDotfilesAndFilesInHiddenDirectories.
  ///
  /// In en, this message translates to:
  /// **'Include dotfiles and files in hidden directories'**
  String get includeDotfilesAndFilesInHiddenDirectories;

  /// No description provided for @searchDirectories.
  ///
  /// In en, this message translates to:
  /// **'Search directories'**
  String get searchDirectories;

  /// No description provided for @excludedDirectoryNames.
  ///
  /// In en, this message translates to:
  /// **'Exclusions must be directory names, one per line.'**
  String get excludedDirectoryNames;

  /// No description provided for @oneDirectoryNamePerLine.
  ///
  /// In en, this message translates to:
  /// **'One directory name per line'**
  String get oneDirectoryNamePerLine;

  /// No description provided for @filesAndFoldersAreScannedIncludingGitignored.
  ///
  /// In en, this message translates to:
  /// **'Files and folders are scanned, including gitignored files. Exclusions narrow the search range. Symbolic links are listed without scanning their targets.'**
  String get filesAndFoldersAreScannedIncludingGitignored;

  /// No description provided for @directoryAccessIsPreparedAtStartupAnd.
  ///
  /// In en, this message translates to:
  /// **'Directory access is prepared at startup and when the search range changes. Protected locations are skipped. Full Disk Access is optional and must be enabled in System Settings.'**
  String get directoryAccessIsPreparedAtStartupAnd;

  /// No description provided for @authorizeSavedDirectories.
  ///
  /// In en, this message translates to:
  /// **'Authorize saved directories'**
  String get authorizeSavedDirectories;

  /// No description provided for @fullDiskAccess.
  ///
  /// In en, this message translates to:
  /// **'Full Disk Access…'**
  String get fullDiskAccess;

  /// No description provided for @saveFileSearch.
  ///
  /// In en, this message translates to:
  /// **'Save file search'**
  String get saveFileSearch;

  /// No description provided for @addAtLeastOneRule.
  ///
  /// In en, this message translates to:
  /// **'Add at least one rule.'**
  String get addAtLeastOneRule;

  /// No description provided for @contextShortcut.
  ///
  /// In en, this message translates to:
  /// **'Context shortcut'**
  String get contextShortcut;

  /// No description provided for @shortcutName.
  ///
  /// In en, this message translates to:
  /// **'Shortcut name'**
  String get shortcutName;

  /// No description provided for @eGOpenTerminal.
  ///
  /// In en, this message translates to:
  /// **'e.g. Open terminal'**
  String get eGOpenTerminal;

  /// No description provided for @global.
  ///
  /// In en, this message translates to:
  /// **'Global'**
  String get global;

  /// No description provided for @moreSpecificConditionsTakePriorityEqualMatches.
  ///
  /// In en, this message translates to:
  /// **'More specific conditions take priority. Equal matches let you choose. An unavailable matching action will not fall back.'**
  String get moreSpecificConditionsTakePriorityEqualMatches;

  /// No description provided for @noRulesYetAddAnActionAnd.
  ///
  /// In en, this message translates to:
  /// **'No rules yet. Add an action and choose the app or context where it should run.'**
  String get noRulesYetAddAnActionAnd;

  /// No description provided for @addRule.
  ///
  /// In en, this message translates to:
  /// **'Add rule'**
  String get addRule;

  /// No description provided for @saveContextShortcut.
  ///
  /// In en, this message translates to:
  /// **'Save context shortcut'**
  String get saveContextShortcut;

  /// No description provided for @savedApp.
  ///
  /// In en, this message translates to:
  /// **'Saved app'**
  String get savedApp;

  /// No description provided for @shortcutRule.
  ///
  /// In en, this message translates to:
  /// **'Shortcut rule'**
  String get shortcutRule;

  /// No description provided for @inApp.
  ///
  /// In en, this message translates to:
  /// **'In app'**
  String get inApp;

  /// No description provided for @anyApp.
  ///
  /// In en, this message translates to:
  /// **'Any app'**
  String get anyApp;

  /// No description provided for @withContext.
  ///
  /// In en, this message translates to:
  /// **'With context'**
  String get withContext;

  /// No description provided for @anyCompatibleContext.
  ///
  /// In en, this message translates to:
  /// **'Any compatible context'**
  String get anyCompatibleContext;

  /// No description provided for @aRuleAlreadyUsesTheseConditions.
  ///
  /// In en, this message translates to:
  /// **'A rule already uses these conditions.'**
  String get aRuleAlreadyUsesTheseConditions;

  /// No description provided for @saveRule.
  ///
  /// In en, this message translates to:
  /// **'Save rule'**
  String get saveRule;

  /// No description provided for @finishOrCancelEditingTheActionShortcut.
  ///
  /// In en, this message translates to:
  /// **'Finish or cancel editing the action shortcut before closing.'**
  String get finishOrCancelEditingTheActionShortcut;

  /// No description provided for @settingsCouldNotBeSavedYouCan.
  ///
  /// In en, this message translates to:
  /// **'Settings could not be saved. You can keep editing or close without saving.'**
  String get settingsCouldNotBeSavedYouCan;

  /// No description provided for @newEditsWereMadeWhileSavingSave.
  ///
  /// In en, this message translates to:
  /// **'New edits were made while saving. Save them before closing.'**
  String get newEditsWereMadeWhileSavingSave;

  /// No description provided for @changesApplyOnTheNextWakeStatus.
  ///
  /// In en, this message translates to:
  /// **'Changes apply on the next wake. Status refers to the last capture.'**
  String get changesApplyOnTheNextWakeStatus;

  /// No description provided for @hiddenApps.
  ///
  /// In en, this message translates to:
  /// **'Hidden apps'**
  String get hiddenApps;

  /// No description provided for @enterOneApplicationIDPerLineTo.
  ///
  /// In en, this message translates to:
  /// **'Enter one application ID per line to keep it out of search.'**
  String get enterOneApplicationIDPerLineTo;

  /// No description provided for @saveApps.
  ///
  /// In en, this message translates to:
  /// **'Save apps'**
  String get saveApps;

  /// No description provided for @pressShortcutKey.
  ///
  /// In en, this message translates to:
  /// **'Press a shortcut'**
  String get pressShortcutKey;

  /// No description provided for @chooseApp.
  ///
  /// In en, this message translates to:
  /// **'Choose app'**
  String get chooseApp;

  /// No description provided for @findAnApp.
  ///
  /// In en, this message translates to:
  /// **'Find an app…'**
  String get findAnApp;

  /// No description provided for @noMatchingApps.
  ///
  /// In en, this message translates to:
  /// **'No matching apps'**
  String get noMatchingApps;

  /// No description provided for @outputCouldNotBeCopiedTryAgain.
  ///
  /// In en, this message translates to:
  /// **'Output could not be copied. Try again.'**
  String get outputCouldNotBeCopiedTryAgain;

  /// No description provided for @output.
  ///
  /// In en, this message translates to:
  /// **'Output'**
  String get output;

  /// No description provided for @diagnostics.
  ///
  /// In en, this message translates to:
  /// **'Diagnostics'**
  String get diagnostics;

  /// No description provided for @copyOutput.
  ///
  /// In en, this message translates to:
  /// **'Copy output'**
  String get copyOutput;

  /// No description provided for @noDiagnosticOutput.
  ///
  /// In en, this message translates to:
  /// **'No diagnostic output.'**
  String get noDiagnosticOutput;

  /// No description provided for @waitingForOutput.
  ///
  /// In en, this message translates to:
  /// **'Waiting for output…'**
  String get waitingForOutput;

  /// No description provided for @noStandardOutput.
  ///
  /// In en, this message translates to:
  /// **'No standard output.'**
  String get noStandardOutput;

  /// No description provided for @followNewOutput.
  ///
  /// In en, this message translates to:
  /// **'Follow new output'**
  String get followNewOutput;

  /// No description provided for @jumpToLatestOutput.
  ///
  /// In en, this message translates to:
  /// **'Jump to latest output'**
  String get jumpToLatestOutput;

  /// No description provided for @following.
  ///
  /// In en, this message translates to:
  /// **'Following'**
  String get following;

  /// No description provided for @latest.
  ///
  /// In en, this message translates to:
  /// **'Latest'**
  String get latest;

  /// No description provided for @theWindowCouldNotBeChangedTry.
  ///
  /// In en, this message translates to:
  /// **'The window could not be changed. Try again.'**
  String get theWindowCouldNotBeChangedTry;

  /// No description provided for @running.
  ///
  /// In en, this message translates to:
  /// **'Running'**
  String get running;

  /// No description provided for @stopping.
  ///
  /// In en, this message translates to:
  /// **'Stopping'**
  String get stopping;

  /// No description provided for @completed.
  ///
  /// In en, this message translates to:
  /// **'Completed'**
  String get completed;

  /// No description provided for @failed.
  ///
  /// In en, this message translates to:
  /// **'Failed'**
  String get failed;

  /// No description provided for @stopped.
  ///
  /// In en, this message translates to:
  /// **'Stopped'**
  String get stopped;

  /// No description provided for @noMatchingTasks.
  ///
  /// In en, this message translates to:
  /// **'No matching tasks'**
  String get noMatchingTasks;

  /// No description provided for @tasksKeepRunningWhenHidden.
  ///
  /// In en, this message translates to:
  /// **'Tasks keep running when hidden'**
  String get tasksKeepRunningWhenHidden;

  /// No description provided for @noRunningTasks.
  ///
  /// In en, this message translates to:
  /// **'No running tasks'**
  String get noRunningTasks;

  /// No description provided for @allTasksFinished.
  ///
  /// In en, this message translates to:
  /// **'All tasks finished'**
  String get allTasksFinished;

  /// No description provided for @launcher.
  ///
  /// In en, this message translates to:
  /// **'Launcher'**
  String get launcher;

  /// No description provided for @findATask.
  ///
  /// In en, this message translates to:
  /// **'Find a task…'**
  String get findATask;

  /// No description provided for @clearSearch.
  ///
  /// In en, this message translates to:
  /// **'Clear search'**
  String get clearSearch;

  /// No description provided for @tasks.
  ///
  /// In en, this message translates to:
  /// **'Tasks'**
  String get tasks;

  /// No description provided for @stoppingProgress.
  ///
  /// In en, this message translates to:
  /// **'Stopping…'**
  String get stoppingProgress;

  /// No description provided for @stop.
  ///
  /// In en, this message translates to:
  /// **'Stop'**
  String get stop;

  /// No description provided for @moreActions.
  ///
  /// In en, this message translates to:
  /// **'More actions'**
  String get moreActions;

  /// No description provided for @commandCopied.
  ///
  /// In en, this message translates to:
  /// **'Command copied'**
  String get commandCopied;

  /// No description provided for @commandCouldNotBeCopiedTryAgain.
  ///
  /// In en, this message translates to:
  /// **'Command could not be copied. Try again.'**
  String get commandCouldNotBeCopiedTryAgain;

  /// No description provided for @copyCommand.
  ///
  /// In en, this message translates to:
  /// **'Copy command'**
  String get copyCommand;

  /// No description provided for @removeResult.
  ///
  /// In en, this message translates to:
  /// **'Remove result'**
  String get removeResult;

  /// No description provided for @commandDetails.
  ///
  /// In en, this message translates to:
  /// **'Command details'**
  String get commandDetails;

  /// No description provided for @showingTheLatestOutput128KiBPer.
  ///
  /// In en, this message translates to:
  /// **'Showing the latest output (128 KiB per stream).'**
  String get showingTheLatestOutput128KiBPer;

  /// No description provided for @noTasksYet.
  ///
  /// In en, this message translates to:
  /// **'No tasks yet.'**
  String get noTasksYet;

  /// No description provided for @chooseAProjectBuildTestOrRun.
  ///
  /// In en, this message translates to:
  /// **'Choose a project Build, Test or Run action in the Launcher.'**
  String get chooseAProjectBuildTestOrRun;

  /// No description provided for @anywhereActionShortcut.
  ///
  /// In en, this message translates to:
  /// **'Anywhere action shortcut'**
  String get anywhereActionShortcut;

  /// No description provided for @notAvailableRightNow.
  ///
  /// In en, this message translates to:
  /// **'Not available right now'**
  String get notAvailableRightNow;

  /// No description provided for @searchThisFileType.
  ///
  /// In en, this message translates to:
  /// **'Search this file type · Tab / Shift+Tab'**
  String get searchThisFileType;

  /// No description provided for @selectAFileToPreview.
  ///
  /// In en, this message translates to:
  /// **'Select a file to preview'**
  String get selectAFileToPreview;

  /// No description provided for @findAppsAndActions.
  ///
  /// In en, this message translates to:
  /// **'Find apps and actions'**
  String get findAppsAndActions;

  /// No description provided for @actionsForYourCurrentEnvironment.
  ///
  /// In en, this message translates to:
  /// **'Actions for your current environment'**
  String get actionsForYourCurrentEnvironment;

  /// No description provided for @allActionsAvailableInTheCurrentApp.
  ///
  /// In en, this message translates to:
  /// **'All actions available in the current app'**
  String get allActionsAvailableInTheCurrentApp;

  /// No description provided for @copyWithReturnHint.
  ///
  /// In en, this message translates to:
  /// **'Copy · ↵'**
  String get copyWithReturnHint;

  /// No description provided for @deleteEntry.
  ///
  /// In en, this message translates to:
  /// **'Delete Entry'**
  String get deleteEntry;

  /// No description provided for @previewUnavailable.
  ///
  /// In en, this message translates to:
  /// **'Preview unavailable'**
  String get previewUnavailable;

  /// No description provided for @loadingPreview.
  ///
  /// In en, this message translates to:
  /// **'Loading preview…'**
  String get loadingPreview;

  /// No description provided for @selectEntryToPreview.
  ///
  /// In en, this message translates to:
  /// **'Select an entry to preview'**
  String get selectEntryToPreview;

  /// No description provided for @noMatchingEntries.
  ///
  /// In en, this message translates to:
  /// **'No matching entries'**
  String get noMatchingEntries;

  /// No description provided for @noSavedEntries.
  ///
  /// In en, this message translates to:
  /// **'No saved entries'**
  String get noSavedEntries;

  /// No description provided for @newlyCopiedItemsWillAppearHereRefresh.
  ///
  /// In en, this message translates to:
  /// **'Newly copied items will appear here.\nRefresh to see new copies.'**
  String get newlyCopiedItemsWillAppearHereRefresh;

  /// No description provided for @manageRecordingInSettingsClipboard.
  ///
  /// In en, this message translates to:
  /// **'Manage recording in Settings → Clipboard.'**
  String get manageRecordingInSettingsClipboard;

  /// No description provided for @imagePreviewUnavailable.
  ///
  /// In en, this message translates to:
  /// **'Image preview unavailable'**
  String get imagePreviewUnavailable;

  /// No description provided for @previewUnavailableTheOriginalImageCanStill.
  ///
  /// In en, this message translates to:
  /// **'Preview unavailable. The original image can still be copied.'**
  String get previewUnavailableTheOriginalImageCanStill;

  /// No description provided for @previewIsNotAvailableForThisFormat.
  ///
  /// In en, this message translates to:
  /// **'Preview is not available for this format. Copy restores its saved data.'**
  String get previewIsNotAvailableForThisFormat;

  /// No description provided for @theWindowCouldNotBeClosedUse.
  ///
  /// In en, this message translates to:
  /// **'The window could not be closed. Use the window close button.'**
  String get theWindowCouldNotBeClosedUse;

  /// No description provided for @backToListEsc.
  ///
  /// In en, this message translates to:
  /// **'Back to list · Esc'**
  String get backToListEsc;

  /// No description provided for @clipboardAlwaysOnTop.
  ///
  /// In en, this message translates to:
  /// **'Clipboard · Always on top'**
  String get clipboardAlwaysOnTop;

  /// No description provided for @searchClipboard.
  ///
  /// In en, this message translates to:
  /// **'Search clipboard…'**
  String get searchClipboard;

  /// No description provided for @noSavedEntriesManageRecordingInSettings.
  ///
  /// In en, this message translates to:
  /// **'No saved entries.\nManage recording in Settings → Clipboard.'**
  String get noSavedEntriesManageRecordingInSettings;

  /// No description provided for @actionFailed.
  ///
  /// In en, this message translates to:
  /// **'Action failed'**
  String get actionFailed;

  /// No description provided for @actionResult.
  ///
  /// In en, this message translates to:
  /// **'Action result'**
  String get actionResult;

  /// No description provided for @result.
  ///
  /// In en, this message translates to:
  /// **'Result'**
  String get result;

  /// No description provided for @scroll.
  ///
  /// In en, this message translates to:
  /// **'↑↓ Scroll'**
  String get scroll;

  /// No description provided for @tool.
  ///
  /// In en, this message translates to:
  /// **'Tool'**
  String get tool;

  /// No description provided for @closeEsc.
  ///
  /// In en, this message translates to:
  /// **'Close · Esc'**
  String get closeEsc;

  /// No description provided for @backEsc.
  ///
  /// In en, this message translates to:
  /// **'Back · Esc'**
  String get backEsc;

  /// No description provided for @reloadTool.
  ///
  /// In en, this message translates to:
  /// **'Reload tool'**
  String get reloadTool;

  /// No description provided for @backToSearchEsc.
  ///
  /// In en, this message translates to:
  /// **'Back to search · Esc'**
  String get backToSearchEsc;

  /// No description provided for @cancelEsc.
  ///
  /// In en, this message translates to:
  /// **'Cancel · Esc'**
  String get cancelEsc;

  /// No description provided for @cancelEscapeSelected.
  ///
  /// In en, this message translates to:
  /// **'✓ Cancel · Esc'**
  String get cancelEscapeSelected;

  /// No description provided for @allowOnce.
  ///
  /// In en, this message translates to:
  /// **'✓ Allow once'**
  String get allowOnce;

  /// No description provided for @allowOnceAction.
  ///
  /// In en, this message translates to:
  /// **'Allow once'**
  String get allowOnceAction;

  /// No description provided for @webToolsAreUnavailableOnThisPlatform.
  ///
  /// In en, this message translates to:
  /// **'Web tools are unavailable on this platform.'**
  String get webToolsAreUnavailableOnThisPlatform;

  /// No description provided for @requestedAccess.
  ///
  /// In en, this message translates to:
  /// **'Requested access'**
  String get requestedAccess;

  /// No description provided for @noHostPermissions.
  ///
  /// In en, this message translates to:
  /// **'No Host permissions.'**
  String get noHostPermissions;

  /// No description provided for @codeRunsInALocalWebViewNetwork.
  ///
  /// In en, this message translates to:
  /// **'Code runs in a local WebView. Network access and navigation are blocked. This is not an OS process sandbox.'**
  String get codeRunsInALocalWebViewNetwork;

  /// No description provided for @thisReplacesTheInstalledCodeClosesIts.
  ///
  /// In en, this message translates to:
  /// **'This replaces the installed code, closes its active page, and keeps its local settings. A previously approved version can be restored.'**
  String get thisReplacesTheInstalledCodeClosesIts;

  /// No description provided for @approveAndInstall.
  ///
  /// In en, this message translates to:
  /// **'Approve and install'**
  String get approveAndInstall;

  /// No description provided for @installedSearchForTheToolByName.
  ///
  /// In en, this message translates to:
  /// **'Installed. Search for the tool by name to open it.'**
  String get installedSearchForTheToolByName;

  /// No description provided for @thePackageCouldNotBeInstalledThe.
  ///
  /// In en, this message translates to:
  /// **'The package could not be installed. The existing version is unchanged.'**
  String get thePackageCouldNotBeInstalledThe;

  /// No description provided for @theOperationCouldNotFinishCheckThe.
  ///
  /// In en, this message translates to:
  /// **'The operation could not finish. Check the current tool status and try again.'**
  String get theOperationCouldNotFinishCheckThe;

  /// No description provided for @webTools.
  ///
  /// In en, this message translates to:
  /// **'Web tools'**
  String get webTools;

  /// No description provided for @import.
  ///
  /// In en, this message translates to:
  /// **'Import…'**
  String get import;

  /// No description provided for @noWebToolsInstalled.
  ///
  /// In en, this message translates to:
  /// **'No Web tools installed.'**
  String get noWebToolsInstalled;

  /// No description provided for @restoreApprovedVersion.
  ///
  /// In en, this message translates to:
  /// **'Restore approved version'**
  String get restoreApprovedVersion;

  /// No description provided for @uninstall.
  ///
  /// In en, this message translates to:
  /// **'Uninstall'**
  String get uninstall;

  /// No description provided for @theToolDidNotBecomeReadyGo.
  ///
  /// In en, this message translates to:
  /// **'The tool did not become ready. Go back and reopen it.'**
  String get theToolDidNotBecomeReadyGo;

  /// No description provided for @thisToolIsNoLongerAvailableReturn.
  ///
  /// In en, this message translates to:
  /// **'This tool is no longer available. Return to search and reopen it.'**
  String get thisToolIsNoLongerAvailableReturn;

  /// No description provided for @theToolCouldNotLoadReloadIt.
  ///
  /// In en, this message translates to:
  /// **'The tool could not load. Reload it or return to search.'**
  String get theToolCouldNotLoadReloadIt;

  /// No description provided for @openingTool.
  ///
  /// In en, this message translates to:
  /// **'Opening tool…'**
  String get openingTool;

  /// No description provided for @loadingTool.
  ///
  /// In en, this message translates to:
  /// **'Loading tool…'**
  String get loadingTool;

  /// No description provided for @theToolStoppedReloadItOrReturn.
  ///
  /// In en, this message translates to:
  /// **'The tool stopped. Reload it or return to search.'**
  String get theToolStoppedReloadItOrReturn;

  /// No description provided for @allowThisOperation.
  ///
  /// In en, this message translates to:
  /// **'Allow this operation?'**
  String get allowThisOperation;

  /// No description provided for @thisTool.
  ///
  /// In en, this message translates to:
  /// **'This tool'**
  String get thisTool;

  /// No description provided for @dismissMessage.
  ///
  /// In en, this message translates to:
  /// **'Dismiss message'**
  String get dismissMessage;

  /// No description provided for @returnKey.
  ///
  /// In en, this message translates to:
  /// **'Return'**
  String get returnKey;

  /// No description provided for @folder.
  ///
  /// In en, this message translates to:
  /// **'Folder'**
  String get folder;

  /// No description provided for @file.
  ///
  /// In en, this message translates to:
  /// **'File'**
  String get file;

  /// No description provided for @selectedFiles.
  ///
  /// In en, this message translates to:
  /// **'Selected files'**
  String get selectedFiles;

  /// No description provided for @webLink.
  ///
  /// In en, this message translates to:
  /// **'Web link'**
  String get webLink;

  /// No description provided for @gitRepository.
  ///
  /// In en, this message translates to:
  /// **'Git repository'**
  String get gitRepository;

  /// No description provided for @nodeProject.
  ///
  /// In en, this message translates to:
  /// **'Node project'**
  String get nodeProject;

  /// No description provided for @mavenProject.
  ///
  /// In en, this message translates to:
  /// **'Maven project'**
  String get mavenProject;

  /// No description provided for @gradleProject.
  ///
  /// In en, this message translates to:
  /// **'Gradle project'**
  String get gradleProject;

  /// No description provided for @rustProject.
  ///
  /// In en, this message translates to:
  /// **'Rust project'**
  String get rustProject;

  /// No description provided for @springProject.
  ///
  /// In en, this message translates to:
  /// **'Spring Boot project'**
  String get springProject;

  /// No description provided for @anywhereTool.
  ///
  /// In en, this message translates to:
  /// **'Anywhere tool'**
  String get anywhereTool;

  /// No description provided for @pdfDocument.
  ///
  /// In en, this message translates to:
  /// **'PDF document'**
  String get pdfDocument;

  /// No description provided for @document.
  ///
  /// In en, this message translates to:
  /// **'Document'**
  String get document;

  /// No description provided for @textFile.
  ///
  /// In en, this message translates to:
  /// **'Text file'**
  String get textFile;

  /// No description provided for @spreadsheet.
  ///
  /// In en, this message translates to:
  /// **'Spreadsheet'**
  String get spreadsheet;

  /// No description provided for @presentation.
  ///
  /// In en, this message translates to:
  /// **'Presentation'**
  String get presentation;

  /// No description provided for @archive.
  ///
  /// In en, this message translates to:
  /// **'Archive'**
  String get archive;

  /// No description provided for @sourceCode.
  ///
  /// In en, this message translates to:
  /// **'Source code'**
  String get sourceCode;

  /// No description provided for @configurationFile.
  ///
  /// In en, this message translates to:
  /// **'Configuration file'**
  String get configurationFile;

  /// No description provided for @ebook.
  ///
  /// In en, this message translates to:
  /// **'E-book'**
  String get ebook;

  /// No description provided for @font.
  ///
  /// In en, this message translates to:
  /// **'Font'**
  String get font;

  /// No description provided for @packageOrBinary.
  ///
  /// In en, this message translates to:
  /// **'Package or binary'**
  String get packageOrBinary;

  /// No description provided for @captureOff.
  ///
  /// In en, this message translates to:
  /// **'Off · applies on the next wake'**
  String get captureOff;

  /// No description provided for @captureNotRead.
  ///
  /// In en, this message translates to:
  /// **'Not captured in the last wake'**
  String get captureNotRead;

  /// No description provided for @captureReady.
  ///
  /// In en, this message translates to:
  /// **'Ready · last wake'**
  String get captureReady;

  /// No description provided for @noItemsSelected.
  ///
  /// In en, this message translates to:
  /// **'No items selected'**
  String get noItemsSelected;

  /// No description provided for @noTextSelected.
  ///
  /// In en, this message translates to:
  /// **'No text selected'**
  String get noTextSelected;

  /// No description provided for @noPageAvailable.
  ///
  /// In en, this message translates to:
  /// **'No web page available'**
  String get noPageAvailable;

  /// No description provided for @noTextCopied.
  ///
  /// In en, this message translates to:
  /// **'No text copied'**
  String get noTextCopied;

  /// No description provided for @permissionNeeded.
  ///
  /// In en, this message translates to:
  /// **'Permission needed'**
  String get permissionNeeded;

  /// No description provided for @sourceUnsupported.
  ///
  /// In en, this message translates to:
  /// **'Not supported by this app'**
  String get sourceUnsupported;

  /// No description provided for @captureAppChanged.
  ///
  /// In en, this message translates to:
  /// **'App changed during capture'**
  String get captureAppChanged;

  /// No description provided for @timedOut.
  ///
  /// In en, this message translates to:
  /// **'Timed out'**
  String get timedOut;

  /// No description provided for @contentTooLong.
  ///
  /// In en, this message translates to:
  /// **'Content too long'**
  String get contentTooLong;

  /// No description provided for @tooManyItems.
  ///
  /// In en, this message translates to:
  /// **'Too many selected items'**
  String get tooManyItems;

  /// No description provided for @sourceUnavailable.
  ///
  /// In en, this message translates to:
  /// **'Could not read this source'**
  String get sourceUnavailable;

  /// No description provided for @readFailed.
  ///
  /// In en, this message translates to:
  /// **'Read failed'**
  String get readFailed;

  /// No description provided for @finderPermissionHelp.
  ///
  /// In en, this message translates to:
  /// **'Allow Finder Automation in System Settings, then reopen Anywhere.'**
  String get finderPermissionHelp;

  /// No description provided for @selectedTextPermissionHelp.
  ///
  /// In en, this message translates to:
  /// **'Allow Accessibility in System Settings, then reopen Anywhere.'**
  String get selectedTextPermissionHelp;

  /// No description provided for @browserPermissionHelp.
  ///
  /// In en, this message translates to:
  /// **'Allow Anywhere to access this browser in System Settings → Privacy & Security → Automation, then reopen Anywhere.'**
  String get browserPermissionHelp;

  /// No description provided for @clipboardPermissionHelp.
  ///
  /// In en, this message translates to:
  /// **'Allow clipboard access in System Settings, then reopen Anywhere.'**
  String get clipboardPermissionHelp;

  /// No description provided for @appChangedHelp.
  ///
  /// In en, this message translates to:
  /// **'The original app exited or restarted. Reopen Anywhere to capture it again.'**
  String get appChangedHelp;

  /// No description provided for @tooManyItemsHelp.
  ///
  /// In en, this message translates to:
  /// **'Select up to 32 items in Finder, then reopen Anywhere.'**
  String get tooManyItemsHelp;

  /// No description provided for @loadingMoreFiles.
  ///
  /// In en, this message translates to:
  /// **'Loading more files…'**
  String get loadingMoreFiles;

  /// No description provided for @authorizeSearchDirectories.
  ///
  /// In en, this message translates to:
  /// **'Authorize search directories'**
  String get authorizeSearchDirectories;

  /// No description provided for @authorizeSearchHelp.
  ///
  /// In en, this message translates to:
  /// **'Open Settings → Search and authorize saved directories. No file search runs before access is prepared.'**
  String get authorizeSearchHelp;

  /// No description provided for @fileSearchOff.
  ///
  /// In en, this message translates to:
  /// **'File search is turned off'**
  String get fileSearchOff;

  /// No description provided for @searchUnavailable.
  ///
  /// In en, this message translates to:
  /// **'Search is unavailable'**
  String get searchUnavailable;

  /// No description provided for @partialResults.
  ///
  /// In en, this message translates to:
  /// **'Partial results'**
  String get partialResults;

  /// No description provided for @noMatchingFiles.
  ///
  /// In en, this message translates to:
  /// **'No matching files'**
  String get noMatchingFiles;

  /// No description provided for @noContentActions.
  ///
  /// In en, this message translates to:
  /// **'No actions for this content'**
  String get noContentActions;

  /// No description provided for @searchingFiles.
  ///
  /// In en, this message translates to:
  /// **'Searching files…'**
  String get searchingFiles;

  /// No description provided for @fileSearchUnavailable.
  ///
  /// In en, this message translates to:
  /// **'File search unavailable'**
  String get fileSearchUnavailable;

  /// No description provided for @fileSearchDisabledHelp.
  ///
  /// In en, this message translates to:
  /// **'File search is disabled. Enable it in Settings → Search.'**
  String get fileSearchDisabledHelp;

  /// No description provided for @typeFileName.
  ///
  /// In en, this message translates to:
  /// **'Type a file or folder name to search'**
  String get typeFileName;

  /// No description provided for @fileSearchIncompleteHelp.
  ///
  /// In en, this message translates to:
  /// **'File search is incomplete. Check the issue details and retry.'**
  String get fileSearchIncompleteHelp;

  /// No description provided for @noMatchingActions.
  ///
  /// In en, this message translates to:
  /// **'No matching actions'**
  String get noMatchingActions;

  /// No description provided for @noActionsAvailable.
  ///
  /// In en, this message translates to:
  /// **'No actions available'**
  String get noActionsAvailable;

  /// No description provided for @fileSearchIncomplete.
  ///
  /// In en, this message translates to:
  /// **'File search is incomplete. Available results are still usable.'**
  String get fileSearchIncomplete;

  /// No description provided for @actionsLoadIncomplete.
  ///
  /// In en, this message translates to:
  /// **'Some actions could not be loaded. Reopen Anywhere to try again; other actions are still available.'**
  String get actionsLoadIncomplete;

  /// No description provided for @fileSearchOtherResults.
  ///
  /// In en, this message translates to:
  /// **'File search unavailable · Other results are still usable'**
  String get fileSearchOtherResults;

  /// No description provided for @currentPage.
  ///
  /// In en, this message translates to:
  /// **'Current page'**
  String get currentPage;

  /// No description provided for @chooseTarget.
  ///
  /// In en, this message translates to:
  /// **'Choose a target'**
  String get chooseTarget;

  /// No description provided for @contentActions.
  ///
  /// In en, this message translates to:
  /// **'Actions for this content'**
  String get contentActions;

  /// No description provided for @searchComplete.
  ///
  /// In en, this message translates to:
  /// **'Search complete'**
  String get searchComplete;

  /// No description provided for @clipboardSettingsFailed.
  ///
  /// In en, this message translates to:
  /// **'Clipboard settings could not be loaded or updated. Try again.'**
  String get clipboardSettingsFailed;

  /// No description provided for @historyLoadFailed.
  ///
  /// In en, this message translates to:
  /// **'Clipboard history could not be loaded. Try Refresh.'**
  String get historyLoadFailed;

  /// No description provided for @historyEntryMissing.
  ///
  /// In en, this message translates to:
  /// **'This entry is no longer available. Refresh history.'**
  String get historyEntryMissing;

  /// No description provided for @historyPreviewFailed.
  ///
  /// In en, this message translates to:
  /// **'This entry could not be previewed.'**
  String get historyPreviewFailed;

  /// No description provided for @historyOperationFailed.
  ///
  /// In en, this message translates to:
  /// **'The operation could not be completed. Refresh and try again.'**
  String get historyOperationFailed;

  /// No description provided for @historyCopyFailed.
  ///
  /// In en, this message translates to:
  /// **'This entry could not be copied. For files, check that the originals still exist and are accessible.'**
  String get historyCopyFailed;

  /// No description provided for @sourceUnsupportedHelp.
  ///
  /// In en, this message translates to:
  /// **'{source} is not exposed by this app. Other sources still work.'**
  String sourceUnsupportedHelp(String source);

  /// No description provided for @sourceTimeoutHelp.
  ///
  /// In en, this message translates to:
  /// **'{source} took too long to respond. Reopen Anywhere to try again.'**
  String sourceTimeoutHelp(String source);

  /// No description provided for @sourceTooLongHelp.
  ///
  /// In en, this message translates to:
  /// **'{source} is too long to use. Use less content and reopen Anywhere.'**
  String sourceTooLongHelp(String source);

  /// No description provided for @sourceFailedHelp.
  ///
  /// In en, this message translates to:
  /// **'{source} could not be read. Reopen Anywhere to try again; other sources still work.'**
  String sourceFailedHelp(String source);

  /// No description provided for @fileFoundCount.
  ///
  /// In en, this message translates to:
  /// **'{count} found'**
  String fileFoundCount(int count);

  /// No description provided for @searchIssues.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =1{1 issue} other{{count} issues}}'**
  String searchIssues(int count);

  /// No description provided for @fileSearchIssueNotice.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =1{File search encountered 1 issue. Available results are still usable.} other{File search encountered {count} issues. Available results are still usable.}}'**
  String fileSearchIssueNotice(int count);

  /// No description provided for @filterActions.
  ///
  /// In en, this message translates to:
  /// **'Filter {scope} actions'**
  String filterActions(String scope);

  /// No description provided for @emptyFileType.
  ///
  /// In en, this message translates to:
  /// **'No {type} in these results. Try All types or another search.'**
  String emptyFileType(String type);

  /// No description provided for @fileRootsHint.
  ///
  /// In en, this message translates to:
  /// **'{example}\nLeave empty to search your home directory'**
  String fileRootsHint(String example);

  /// No description provided for @runningCount.
  ///
  /// In en, this message translates to:
  /// **'{count} running'**
  String runningCount(int count);

  /// No description provided for @exitCode.
  ///
  /// In en, this message translates to:
  /// **'Exit {code}'**
  String exitCode(String code);

  /// No description provided for @taskKeyboardHelp.
  ///
  /// In en, this message translates to:
  /// **'↑↓ Tasks   {find} Find   {stop} Stop   Esc Hide'**
  String taskKeyboardHelp(String find, String stop);

  /// No description provided for @shortcutHint.
  ///
  /// In en, this message translates to:
  /// **'{label} · {shortcut}'**
  String shortcutHint(String label, String shortcut);

  /// No description provided for @enableFeature.
  ///
  /// In en, this message translates to:
  /// **'Enable {name}'**
  String enableFeature(String name);

  /// No description provided for @allowFeature.
  ///
  /// In en, this message translates to:
  /// **'Allow {name}?'**
  String allowFeature(String name);

  /// No description provided for @removeRule.
  ///
  /// In en, this message translates to:
  /// **'Remove rule {number}'**
  String removeRule(int number);

  /// No description provided for @contextRulesCount.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =1{1 context rule} other{{count} context rules}}'**
  String contextRulesCount(int count);

  /// No description provided for @editActionShortcut.
  ///
  /// In en, this message translates to:
  /// **'Edit shortcut for {title}'**
  String editActionShortcut(String title);

  /// No description provided for @removeActionShortcut.
  ///
  /// In en, this message translates to:
  /// **'Remove shortcut for {title}'**
  String removeActionShortcut(String title);

  /// No description provided for @results.
  ///
  /// In en, this message translates to:
  /// **'Results'**
  String get results;

  /// No description provided for @succeeded.
  ///
  /// In en, this message translates to:
  /// **'Succeeded'**
  String get succeeded;

  /// No description provided for @input.
  ///
  /// In en, this message translates to:
  /// **'Input'**
  String get input;

  /// No description provided for @web.
  ///
  /// In en, this message translates to:
  /// **'Web'**
  String get web;

  /// No description provided for @tools.
  ///
  /// In en, this message translates to:
  /// **'Tools'**
  String get tools;

  /// No description provided for @restrictedLocationsSkipped.
  ///
  /// In en, this message translates to:
  /// **'{count} restricted locations skipped'**
  String restrictedLocationsSkipped(String count);

  /// No description provided for @clipboardConsent.
  ///
  /// In en, this message translates to:
  /// **'{tool} is requesting a clipboard operation. Approval applies once.'**
  String clipboardConsent(String tool);

  /// No description provided for @quitAnywhere.
  ///
  /// In en, this message translates to:
  /// **'Quit Anywhere'**
  String get quitAnywhere;

  /// No description provided for @edit.
  ///
  /// In en, this message translates to:
  /// **'Edit'**
  String get edit;

  /// No description provided for @undo.
  ///
  /// In en, this message translates to:
  /// **'Undo'**
  String get undo;

  /// No description provided for @redo.
  ///
  /// In en, this message translates to:
  /// **'Redo'**
  String get redo;

  /// No description provided for @cut.
  ///
  /// In en, this message translates to:
  /// **'Cut'**
  String get cut;

  /// No description provided for @paste.
  ///
  /// In en, this message translates to:
  /// **'Paste'**
  String get paste;

  /// No description provided for @selectAll.
  ///
  /// In en, this message translates to:
  /// **'Select All'**
  String get selectAll;

  /// No description provided for @window.
  ///
  /// In en, this message translates to:
  /// **'Window'**
  String get window;

  /// No description provided for @minimize.
  ///
  /// In en, this message translates to:
  /// **'Minimize'**
  String get minimize;

  /// No description provided for @zoom.
  ///
  /// In en, this message translates to:
  /// **'Zoom'**
  String get zoom;

  /// No description provided for @closeWindow.
  ///
  /// In en, this message translates to:
  /// **'Close Window'**
  String get closeWindow;

  /// No description provided for @findTaskMenu.
  ///
  /// In en, this message translates to:
  /// **'Find Task…'**
  String get findTaskMenu;

  /// No description provided for @stopSelectedTask.
  ///
  /// In en, this message translates to:
  /// **'Stop Selected Task'**
  String get stopSelectedTask;

  /// No description provided for @refreshTasks.
  ///
  /// In en, this message translates to:
  /// **'Refresh Tasks'**
  String get refreshTasks;

  /// No description provided for @openLauncher.
  ///
  /// In en, this message translates to:
  /// **'Open Launcher'**
  String get openLauncher;

  /// No description provided for @closeTaskWindow.
  ///
  /// In en, this message translates to:
  /// **'Close Task Window'**
  String get closeTaskWindow;

  /// No description provided for @runningTasksMenu.
  ///
  /// In en, this message translates to:
  /// **'Running Tasks…'**
  String get runningTasksMenu;

  /// No description provided for @settingsMenu.
  ///
  /// In en, this message translates to:
  /// **'Settings…'**
  String get settingsMenu;

  /// No description provided for @authorizeFileSearch.
  ///
  /// In en, this message translates to:
  /// **'Authorize File Search'**
  String get authorizeFileSearch;

  /// No description provided for @authorize.
  ///
  /// In en, this message translates to:
  /// **'Authorize'**
  String get authorize;

  /// No description provided for @importLocalWebTool.
  ///
  /// In en, this message translates to:
  /// **'Import a local Web tool'**
  String get importLocalWebTool;

  /// No description provided for @importPackage.
  ///
  /// In en, this message translates to:
  /// **'Import'**
  String get importPackage;

  /// No description provided for @fileSearchAccessMessage.
  ///
  /// In en, this message translates to:
  /// **'Choose folders Anywhere may search. Access is saved for future launches. Protected system or app data may still require Full Disk Access in System Settings.'**
  String get fileSearchAccessMessage;

  /// No description provided for @inspectTrustedPackage.
  ///
  /// In en, this message translates to:
  /// **'Choose a trusted .awplugin package to inspect before installing.'**
  String get inspectTrustedPackage;

  /// No description provided for @readClipboardText.
  ///
  /// In en, this message translates to:
  /// **'Read clipboard text'**
  String get readClipboardText;

  /// No description provided for @copyTextToClipboard.
  ///
  /// In en, this message translates to:
  /// **'Copy text to clipboard'**
  String get copyTextToClipboard;

  /// No description provided for @permissionCurrentApp.
  ///
  /// In en, this message translates to:
  /// **'Current application name'**
  String get permissionCurrentApp;

  /// No description provided for @permissionReadClipboard.
  ///
  /// In en, this message translates to:
  /// **'Read clipboard text (confirmed per operation)'**
  String get permissionReadClipboard;

  /// No description provided for @permissionWriteClipboard.
  ///
  /// In en, this message translates to:
  /// **'Copy text (confirmed per operation)'**
  String get permissionWriteClipboard;

  /// No description provided for @permissionLocalStorage.
  ///
  /// In en, this message translates to:
  /// **'Private local settings (1 MiB)'**
  String get permissionLocalStorage;

  /// No description provided for @launcherStartupFailed.
  ///
  /// In en, this message translates to:
  /// **'Launcher could not start'**
  String get launcherStartupFailed;

  /// No description provided for @tasksStartupFailed.
  ///
  /// In en, this message translates to:
  /// **'Running Tasks could not open'**
  String get tasksStartupFailed;

  /// No description provided for @shelfStartupFailed.
  ///
  /// In en, this message translates to:
  /// **'Clipboard Shelf could not open'**
  String get shelfStartupFailed;

  /// No description provided for @savedDirectories.
  ///
  /// In en, this message translates to:
  /// **'Saved Directories'**
  String get savedDirectories;

  /// No description provided for @browserPageDescription.
  ///
  /// In en, this message translates to:
  /// **'Use the active Safari, Chrome, or Edge tab’s web address'**
  String get browserPageDescription;

  /// No description provided for @permissionContextRead.
  ///
  /// In en, this message translates to:
  /// **'Selected context: {types}'**
  String permissionContextRead(String types);

  /// No description provided for @supportedBrowserDescription.
  ///
  /// In en, this message translates to:
  /// **'Active URL in a supported browser'**
  String get supportedBrowserDescription;

  /// No description provided for @installToolReview.
  ///
  /// In en, this message translates to:
  /// **'Install {name}?'**
  String installToolReview(String name);

  /// No description provided for @updateToolReview.
  ///
  /// In en, this message translates to:
  /// **'Update {name}?'**
  String updateToolReview(String name);

  /// No description provided for @paused.
  ///
  /// In en, this message translates to:
  /// **'Paused'**
  String get paused;

  /// No description provided for @detail.
  ///
  /// In en, this message translates to:
  /// **'Detail'**
  String get detail;

  /// No description provided for @formatsCount.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =1{1 format} other{{count} formats}}'**
  String formatsCount(int count);

  /// No description provided for @automationUsage.
  ///
  /// In en, this message translates to:
  /// **'Read the current Finder location and selection, and the active Safari, Chrome, or Edge tab URL to suggest actions when you open Anywhere.'**
  String get automationUsage;

  /// No description provided for @contentPreview.
  ///
  /// In en, this message translates to:
  /// **'{kind} preview'**
  String contentPreview(String kind);

  /// No description provided for @savedSize.
  ///
  /// In en, this message translates to:
  /// **'{size} saved'**
  String savedSize(String size);

  /// No description provided for @invalidActionIdentity.
  ///
  /// In en, this message translates to:
  /// **'Invalid action identity.'**
  String get invalidActionIdentity;

  /// No description provided for @letterWithModifier.
  ///
  /// In en, this message translates to:
  /// **'Use a letter key with a modifier.'**
  String get letterWithModifier;

  /// No description provided for @modifierWithLetter.
  ///
  /// In en, this message translates to:
  /// **'Use a modifier with the letter key.'**
  String get modifierWithLetter;

  /// No description provided for @reservedEditingShortcut.
  ///
  /// In en, this message translates to:
  /// **'This shortcut is reserved for editing or window commands.'**
  String get reservedEditingShortcut;

  /// No description provided for @invalidShortcutIntent.
  ///
  /// In en, this message translates to:
  /// **'Invalid shortcut intent identity.'**
  String get invalidShortcutIntent;

  /// No description provided for @shortcutRuleLimit.
  ///
  /// In en, this message translates to:
  /// **'Name the shortcut and use at most 16 rules.'**
  String get shortcutRuleLimit;

  /// No description provided for @differentShortcutConditions.
  ///
  /// In en, this message translates to:
  /// **'Rules must use different app and context conditions.'**
  String get differentShortcutConditions;

  /// No description provided for @shortcutAlreadyOpens.
  ///
  /// In en, this message translates to:
  /// **'This shortcut already opens Anywhere.'**
  String get shortcutAlreadyOpens;

  /// No description provided for @invalidShortcutRule.
  ///
  /// In en, this message translates to:
  /// **'Invalid shortcut rule.'**
  String get invalidShortcutRule;

  /// No description provided for @searchDirectoryLimits.
  ///
  /// In en, this message translates to:
  /// **'Use at most 8 directories and 32 exclusions.'**
  String get searchDirectoryLimits;

  /// No description provided for @absoluteSearchDirectories.
  ///
  /// In en, this message translates to:
  /// **'Search directories must be absolute paths, one per line.'**
  String get absoluteSearchDirectories;

  /// No description provided for @unsupportedShortcutSettings.
  ///
  /// In en, this message translates to:
  /// **'Unsupported action shortcut settings version. Update Anywhere before changing settings.'**
  String get unsupportedShortcutSettings;

  /// No description provided for @oneShortcutPerAction.
  ///
  /// In en, this message translates to:
  /// **'An action can only have one shortcut.'**
  String get oneShortcutPerAction;

  /// No description provided for @shortcutsRefreshFailed.
  ///
  /// In en, this message translates to:
  /// **'Action shortcuts could not be refreshed. Reopen Settings to try again.'**
  String get shortcutsRefreshFailed;

  /// No description provided for @shortcutAssignedTo.
  ///
  /// In en, this message translates to:
  /// **'This shortcut is assigned to {title}.'**
  String shortcutAssignedTo(String title);

  /// No description provided for @maxActionShortcuts.
  ///
  /// In en, this message translates to:
  /// **'At most {count} action shortcuts are supported.'**
  String maxActionShortcuts(int count);

  /// No description provided for @shortcutsSavedByNewerVersion.
  ///
  /// In en, this message translates to:
  /// **'Action shortcuts were saved by a newer version. Update Anywhere before changing settings.'**
  String get shortcutsSavedByNewerVersion;

  /// No description provided for @directoryAccessFailed.
  ///
  /// In en, this message translates to:
  /// **'Directory access could not be prepared. Try again in Search settings.'**
  String get directoryAccessFailed;

  /// No description provided for @openSystemSettingsFailed.
  ///
  /// In en, this message translates to:
  /// **'System Settings could not be opened. Open the system settings manually.'**
  String get openSystemSettingsFailed;

  /// No description provided for @shortcutsRestoreFailed.
  ///
  /// In en, this message translates to:
  /// **'Action shortcuts could not be restored. Change the conflicting shortcut or try closing again.'**
  String get shortcutsRestoreFailed;

  /// No description provided for @extensionsLoadFailed.
  ///
  /// In en, this message translates to:
  /// **'Extensions could not be loaded: {detail}'**
  String extensionsLoadFailed(String detail);

  /// No description provided for @extensionsUpdateFailed.
  ///
  /// In en, this message translates to:
  /// **'Extension settings could not be updated: {detail}'**
  String extensionsUpdateFailed(String detail);

  /// No description provided for @shortcutLoadFailed.
  ///
  /// In en, this message translates to:
  /// **'Action shortcuts could not be loaded: {detail}'**
  String shortcutLoadFailed(String detail);

  /// No description provided for @shortcutsPausedHelp.
  ///
  /// In en, this message translates to:
  /// **'Some action shortcuts are paused. Reopen Settings to try again, or choose a different key combination.'**
  String get shortcutsPausedHelp;

  /// No description provided for @shelfSelectHint.
  ///
  /// In en, this message translates to:
  /// **'Single-click selects · Double-click copies'**
  String get shelfSelectHint;

  /// No description provided for @shelfCopyAndReturn.
  ///
  /// In en, this message translates to:
  /// **'Copy and return to app'**
  String get shelfCopyAndReturn;

  /// No description provided for @shelfDeleteTitle.
  ///
  /// In en, this message translates to:
  /// **'Remove this entry?'**
  String get shelfDeleteTitle;

  /// No description provided for @shelfDeleteDescription.
  ///
  /// In en, this message translates to:
  /// **'Only this saved history entry will be removed.'**
  String get shelfDeleteDescription;

  /// No description provided for @hidePreview.
  ///
  /// In en, this message translates to:
  /// **'Hide preview'**
  String get hidePreview;

  /// No description provided for @shelfPreviewScrollHint.
  ///
  /// In en, this message translates to:
  /// **'{shortcut} Scroll preview'**
  String shelfPreviewScrollHint(String shortcut);

  /// No description provided for @details.
  ///
  /// In en, this message translates to:
  /// **'Details'**
  String get details;

  /// No description provided for @sourceStatus.
  ///
  /// In en, this message translates to:
  /// **'Source status'**
  String get sourceStatus;

  /// No description provided for @searchRules.
  ///
  /// In en, this message translates to:
  /// **'Search rules'**
  String get searchRules;

  /// No description provided for @searchHelp.
  ///
  /// In en, this message translates to:
  /// **'How search works'**
  String get searchHelp;

  /// No description provided for @toolOptions.
  ///
  /// In en, this message translates to:
  /// **'Tool options'**
  String get toolOptions;

  /// No description provided for @keyboardHelp.
  ///
  /// In en, this message translates to:
  /// **'Keyboard shortcuts'**
  String get keyboardHelp;

  /// No description provided for @fileInformation.
  ///
  /// In en, this message translates to:
  /// **'File information'**
  String get fileInformation;

  /// No description provided for @historyFeatureDisabled.
  ///
  /// In en, this message translates to:
  /// **'Clipboard History is disabled in Features.'**
  String get historyFeatureDisabled;

  /// No description provided for @historyStorageUnavailable.
  ///
  /// In en, this message translates to:
  /// **'Clipboard history storage is unavailable. Restart after repairing the data file.'**
  String get historyStorageUnavailable;

  /// No description provided for @historyAccessRequired.
  ///
  /// In en, this message translates to:
  /// **'Allow clipboard access in system settings to record new copies. Background capture will not prompt.'**
  String get historyAccessRequired;

  /// No description provided for @historyItemTooLarge.
  ///
  /// In en, this message translates to:
  /// **'The copied item exceeds the 16 MiB history limit and was not saved.'**
  String get historyItemTooLarge;

  /// No description provided for @historyReadFailed.
  ///
  /// In en, this message translates to:
  /// **'A clipboard item could not be read. Existing history is still available.'**
  String get historyReadFailed;

  /// No description provided for @historyCaptureUnavailable.
  ///
  /// In en, this message translates to:
  /// **'Clipboard history capture is unavailable. Existing history is still available.'**
  String get historyCaptureUnavailable;

  /// No description provided for @historyMonitoringUnavailable.
  ///
  /// In en, this message translates to:
  /// **'Clipboard monitoring is unavailable on this platform.'**
  String get historyMonitoringUnavailable;

  /// No description provided for @filePreviewChanged.
  ///
  /// In en, this message translates to:
  /// **'Preview unavailable. Search again if the file has changed.'**
  String get filePreviewChanged;

  /// No description provided for @filePreviewUnsupported.
  ///
  /// In en, this message translates to:
  /// **'Text preview is unavailable for this file type'**
  String get filePreviewUnsupported;

  /// No description provided for @filePreviewTruncated.
  ///
  /// In en, this message translates to:
  /// **'Showing the first {size}'**
  String filePreviewTruncated(String size);

  /// No description provided for @filePreviewEncodingGuessed.
  ///
  /// In en, this message translates to:
  /// **'Encoding guessed as {encoding}'**
  String filePreviewEncodingGuessed(String encoding);

  /// No description provided for @filePreviewDecoded.
  ///
  /// In en, this message translates to:
  /// **'Decoded as {encoding}'**
  String filePreviewDecoded(String encoding);

  /// No description provided for @shelfReturnFailed.
  ///
  /// In en, this message translates to:
  /// **'Copied. Could not return to the app. Switch to it to paste.'**
  String get shelfReturnFailed;

  /// No description provided for @shelfReturnUnavailable.
  ///
  /// In en, this message translates to:
  /// **'Copied. The destination app is no longer available. Switch to an app to paste.'**
  String get shelfReturnUnavailable;

  /// No description provided for @historyRecordingPaused.
  ///
  /// In en, this message translates to:
  /// **'History recording is paused. Existing history is still available.'**
  String get historyRecordingPaused;

  /// No description provided for @pauseHistoryRecording.
  ///
  /// In en, this message translates to:
  /// **'Pause without clearing saved history'**
  String get pauseHistoryRecording;

  /// No description provided for @historyPausedUntil.
  ///
  /// In en, this message translates to:
  /// **'Paused until {time}'**
  String historyPausedUntil(String time);

  /// No description provided for @resumeRecording.
  ///
  /// In en, this message translates to:
  /// **'Resume'**
  String get resumeRecording;

  /// No description provided for @pauseFor.
  ///
  /// In en, this message translates to:
  /// **'Pause…'**
  String get pauseFor;

  /// No description provided for @pauseMinutes.
  ///
  /// In en, this message translates to:
  /// **'{minutes} minutes'**
  String pauseMinutes(int minutes);

  /// No description provided for @keepHistoryFor.
  ///
  /// In en, this message translates to:
  /// **'Keep history for'**
  String get keepHistoryFor;

  /// No description provided for @untilStorageLimit.
  ///
  /// In en, this message translates to:
  /// **'Until storage limit'**
  String get untilStorageLimit;

  /// No description provided for @retentionDays.
  ///
  /// In en, this message translates to:
  /// **'{days, plural, =1{1 day} other{{days} days}}'**
  String retentionDays(int days);

  /// No description provided for @changeHistoryRetention.
  ///
  /// In en, this message translates to:
  /// **'Change history retention?'**
  String get changeHistoryRetention;

  /// No description provided for @historyRetentionWarning.
  ///
  /// In en, this message translates to:
  /// **'Entries older than the selected period will be permanently deleted now. Extending the period later cannot restore them.'**
  String get historyRetentionWarning;

  /// No description provided for @historyStorageUsage.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =1{1 entry} other{{count} entries}} · {mib} MiB stored'**
  String historyStorageUsage(int count, String mib);

  /// No description provided for @historyStorageExplanation.
  ///
  /// In en, this message translates to:
  /// **'Stored only on this Mac, without encryption. Up to 1,000 entries / 128 MiB of logical data; database file size may differ. Expiry deletes all saved formats, not the system clipboard or original files.'**
  String get historyStorageExplanation;

  /// No description provided for @appHistory.
  ///
  /// In en, this message translates to:
  /// **'App History'**
  String get appHistory;

  /// No description provided for @appHistorySearch.
  ///
  /// In en, this message translates to:
  /// **'Search recent projects and files…'**
  String get appHistorySearch;

  /// No description provided for @appHistorySources.
  ///
  /// In en, this message translates to:
  /// **'History sources'**
  String get appHistorySources;

  /// No description provided for @appHistorySourceDescription.
  ///
  /// In en, this message translates to:
  /// **'Finds IDEA and VS Code history files automatically. You can use a custom path or turn a source off. History is read only when you use its action.'**
  String get appHistorySourceDescription;

  /// No description provided for @appHistorySourcePath.
  ///
  /// In en, this message translates to:
  /// **'History metadata file path'**
  String get appHistorySourcePath;

  /// No description provided for @appHistoryUnconfigured.
  ///
  /// In en, this message translates to:
  /// **'No history file is available. Check automatic detection or set a custom path in Settings → App History.'**
  String get appHistoryUnconfigured;

  /// No description provided for @appHistoryReadFailed.
  ///
  /// In en, this message translates to:
  /// **'Some sources could not be read. Check their files in Settings → App History.'**
  String get appHistoryReadFailed;

  /// No description provided for @appHistoryDisabled.
  ///
  /// In en, this message translates to:
  /// **'App History is disabled. Enable it in Settings → Features.'**
  String get appHistoryDisabled;

  /// No description provided for @appHistoryOperationFailed.
  ///
  /// In en, this message translates to:
  /// **'The history operation could not be completed. Refresh the list and try again.'**
  String get appHistoryOperationFailed;

  /// No description provided for @appHistorySettingsFailed.
  ///
  /// In en, this message translates to:
  /// **'History sources could not be loaded or saved. Check the file path and try again.'**
  String get appHistorySettingsFailed;

  /// No description provided for @appHistoryCopyTarget.
  ///
  /// In en, this message translates to:
  /// **'Copy path or URI'**
  String get appHistoryCopyTarget;

  /// No description provided for @appHistoryAutomatic.
  ///
  /// In en, this message translates to:
  /// **'Using an automatically detected file'**
  String get appHistoryAutomatic;

  /// No description provided for @appHistoryCustom.
  ///
  /// In en, this message translates to:
  /// **'Using your custom file path'**
  String get appHistoryCustom;

  /// No description provided for @appHistorySourceOff.
  ///
  /// In en, this message translates to:
  /// **'Source turned off'**
  String get appHistorySourceOff;

  /// No description provided for @appHistoryNotDetected.
  ///
  /// In en, this message translates to:
  /// **'No file found automatically. You can set a custom path.'**
  String get appHistoryNotDetected;

  /// No description provided for @appHistoryUseAutomatic.
  ///
  /// In en, this message translates to:
  /// **'Use automatic detection'**
  String get appHistoryUseAutomatic;

  /// No description provided for @appHistoryDetectAgain.
  ///
  /// In en, this message translates to:
  /// **'Detect again'**
  String get appHistoryDetectAgain;

  /// No description provided for @appHistorySaveCustom.
  ///
  /// In en, this message translates to:
  /// **'Save custom path'**
  String get appHistorySaveCustom;

  /// No description provided for @appHistoryActionEnabled.
  ///
  /// In en, this message translates to:
  /// **'Show action'**
  String get appHistoryActionEnabled;

  /// No description provided for @appHistoryCustomAdapters.
  ///
  /// In en, this message translates to:
  /// **'Custom adapters'**
  String get appHistoryCustomAdapters;

  /// No description provided for @appHistoryCustomAdaptersHelp.
  ///
  /// In en, this message translates to:
  /// **'Place source.json and transform.js in a subfolder here, then restart Anywhere. Matching IDs override built-ins. Import only trusted local configuration; transforms run when you use their action.'**
  String get appHistoryCustomAdaptersHelp;

  /// No description provided for @appHistoryOpenAdapters.
  ///
  /// In en, this message translates to:
  /// **'Open adapters folder'**
  String get appHistoryOpenAdapters;
}

class _AppLocalizationsDelegate extends LocalizationsDelegate<AppLocalizations> {
  const _AppLocalizationsDelegate();

  @override
  Future<AppLocalizations> load(Locale locale) {
    return SynchronousFuture<AppLocalizations>(lookupAppLocalizations(locale));
  }

  @override
  bool isSupported(Locale locale) => <String>['en', 'zh'].contains(locale.languageCode);

  @override
  bool shouldReload(_AppLocalizationsDelegate old) => false;
}

AppLocalizations lookupAppLocalizations(Locale locale) {


  // Lookup logic when only language code is specified.
  switch (locale.languageCode) {
    case 'en': return AppLocalizationsEn();
    case 'zh': return AppLocalizationsZh();
  }

  throw FlutterError(
    'AppLocalizations.delegate failed to load unsupported locale "$locale". This is likely '
    'an issue with the localizations generation tool. Please file an issue '
    'on GitHub with a reproducible sample app and the gen-l10n configuration '
    'that was used.'
  );
}
