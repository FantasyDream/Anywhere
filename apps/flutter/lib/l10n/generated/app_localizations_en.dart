// ignore: unused_import
import 'package:intl/intl.dart' as intl;
import 'app_localizations.dart';

// ignore_for_file: type=lint

/// The translations for English (`en`).
class AppLocalizationsEn extends AppLocalizations {
  AppLocalizationsEn([String locale = 'en']) : super(locale);

  @override
  String get settings => 'Settings';

  @override
  String get general => 'General';

  @override
  String get shortcuts => 'Shortcuts';

  @override
  String get context => 'Context';

  @override
  String get search => 'Search';

  @override
  String get clipboard => 'Clipboard';

  @override
  String get features => 'Features';

  @override
  String get language => 'Language';

  @override
  String get languageDescription => 'Choose the language used throughout Anywhere';

  @override
  String get languageSystem => 'System default';

  @override
  String get languageEnglish => 'English';

  @override
  String get languageChinese => '简体中文';

  @override
  String get done => 'Done';

  @override
  String get saving => 'Saving…';

  @override
  String get openAtLogin => 'Open at login';

  @override
  String get openAtLoginDescription => 'Start quietly when you sign in';

  @override
  String get currentApp => 'Current App';

  @override
  String get globalSearch => 'Global Search';

  @override
  String get universalSearch => 'Universal Search';

  @override
  String get navigationModes => 'Global Search / Current App';

  @override
  String get backToResults => 'Back to results';

  @override
  String get back => 'Back';

  @override
  String get run => 'Run';

  @override
  String get open => 'Open';

  @override
  String get copy => 'Copy';

  @override
  String get actions => 'Actions';

  @override
  String get navigate => 'Navigate';

  @override
  String get retry => 'Retry';

  @override
  String get cancel => 'Cancel';

  @override
  String get close => 'Close';

  @override
  String get clear => 'Clear';

  @override
  String get remove => 'Remove';

  @override
  String get delete => 'Delete';

  @override
  String get preview => 'Preview';

  @override
  String get kind => 'Kind';

  @override
  String get size => 'Size';

  @override
  String get modified => 'Modified';

  @override
  String get location => 'Location';

  @override
  String get allTypes => 'All types';

  @override
  String get files => 'Files';

  @override
  String get folders => 'Folders';

  @override
  String get documents => 'Documents';

  @override
  String get pdf => 'PDF';

  @override
  String get images => 'Images';

  @override
  String get audio => 'Audio';

  @override
  String get video => 'Video';

  @override
  String get archives => 'Archives';

  @override
  String get code => 'Code';

  @override
  String get fileSearch => 'File Search';

  @override
  String get clipboardHistory => 'Clipboard History';

  @override
  String get clipboardShelf => 'Clipboard Shelf';

  @override
  String get runningTasks => 'Running Tasks';

  @override
  String get searchAppsActions => 'Search apps and actions…';

  @override
  String get searchFilesFolders => 'Search files and folders…';

  @override
  String get searchHistory => 'Search clipboard history…';

  @override
  String get findFilesFolders => 'Find files and folders';

  @override
  String get noResults => 'No results';

  @override
  String get noMatches => 'No matches';

  @override
  String get noHistory => 'No clipboard history';

  @override
  String get copied => 'Copied';

  @override
  String get copiedPaste => 'Copied. Paste in the destination app.';

  @override
  String get copyFailed => 'Could not copy. Try again.';

  @override
  String get loading => 'Loading…';

  @override
  String get searching => 'Searching…';

  @override
  String get suggested => 'Suggested';

  @override
  String get menus => 'Menus';

  @override
  String get browseMenus => 'Browse Menus…';

  @override
  String get contextSources => 'CONTEXT SOURCES';

  @override
  String get contextDescription => 'Choose what Anywhere can use when you open it.';

  @override
  String get finderSelection => 'Finder selection';

  @override
  String get fileSelection => 'File selection';

  @override
  String get selectedFilesDescription => 'Selected files and folders';

  @override
  String get selectedText => 'Selected text';

  @override
  String get selectedTextDescription => 'Text selected in the current app';

  @override
  String get browserCurrentPage => 'Browser current page';

  @override
  String get clipboardText => 'Clipboard text';

  @override
  String get clipboardAgeDescription => 'Auto-fill new copies for 15 seconds';

  @override
  String get openSettings => 'Open Settings';

  @override
  String get keepEditing => 'Keep editing';

  @override
  String get closeWithoutSaving => 'Close without saving';

  @override
  String get saveChanges => 'Save changes';

  @override
  String get saveBeforeClose => 'Save changes before closing?';

  @override
  String get changesNotSaved => 'Some changes were not saved';

  @override
  String get searchDrafts => 'Search settings have unapplied changes.';

  @override
  String get settingsSaveFailed => 'Settings could not be saved. Try again.';

  @override
  String get shortcutHelp => 'Choose a shortcut, then press the new key combination. Esc cancels.';

  @override
  String get pressShortcut => 'Press shortcut…';

  @override
  String get workspace => 'Workspace';

  @override
  String get localProcessing => 'Local processing';

  @override
  String get webTool => 'Web tool';

  @override
  String get reload => 'Reload';

  @override
  String get image => 'Image';

  @override
  String get richText => 'Rich text';

  @override
  String get data => 'Data';

  @override
  String get text => 'Text';

  @override
  String get pin => 'Pin';

  @override
  String get unpin => 'Unpin';

  @override
  String get copiedContent => 'Copied content';

  @override
  String resultsCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count results',
      one: '1 result',
      zero: '0 results',
    );
    return '$_temp0';
  }

  @override
  String entriesCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count entries',
      one: '1 entry',
      zero: '0 entries',
    );
    return '$_temp0';
  }

  @override
  String searchAppActions(String app) {
    return 'Search $app actions…';
  }

  @override
  String contentEscapeHint(String source) {
    return '$source · Esc → Current App';
  }

  @override
  String fileRange(String range) {
    return 'File Search · $range';
  }

  @override
  String get editCapturedContent => 'Edit captured content…';

  @override
  String fileLocations(int count) {
    return 'File Search · $count locations';
  }
}
