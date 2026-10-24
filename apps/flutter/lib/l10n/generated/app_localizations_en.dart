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
  String get workspace => 'Workspace';

  @override
  String get image => 'Image';

  @override
  String get richText => 'Rich text';

  @override
  String get data => 'Data';

  @override
  String get text => 'Text';

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

  @override
  String get actionShortcuts => 'Action shortcuts';

  @override
  String get bindAKeyToAnActionOr => 'Bind a key to an action, or choose actions by context.';

  @override
  String get globalWorksFromOtherApps => 'Global · works from other apps';

  @override
  String get whileAnywhereIsOpen => 'While Anywhere is open';

  @override
  String get actionUnavailableShortcutPaused => 'Action unavailable · shortcut paused';

  @override
  String get actionsUnavailableShortcutPaused => 'Actions unavailable · shortcut paused';

  @override
  String get someRulesAreUnavailable => 'Some rules are unavailable';

  @override
  String get notDetectedInTheLastContext => 'Not detected in the last context';

  @override
  String get addActionShortcut => 'Add action shortcut';

  @override
  String get addContextShortcut => 'Add context shortcut';

  @override
  String get toolsSuchAsFileSearchAreAlways => 'Tools such as File Search are always available to bind. Project actions appear when their project is active.';

  @override
  String get actionShortcut => 'Action shortcut';

  @override
  String get findAnAction => 'Find an action…';

  @override
  String get notDetectedInTheLastContextThe => 'Not detected in the last context. The saved action is retained.';

  @override
  String get shortcutRecordingFailedTryAgain => 'Shortcut recording failed. Try again.';

  @override
  String get pressAShortcut => 'Press a shortcut…';

  @override
  String get recordShortcut => 'Record shortcut';

  @override
  String get useCommandOptionOrControlWithA => 'Use Command, Option or Control with a letter. Esc cancels recording.';

  @override
  String get globalShortcut => 'Global shortcut';

  @override
  String get worksFromOtherAppsUsesFreshContext => 'Works from other apps. Uses fresh context when needed.';

  @override
  String get chooseAnotherAction => 'Choose another action';

  @override
  String get saveShortcut => 'Save shortcut';

  @override
  String get clearClipboardHistory => 'Clear clipboard history?';

  @override
  String get allSavedEntriesWillBePermanentlyRemoved => 'All saved entries will be permanently removed. The current system clipboard is unchanged.';

  @override
  String get clearHistory => 'Clear History';

  @override
  String get clipboardHistorySetting => 'Clipboard history';

  @override
  String get recordClipboardHistory => 'Record clipboard history';

  @override
  String get saveFutureCopiesWhileAnywhereIsRunning => 'Save future copies while Anywhere is running. Existing clipboard contents are not imported.';

  @override
  String get textImagesRichTextAndFileReferences => 'Text, images, rich text and file references are saved locally. Pausing keeps existing history.';

  @override
  String get tryAgain => 'Try Again';

  @override
  String get whenOpeningLauncher => 'When opening Launcher';

  @override
  String get autoFillSupportedContentCopiedWithinThe => 'Auto-fill supported content copied within the last 15 seconds. Independent of history recording.';

  @override
  String get historyStorage => 'History storage';

  @override
  String get removeAllSavedEntriesThisDoesNot => 'Remove all saved entries. This does not change recording or the current system clipboard.';

  @override
  String get clearHistoryAction => 'Clear History…';

  @override
  String get enabled => 'Enabled';

  @override
  String get disabled => 'Disabled';

  @override
  String get awaitingPermission => 'Awaiting permission';

  @override
  String get couldNotStart => 'Could not start';

  @override
  String get unavailable => 'Unavailable';

  @override
  String get restartRequired => 'Restart required';

  @override
  String get updating => 'Updating…';

  @override
  String get refresh => 'Refresh';

  @override
  String get chooseTheToolsAvailableInAnywhere => 'Choose the tools available in Anywhere.';

  @override
  String get builtIn => 'Built in';

  @override
  String get installedPlugins => 'Installed plugins';

  @override
  String get legacyV1PermissionsCannotBeRevokedIndividually => 'Legacy v1 permissions cannot be revoked individually. Disable to stop its actions.';

  @override
  String get reviewPermissions => 'Review permissions…';

  @override
  String get revokePermissions => 'Revoke permissions';

  @override
  String get thisExtensionRequestsPermissionTo => 'This extension requests permission to:';

  @override
  String get noHostPermissionsRequested => 'No host permissions requested.';

  @override
  String get approvalAppliesToThisInspectedDefinitionRestart => 'Approval applies to this inspected definition. Restart Anywhere to activate a newly approved plugin. Revoking permission takes effect immediately.';

  @override
  String get allowForNextLaunch => 'Allow for next launch';

  @override
  String get showAPluginViewInAnywhere => 'Show a plugin view in Anywhere';

  @override
  String get writeToTheClipboard => 'Write to the clipboard';

  @override
  String get openWebAddresses => 'Open web addresses';

  @override
  String get openFiles => 'Open files';

  @override
  String get readRepositoryInformation => 'Read repository information';

  @override
  String get runProjectTasks => 'Run project tasks';

  @override
  String get saveExtensionData => 'Save extension data';

  @override
  String get fileSearchSetting => 'File search';

  @override
  String get searchFileNamesInTheFileSearch => 'Search file names in the File Search action';

  @override
  String get systemSearchShowsEarlyResultsFilenameScanning => 'System search shows early results. Filename scanning uses accessible, authorized directories.';

  @override
  String get includeHiddenFiles => 'Include hidden files';

  @override
  String get includeDotfilesAndFilesInHiddenDirectories => 'Include dotfiles and files in hidden directories';

  @override
  String get searchDirectories => 'Search directories';

  @override
  String get excludedDirectoryNames => 'Exclusions must be directory names, one per line.';

  @override
  String get oneDirectoryNamePerLine => 'One directory name per line';

  @override
  String get filesAndFoldersAreScannedIncludingGitignored => 'Files and folders are scanned, including gitignored files. Exclusions narrow the search range. Symbolic links are listed without scanning their targets.';

  @override
  String get directoryAccessIsPreparedAtStartupAnd => 'Directory access is prepared at startup and when the search range changes. Protected locations are skipped. Full Disk Access is optional and must be enabled in System Settings.';

  @override
  String get authorizeSavedDirectories => 'Authorize saved directories';

  @override
  String get fullDiskAccess => 'Full Disk Access…';

  @override
  String get saveFileSearch => 'Save file search';

  @override
  String get addAtLeastOneRule => 'Add at least one rule.';

  @override
  String get contextShortcut => 'Context shortcut';

  @override
  String get shortcutName => 'Shortcut name';

  @override
  String get eGOpenTerminal => 'e.g. Open terminal';

  @override
  String get global => 'Global';

  @override
  String get moreSpecificConditionsTakePriorityEqualMatches => 'More specific conditions take priority. Equal matches let you choose. An unavailable matching action will not fall back.';

  @override
  String get noRulesYetAddAnActionAnd => 'No rules yet. Add an action and choose the app or context where it should run.';

  @override
  String get addRule => 'Add rule';

  @override
  String get saveContextShortcut => 'Save context shortcut';

  @override
  String get savedApp => 'Saved app';

  @override
  String get shortcutRule => 'Shortcut rule';

  @override
  String get inApp => 'In app';

  @override
  String get anyApp => 'Any app';

  @override
  String get withContext => 'With context';

  @override
  String get anyCompatibleContext => 'Any compatible context';

  @override
  String get aRuleAlreadyUsesTheseConditions => 'A rule already uses these conditions.';

  @override
  String get saveRule => 'Save rule';

  @override
  String get finishOrCancelEditingTheActionShortcut => 'Finish or cancel editing the action shortcut before closing.';

  @override
  String get settingsCouldNotBeSavedYouCan => 'Settings could not be saved. You can keep editing or close without saving.';

  @override
  String get newEditsWereMadeWhileSavingSave => 'New edits were made while saving. Save them before closing.';

  @override
  String get changesApplyOnTheNextWakeStatus => 'Changes apply on the next wake. Status refers to the last capture.';

  @override
  String get hiddenApps => 'Hidden apps';

  @override
  String get enterOneApplicationIDPerLineTo => 'Enter one application ID per line to keep it out of search.';

  @override
  String get saveApps => 'Save apps';

  @override
  String get pressShortcutKey => 'Press a shortcut';

  @override
  String get chooseApp => 'Choose app';

  @override
  String get findAnApp => 'Find an app…';

  @override
  String get noMatchingApps => 'No matching apps';

  @override
  String get outputCouldNotBeCopiedTryAgain => 'Output could not be copied. Try again.';

  @override
  String get output => 'Output';

  @override
  String get diagnostics => 'Diagnostics';

  @override
  String get copyOutput => 'Copy output';

  @override
  String get noDiagnosticOutput => 'No diagnostic output.';

  @override
  String get waitingForOutput => 'Waiting for output…';

  @override
  String get noStandardOutput => 'No standard output.';

  @override
  String get followNewOutput => 'Follow new output';

  @override
  String get jumpToLatestOutput => 'Jump to latest output';

  @override
  String get following => 'Following';

  @override
  String get latest => 'Latest';

  @override
  String get theWindowCouldNotBeChangedTry => 'The window could not be changed. Try again.';

  @override
  String get running => 'Running';

  @override
  String get stopping => 'Stopping';

  @override
  String get completed => 'Completed';

  @override
  String get failed => 'Failed';

  @override
  String get stopped => 'Stopped';

  @override
  String get noMatchingTasks => 'No matching tasks';

  @override
  String get tasksKeepRunningWhenHidden => 'Tasks keep running when hidden';

  @override
  String get noRunningTasks => 'No running tasks';

  @override
  String get allTasksFinished => 'All tasks finished';

  @override
  String get launcher => 'Launcher';

  @override
  String get findATask => 'Find a task…';

  @override
  String get clearSearch => 'Clear search';

  @override
  String get tasks => 'Tasks';

  @override
  String get stoppingProgress => 'Stopping…';

  @override
  String get stop => 'Stop';

  @override
  String get moreActions => 'More actions';

  @override
  String get commandCopied => 'Command copied';

  @override
  String get commandCouldNotBeCopiedTryAgain => 'Command could not be copied. Try again.';

  @override
  String get copyCommand => 'Copy command';

  @override
  String get removeResult => 'Remove result';

  @override
  String get commandDetails => 'Command details';

  @override
  String get showingTheLatestOutput128KiBPer => 'Showing the latest output (128 KiB per stream).';

  @override
  String get noTasksYet => 'No tasks yet.';

  @override
  String get chooseAProjectBuildTestOrRun => 'Choose a project Build, Test or Run action in the Launcher.';

  @override
  String get anywhereActionShortcut => 'Anywhere action shortcut';

  @override
  String get notAvailableRightNow => 'Not available right now';

  @override
  String get searchThisFileType => 'Search this file type · Tab / Shift+Tab';

  @override
  String get selectAFileToPreview => 'Select a file to preview';

  @override
  String get findAppsAndActions => 'Find apps and actions';

  @override
  String get actionsForYourCurrentEnvironment => 'Actions for your current environment';

  @override
  String get allActionsAvailableInTheCurrentApp => 'All actions available in the current app';

  @override
  String get copyWithReturnHint => 'Copy · ↵';

  @override
  String get deleteEntry => 'Delete Entry';

  @override
  String get previewUnavailable => 'Preview unavailable';

  @override
  String get loadingPreview => 'Loading preview…';

  @override
  String get selectEntryToPreview => 'Select an entry to preview';

  @override
  String get noMatchingEntries => 'No matching entries';

  @override
  String get noSavedEntries => 'No saved entries';

  @override
  String get newlyCopiedItemsWillAppearHereRefresh => 'Newly copied items will appear here.\nRefresh to see new copies.';

  @override
  String get manageRecordingInSettingsClipboard => 'Manage recording in Settings → Clipboard.';

  @override
  String get imagePreviewUnavailable => 'Image preview unavailable';

  @override
  String get previewUnavailableTheOriginalImageCanStill => 'Preview unavailable. The original image can still be copied.';

  @override
  String get previewIsNotAvailableForThisFormat => 'Preview is not available for this format. Copy restores its saved data.';

  @override
  String get theWindowCouldNotBeClosedUse => 'The window could not be closed. Use the window close button.';

  @override
  String get backToListEsc => 'Back to list · Esc';

  @override
  String get clipboardAlwaysOnTop => 'Clipboard · Always on top';

  @override
  String get searchClipboard => 'Search clipboard…';

  @override
  String get noSavedEntriesManageRecordingInSettings => 'No saved entries.\nManage recording in Settings → Clipboard.';

  @override
  String get actionFailed => 'Action failed';

  @override
  String get actionResult => 'Action result';

  @override
  String get result => 'Result';

  @override
  String get scroll => '↑↓ Scroll';

  @override
  String get tool => 'Tool';

  @override
  String get closeEsc => 'Close · Esc';

  @override
  String get backEsc => 'Back · Esc';

  @override
  String get reloadTool => 'Reload tool';

  @override
  String get backToSearchEsc => 'Back to search · Esc';

  @override
  String get cancelEsc => 'Cancel · Esc';

  @override
  String get cancelEscapeSelected => '✓ Cancel · Esc';

  @override
  String get allowOnce => '✓ Allow once';

  @override
  String get allowOnceAction => 'Allow once';

  @override
  String get webToolsAreUnavailableOnThisPlatform => 'Web tools are unavailable on this platform.';

  @override
  String get requestedAccess => 'Requested access';

  @override
  String get noHostPermissions => 'No Host permissions.';

  @override
  String get codeRunsInALocalWebViewNetwork => 'Code runs in a local WebView. Network access and navigation are blocked. This is not an OS process sandbox.';

  @override
  String get thisReplacesTheInstalledCodeClosesIts => 'This replaces the installed code, closes its active page, and keeps its local settings. A previously approved version can be restored.';

  @override
  String get approveAndInstall => 'Approve and install';

  @override
  String get installedSearchForTheToolByName => 'Installed. Search for the tool by name to open it.';

  @override
  String get thePackageCouldNotBeInstalledThe => 'The package could not be installed. The existing version is unchanged.';

  @override
  String get theOperationCouldNotFinishCheckThe => 'The operation could not finish. Check the current tool status and try again.';

  @override
  String get webTools => 'Web tools';

  @override
  String get import => 'Import…';

  @override
  String get noWebToolsInstalled => 'No Web tools installed.';

  @override
  String get restoreApprovedVersion => 'Restore approved version';

  @override
  String get uninstall => 'Uninstall';

  @override
  String get theToolDidNotBecomeReadyGo => 'The tool did not become ready. Go back and reopen it.';

  @override
  String get thisToolIsNoLongerAvailableReturn => 'This tool is no longer available. Return to search and reopen it.';

  @override
  String get theToolCouldNotLoadReloadIt => 'The tool could not load. Reload it or return to search.';

  @override
  String get openingTool => 'Opening tool…';

  @override
  String get loadingTool => 'Loading tool…';

  @override
  String get theToolStoppedReloadItOrReturn => 'The tool stopped. Reload it or return to search.';

  @override
  String get allowThisOperation => 'Allow this operation?';

  @override
  String get thisTool => 'This tool';

  @override
  String get dismissMessage => 'Dismiss message';

  @override
  String get returnKey => 'Return';

  @override
  String get folder => 'Folder';

  @override
  String get file => 'File';

  @override
  String get selectedFiles => 'Selected files';

  @override
  String get webLink => 'Web link';

  @override
  String get gitRepository => 'Git repository';

  @override
  String get nodeProject => 'Node project';

  @override
  String get mavenProject => 'Maven project';

  @override
  String get gradleProject => 'Gradle project';

  @override
  String get rustProject => 'Rust project';

  @override
  String get springProject => 'Spring Boot project';

  @override
  String get anywhereTool => 'Anywhere tool';

  @override
  String get pdfDocument => 'PDF document';

  @override
  String get document => 'Document';

  @override
  String get textFile => 'Text file';

  @override
  String get spreadsheet => 'Spreadsheet';

  @override
  String get presentation => 'Presentation';

  @override
  String get archive => 'Archive';

  @override
  String get sourceCode => 'Source code';

  @override
  String get configurationFile => 'Configuration file';

  @override
  String get ebook => 'E-book';

  @override
  String get font => 'Font';

  @override
  String get packageOrBinary => 'Package or binary';

  @override
  String get captureOff => 'Off · applies on the next wake';

  @override
  String get captureNotRead => 'Not captured in the last wake';

  @override
  String get captureReady => 'Ready · last wake';

  @override
  String get noItemsSelected => 'No items selected';

  @override
  String get noTextSelected => 'No text selected';

  @override
  String get noPageAvailable => 'No web page available';

  @override
  String get noTextCopied => 'No text copied';

  @override
  String get permissionNeeded => 'Permission needed';

  @override
  String get sourceUnsupported => 'Not supported by this app';

  @override
  String get captureAppChanged => 'App changed during capture';

  @override
  String get timedOut => 'Timed out';

  @override
  String get contentTooLong => 'Content too long';

  @override
  String get tooManyItems => 'Too many selected items';

  @override
  String get sourceUnavailable => 'Could not read this source';

  @override
  String get readFailed => 'Read failed';

  @override
  String get finderPermissionHelp => 'Allow Finder Automation in System Settings, then reopen Anywhere.';

  @override
  String get selectedTextPermissionHelp => 'Allow Accessibility in System Settings, then reopen Anywhere.';

  @override
  String get browserPermissionHelp => 'Allow Anywhere to access this browser in System Settings → Privacy & Security → Automation, then reopen Anywhere.';

  @override
  String get clipboardPermissionHelp => 'Allow clipboard access in System Settings, then reopen Anywhere.';

  @override
  String get appChangedHelp => 'The original app exited or restarted. Reopen Anywhere to capture it again.';

  @override
  String get tooManyItemsHelp => 'Select up to 32 items in Finder, then reopen Anywhere.';

  @override
  String get loadingMoreFiles => 'Loading more files…';

  @override
  String get authorizeSearchDirectories => 'Authorize search directories';

  @override
  String get authorizeSearchHelp => 'Open Settings → Search and authorize saved directories. No file search runs before access is prepared.';

  @override
  String get fileSearchOff => 'File search is turned off';

  @override
  String get searchUnavailable => 'Search is unavailable';

  @override
  String get partialResults => 'Partial results';

  @override
  String get noMatchingFiles => 'No matching files';

  @override
  String get noContentActions => 'No actions for this content';

  @override
  String get searchingFiles => 'Searching files…';

  @override
  String get fileSearchUnavailable => 'File search unavailable';

  @override
  String get fileSearchDisabledHelp => 'File search is disabled. Enable it in Settings → Search.';

  @override
  String get typeFileName => 'Type a file or folder name to search';

  @override
  String get fileSearchIncompleteHelp => 'File search is incomplete. Check the issue details and retry.';

  @override
  String get noMatchingActions => 'No matching actions';

  @override
  String get noActionsAvailable => 'No actions available';

  @override
  String get fileSearchIncomplete => 'File search is incomplete. Available results are still usable.';

  @override
  String get actionsLoadIncomplete => 'Some actions could not be loaded. Reopen Anywhere to try again; other actions are still available.';

  @override
  String get fileSearchOtherResults => 'File search unavailable · Other results are still usable';

  @override
  String get currentPage => 'Current page';

  @override
  String get chooseTarget => 'Choose a target';

  @override
  String get contentActions => 'Actions for this content';

  @override
  String get searchComplete => 'Search complete';

  @override
  String get clipboardSettingsFailed => 'Clipboard settings could not be loaded or updated. Try again.';

  @override
  String get historyLoadFailed => 'Clipboard history could not be loaded. Try Refresh.';

  @override
  String get historyEntryMissing => 'This entry is no longer available. Refresh history.';

  @override
  String get historyPreviewFailed => 'This entry could not be previewed.';

  @override
  String get historyOperationFailed => 'The operation could not be completed. Refresh and try again.';

  @override
  String get historyCopyFailed => 'This entry could not be copied. For files, check that the originals still exist and are accessible.';

  @override
  String sourceUnsupportedHelp(String source) {
    return '$source is not exposed by this app. Other sources still work.';
  }

  @override
  String sourceTimeoutHelp(String source) {
    return '$source took too long to respond. Reopen Anywhere to try again.';
  }

  @override
  String sourceTooLongHelp(String source) {
    return '$source is too long to use. Use less content and reopen Anywhere.';
  }

  @override
  String sourceFailedHelp(String source) {
    return '$source could not be read. Reopen Anywhere to try again; other sources still work.';
  }

  @override
  String fileFoundCount(int count) {
    return '$count found';
  }

  @override
  String searchIssues(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count issues',
      one: '1 issue',
    );
    return '$_temp0';
  }

  @override
  String fileSearchIssueNotice(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'File search encountered $count issues. Available results are still usable.',
      one: 'File search encountered 1 issue. Available results are still usable.',
    );
    return '$_temp0';
  }

  @override
  String filterActions(String scope) {
    return 'Filter $scope actions';
  }

  @override
  String emptyFileType(String type) {
    return 'No $type in these results. Try All types or another search.';
  }

  @override
  String fileRootsHint(String example) {
    return '$example\nLeave empty to search your home directory';
  }

  @override
  String runningCount(int count) {
    return '$count running';
  }

  @override
  String exitCode(String code) {
    return 'Exit $code';
  }

  @override
  String taskKeyboardHelp(String find, String stop) {
    return '↑↓ Tasks   $find Find   $stop Stop   Esc Hide';
  }

  @override
  String shortcutHint(String label, String shortcut) {
    return '$label · $shortcut';
  }

  @override
  String enableFeature(String name) {
    return 'Enable $name';
  }

  @override
  String allowFeature(String name) {
    return 'Allow $name?';
  }

  @override
  String removeRule(int number) {
    return 'Remove rule $number';
  }

  @override
  String contextRulesCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count context rules',
      one: '1 context rule',
    );
    return '$_temp0';
  }

  @override
  String editActionShortcut(String title) {
    return 'Edit shortcut for $title';
  }

  @override
  String removeActionShortcut(String title) {
    return 'Remove shortcut for $title';
  }

  @override
  String get results => 'Results';

  @override
  String get succeeded => 'Succeeded';

  @override
  String get input => 'Input';

  @override
  String get web => 'Web';

  @override
  String get tools => 'Tools';

  @override
  String restrictedLocationsSkipped(String count) {
    return '$count restricted locations skipped';
  }

  @override
  String clipboardConsent(String tool) {
    return '$tool is requesting a clipboard operation. Approval applies once.';
  }

  @override
  String get quitAnywhere => 'Quit Anywhere';

  @override
  String get edit => 'Edit';

  @override
  String get undo => 'Undo';

  @override
  String get redo => 'Redo';

  @override
  String get cut => 'Cut';

  @override
  String get paste => 'Paste';

  @override
  String get selectAll => 'Select All';

  @override
  String get window => 'Window';

  @override
  String get minimize => 'Minimize';

  @override
  String get zoom => 'Zoom';

  @override
  String get closeWindow => 'Close Window';

  @override
  String get findTaskMenu => 'Find Task…';

  @override
  String get stopSelectedTask => 'Stop Selected Task';

  @override
  String get refreshTasks => 'Refresh Tasks';

  @override
  String get openLauncher => 'Open Launcher';

  @override
  String get closeTaskWindow => 'Close Task Window';

  @override
  String get runningTasksMenu => 'Running Tasks…';

  @override
  String get settingsMenu => 'Settings…';

  @override
  String get authorizeFileSearch => 'Authorize File Search';

  @override
  String get authorize => 'Authorize';

  @override
  String get importLocalWebTool => 'Import a local Web tool';

  @override
  String get importPackage => 'Import';

  @override
  String get fileSearchAccessMessage => 'Choose folders Anywhere may search. Access is saved for future launches. Protected system or app data may still require Full Disk Access in System Settings.';

  @override
  String get inspectTrustedPackage => 'Choose a trusted .awplugin package to inspect before installing.';

  @override
  String get readClipboardText => 'Read clipboard text';

  @override
  String get copyTextToClipboard => 'Copy text to clipboard';

  @override
  String get permissionCurrentApp => 'Current application name';

  @override
  String get permissionReadClipboard => 'Read clipboard text (confirmed per operation)';

  @override
  String get permissionWriteClipboard => 'Copy text (confirmed per operation)';

  @override
  String get permissionLocalStorage => 'Private local settings (1 MiB)';

  @override
  String get launcherStartupFailed => 'Launcher could not start';

  @override
  String get tasksStartupFailed => 'Running Tasks could not open';

  @override
  String get shelfStartupFailed => 'Clipboard Shelf could not open';

  @override
  String get savedDirectories => 'Saved Directories';

  @override
  String get browserPageDescription => 'Use the active Safari, Chrome, or Edge tab’s web address';

  @override
  String permissionContextRead(String types) {
    return 'Selected context: $types';
  }

  @override
  String get supportedBrowserDescription => 'Active URL in a supported browser';

  @override
  String installToolReview(String name) {
    return 'Install $name?';
  }

  @override
  String updateToolReview(String name) {
    return 'Update $name?';
  }

  @override
  String get paused => 'Paused';

  @override
  String get detail => 'Detail';

  @override
  String formatsCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count formats',
      one: '1 format',
    );
    return '$_temp0';
  }

  @override
  String get automationUsage => 'Read the current Finder location and selection, and the active Safari, Chrome, or Edge tab URL to suggest actions when you open Anywhere.';

  @override
  String contentPreview(String kind) {
    return '$kind preview';
  }

  @override
  String savedSize(String size) {
    return '$size saved';
  }

  @override
  String get invalidActionIdentity => 'Invalid action identity.';

  @override
  String get letterWithModifier => 'Use a letter key with a modifier.';

  @override
  String get modifierWithLetter => 'Use a modifier with the letter key.';

  @override
  String get reservedEditingShortcut => 'This shortcut is reserved for editing or window commands.';

  @override
  String get invalidShortcutIntent => 'Invalid shortcut intent identity.';

  @override
  String get shortcutRuleLimit => 'Name the shortcut and use at most 16 rules.';

  @override
  String get differentShortcutConditions => 'Rules must use different app and context conditions.';

  @override
  String get shortcutAlreadyOpens => 'This shortcut already opens Anywhere.';

  @override
  String get invalidShortcutRule => 'Invalid shortcut rule.';

  @override
  String get searchDirectoryLimits => 'Use at most 8 directories and 32 exclusions.';

  @override
  String get absoluteSearchDirectories => 'Search directories must be absolute paths, one per line.';

  @override
  String get unsupportedShortcutSettings => 'Unsupported action shortcut settings version. Update Anywhere before changing settings.';

  @override
  String get oneShortcutPerAction => 'An action can only have one shortcut.';

  @override
  String get shortcutsRefreshFailed => 'Action shortcuts could not be refreshed. Reopen Settings to try again.';

  @override
  String shortcutAssignedTo(String title) {
    return 'This shortcut is assigned to $title.';
  }

  @override
  String maxActionShortcuts(int count) {
    return 'At most $count action shortcuts are supported.';
  }

  @override
  String get shortcutsSavedByNewerVersion => 'Action shortcuts were saved by a newer version. Update Anywhere before changing settings.';

  @override
  String get directoryAccessFailed => 'Directory access could not be prepared. Try again in Search settings.';

  @override
  String get openSystemSettingsFailed => 'System Settings could not be opened. Open the system settings manually.';

  @override
  String get shortcutsRestoreFailed => 'Action shortcuts could not be restored. Change the conflicting shortcut or try closing again.';

  @override
  String extensionsLoadFailed(String detail) {
    return 'Extensions could not be loaded: $detail';
  }

  @override
  String extensionsUpdateFailed(String detail) {
    return 'Extension settings could not be updated: $detail';
  }

  @override
  String shortcutLoadFailed(String detail) {
    return 'Action shortcuts could not be loaded: $detail';
  }

  @override
  String get shortcutsPausedHelp => 'Some action shortcuts are paused. Reopen Settings to try again, or choose a different key combination.';

  @override
  String get shelfSelectHint => 'Single-click selects · Double-click copies';

  @override
  String get shelfCopyAndReturn => 'Copy and return to app';

  @override
  String get shelfDeleteTitle => 'Remove this entry?';

  @override
  String get shelfDeleteDescription => 'Only this saved history entry will be removed.';

  @override
  String get hidePreview => 'Hide preview';

  @override
  String shelfPreviewScrollHint(String shortcut) {
    return '$shortcut Scroll preview';
  }

  @override
  String get details => 'Details';

  @override
  String get sourceStatus => 'Source status';

  @override
  String get searchRules => 'Search rules';

  @override
  String get searchHelp => 'How search works';

  @override
  String get toolOptions => 'Tool options';

  @override
  String get keyboardHelp => 'Keyboard shortcuts';

  @override
  String get fileInformation => 'File information';

  @override
  String get historyFeatureDisabled => 'Clipboard History is disabled in Features.';

  @override
  String get historyStorageUnavailable => 'Clipboard history storage is unavailable. Restart after repairing the data file.';

  @override
  String get historyAccessRequired => 'Allow clipboard access in system settings to record new copies. Background capture will not prompt.';

  @override
  String get historyItemTooLarge => 'The copied item exceeds the 16 MiB history limit and was not saved.';

  @override
  String get historyReadFailed => 'A clipboard item could not be read. Existing history is still available.';

  @override
  String get historyCaptureUnavailable => 'Clipboard history capture is unavailable. Existing history is still available.';

  @override
  String get historyMonitoringUnavailable => 'Clipboard monitoring is unavailable on this platform.';

  @override
  String get filePreviewChanged => 'Preview unavailable. Search again if the file has changed.';

  @override
  String get filePreviewUnsupported => 'Text preview is unavailable for this file type';

  @override
  String filePreviewTruncated(String size) {
    return 'Showing the first $size';
  }

  @override
  String filePreviewEncodingGuessed(String encoding) {
    return 'Encoding guessed as $encoding';
  }

  @override
  String filePreviewDecoded(String encoding) {
    return 'Decoded as $encoding';
  }

  @override
  String get shelfReturnFailed => 'Copied. Could not return to the app. Switch to it to paste.';

  @override
  String get shelfReturnUnavailable => 'Copied. The destination app is no longer available. Switch to an app to paste.';

  @override
  String get historyRecordingPaused => 'History recording is paused. Existing history is still available.';

  @override
  String get pauseHistoryRecording => 'Pause without clearing saved history';

  @override
  String historyPausedUntil(String time) {
    return 'Paused until $time';
  }

  @override
  String get resumeRecording => 'Resume';

  @override
  String get pauseFor => 'Pause…';

  @override
  String pauseMinutes(int minutes) {
    return '$minutes minutes';
  }

  @override
  String get keepHistoryFor => 'Keep history for';

  @override
  String get untilStorageLimit => 'Until storage limit';

  @override
  String retentionDays(int days) {
    String _temp0 = intl.Intl.pluralLogic(
      days,
      locale: localeName,
      other: '$days days',
      one: '1 day',
    );
    return '$_temp0';
  }

  @override
  String get changeHistoryRetention => 'Change history retention?';

  @override
  String get historyRetentionWarning => 'Entries older than the selected period will be permanently deleted now. Extending the period later cannot restore them.';

  @override
  String historyStorageUsage(int count, String mib) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count entries',
      one: '1 entry',
    );
    return '$_temp0 · $mib MiB stored';
  }

  @override
  String get historyStorageExplanation => 'Stored only on this Mac, without encryption. Up to 1,000 entries / 128 MiB of logical data; database file size may differ. Expiry deletes all saved formats, not the system clipboard or original files.';

  @override
  String get appHistory => 'App History';

  @override
  String get appHistorySearch => 'Search recent projects and files…';

  @override
  String get appHistorySources => 'History sources';

  @override
  String get appHistorySourceDescription => 'Enable a source by choosing its history metadata file. Only that file is read; history is not imported permanently. This preview supports IDEA and VS Code.';

  @override
  String get appHistorySourcePath => 'History metadata file path';

  @override
  String get appHistoryEnable => 'Enable source';

  @override
  String get appHistoryDisable => 'Disable source';

  @override
  String get appHistoryUnconfigured => 'Enable a source in Settings → App History to see recent projects and files.';

  @override
  String get appHistoryReadFailed => 'Some sources could not be read. Check their files in Settings → App History.';

  @override
  String get appHistoryDisabled => 'App History is disabled. Enable it in Settings → Features.';

  @override
  String get appHistoryOperationFailed => 'The history operation could not be completed. Refresh the list and try again.';

  @override
  String get appHistorySettingsFailed => 'History sources could not be loaded or saved. Check the file path and try again.';

  @override
  String get appHistoryAll => 'All apps';

  @override
  String get appHistoryCopyTarget => 'Copy path or URI';
}
