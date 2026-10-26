// ignore: unused_import
import 'package:intl/intl.dart' as intl;
import 'app_localizations.dart';

// ignore_for_file: type=lint

/// The translations for Chinese (`zh`).
class AppLocalizationsZh extends AppLocalizations {
  AppLocalizationsZh([String locale = 'zh']) : super(locale);

  @override
  String get settings => '设置';

  @override
  String get general => '通用';

  @override
  String get shortcuts => '快捷键';

  @override
  String get context => '上下文';

  @override
  String get search => '搜索';

  @override
  String get clipboard => '剪贴板';

  @override
  String get features => '功能';

  @override
  String get language => '语言';

  @override
  String get languageDescription => '选择 Anywhere 使用的语言';

  @override
  String get languageSystem => '跟随系统';

  @override
  String get languageEnglish => 'English';

  @override
  String get languageChinese => '简体中文';

  @override
  String get done => '完成';

  @override
  String get saving => '正在保存…';

  @override
  String get openAtLogin => '登录时启动';

  @override
  String get openAtLoginDescription => '登录后在后台启动';

  @override
  String get currentApp => '当前应用';

  @override
  String get globalSearch => '全局搜索';

  @override
  String get universalSearch => '全局搜索';

  @override
  String get navigationModes => '全局搜索 / 当前应用';

  @override
  String get backToResults => '返回结果';

  @override
  String get back => '返回';

  @override
  String get run => '运行';

  @override
  String get open => '打开';

  @override
  String get copy => '复制';

  @override
  String get actions => '操作';

  @override
  String get retry => '重试';

  @override
  String get cancel => '取消';

  @override
  String get close => '关闭';

  @override
  String get clear => '清空';

  @override
  String get remove => '移除';

  @override
  String get delete => '删除';

  @override
  String get preview => '预览';

  @override
  String get kind => '类型';

  @override
  String get size => '大小';

  @override
  String get modified => '修改时间';

  @override
  String get location => '位置';

  @override
  String get allTypes => '所有类型';

  @override
  String get files => '文件';

  @override
  String get folders => '文件夹';

  @override
  String get documents => '文档';

  @override
  String get pdf => 'PDF';

  @override
  String get images => '图片';

  @override
  String get audio => '音频';

  @override
  String get video => '视频';

  @override
  String get archives => '压缩文件';

  @override
  String get code => '代码';

  @override
  String get fileSearch => '文件搜索';

  @override
  String get clipboardHistory => '剪贴板历史';

  @override
  String get clipboardShelf => '悬浮剪贴板';

  @override
  String get runningTasks => '运行中的任务';

  @override
  String get searchAppsActions => '搜索应用和操作…';

  @override
  String get searchFilesFolders => '搜索文件和文件夹…';

  @override
  String get searchHistory => '搜索剪贴板历史…';

  @override
  String get findFilesFolders => '查找文件和文件夹';

  @override
  String get noResults => '无结果';

  @override
  String get copied => '已复制';

  @override
  String get copiedPaste => '已复制，可在目标应用中粘贴。';

  @override
  String get copyFailed => '复制失败，请重试。';

  @override
  String get loading => '正在加载…';

  @override
  String get searching => '正在搜索…';

  @override
  String get suggested => '建议';

  @override
  String get menus => '菜单';

  @override
  String get browseMenus => '浏览菜单…';

  @override
  String get contextSources => '上下文来源';

  @override
  String get contextDescription => '选择唤醒 Anywhere 时可以使用的内容。';

  @override
  String get finderSelection => '访达选中项';

  @override
  String get fileSelection => '选中文件';

  @override
  String get selectedFilesDescription => '选中的文件和文件夹';

  @override
  String get selectedText => '选中文本';

  @override
  String get selectedTextDescription => '当前应用中选中的文本';

  @override
  String get browserCurrentPage => '浏览器当前页面';

  @override
  String get clipboardText => '剪贴板文本';

  @override
  String get openSettings => '打开设置';

  @override
  String get keepEditing => '继续编辑';

  @override
  String get closeWithoutSaving => '放弃修改并关闭';

  @override
  String get saveChanges => '保存修改';

  @override
  String get saveBeforeClose => '关闭前保存修改？';

  @override
  String get changesNotSaved => '部分修改尚未保存';

  @override
  String get settingsDrafts => '部分设置存在尚未保存的修改。';

  @override
  String get settingsSaveFailed => '设置保存失败，请重试。';

  @override
  String get shortcutHelp => '选择快捷键，再按下新的按键组合。按 Esc 取消。';

  @override
  String get workspace => '工作区';

  @override
  String get image => '图片';

  @override
  String get richText => '富文本';

  @override
  String get data => '数据';

  @override
  String get text => '文本';

  @override
  String searchAppActions(String app) {
    return '搜索 $app 的操作…';
  }

  @override
  String contentEscapeHint(String source) {
    return '$source · Esc → 当前应用';
  }

  @override
  String fileRange(String range) {
    return '文件搜索 · $range';
  }

  @override
  String get editCapturedContent => '编辑捕获的内容…';

  @override
  String fileLocations(int count) {
    return '文件搜索 · $count 个位置';
  }

  @override
  String get actionShortcuts => '动作快捷键';

  @override
  String get bindAKeyToAnActionOr => '为动作绑定按键，或根据上下文选择要执行的动作。';

  @override
  String get globalWorksFromOtherApps => '全局 · 在其他应用中也可使用';

  @override
  String get whileAnywhereIsOpen => '仅在 Anywhere 打开时可用';

  @override
  String get actionUnavailableShortcutPaused => '动作不可用 · 快捷键已暂停';

  @override
  String get actionsUnavailableShortcutPaused => '动作不可用 · 快捷键已暂停';

  @override
  String get someRulesAreUnavailable => '部分规则不可用';

  @override
  String get notDetectedInTheLastContext => '上次未检测到对应上下文';

  @override
  String get addActionShortcut => '添加动作快捷键';

  @override
  String get addContextShortcut => '添加上下文快捷键';

  @override
  String get toolsSuchAsFileSearchAreAlways => '文件搜索等工具始终可绑定。项目动作会在对应项目处于当前上下文时显示。';

  @override
  String get actionShortcut => '动作快捷键';

  @override
  String get findAnAction => '查找动作…';

  @override
  String get notDetectedInTheLastContextThe => '上次未检测到对应上下文，已保留保存的动作。';

  @override
  String get shortcutRecordingFailedTryAgain => '快捷键录制失败，请重试。';

  @override
  String get pressAShortcut => '请按下快捷键…';

  @override
  String get recordShortcut => '录制快捷键';

  @override
  String get useCommandOptionOrControlWithA => '请使用 Command、Option 或 Control 加字母。按 Esc 取消录制。';

  @override
  String get globalShortcut => '全局快捷键';

  @override
  String get worksFromOtherAppsUsesFreshContext => '在其他应用中也可使用，需要时会重新采集上下文。';

  @override
  String get chooseAnotherAction => '选择其他动作';

  @override
  String get saveShortcut => '保存快捷键';

  @override
  String get clearClipboardHistory => '清空剪贴板历史？';

  @override
  String get allSavedEntriesWillBePermanentlyRemoved => '所有历史记录将永久删除，系统剪贴板的当前内容不变。';

  @override
  String get clearHistory => '清空历史';

  @override
  String get clipboardHistorySetting => '剪贴板历史';

  @override
  String get recordClipboardHistory => '记录剪贴板历史';

  @override
  String get saveFutureCopiesWhileAnywhereIsRunning => '保存 Anywhere 运行期间新复制的内容，不导入启动前已有的内容。';

  @override
  String get textImagesRichTextAndFileReferences => '文本、图片、富文本和文件引用均保存在本机。暂停记录会保留已有历史。';

  @override
  String get tryAgain => '重试';

  @override
  String get whenOpeningLauncher => '唤醒启动器时';

  @override
  String get autoFillSupportedContentCopiedWithinThe => '自动填入最近 15 秒内复制且可识别的内容，与历史记录开关相互独立。';

  @override
  String get historyStorage => '历史记录存储';

  @override
  String get removeAllSavedEntriesThisDoesNot => '删除所有已保存的记录，不改变采集开关或当前系统剪贴板。';

  @override
  String get clearHistoryAction => '清空历史…';

  @override
  String get enabled => '已启用';

  @override
  String get disabled => '已停用';

  @override
  String get awaitingPermission => '等待授权';

  @override
  String get couldNotStart => '无法启动';

  @override
  String get unavailable => '不可用';

  @override
  String get restartRequired => '需要重新启动';

  @override
  String get updating => '正在更新…';

  @override
  String get refresh => '刷新';

  @override
  String get chooseTheToolsAvailableInAnywhere => '选择在 Anywhere 中可用的工具。';

  @override
  String get builtIn => '内置功能';

  @override
  String get installedPlugins => '已安装的插件';

  @override
  String get legacyV1PermissionsCannotBeRevokedIndividually => '旧版 v1 权限无法逐项撤销，停用扩展后可停止其动作。';

  @override
  String get reviewPermissions => '查看权限…';

  @override
  String get revokePermissions => '撤销权限';

  @override
  String get thisExtensionRequestsPermissionTo => '此扩展请求以下权限：';

  @override
  String get noHostPermissionsRequested => '未请求宿主权限。';

  @override
  String get approvalAppliesToThisInspectedDefinitionRestart => '授权仅适用于当前检查的定义。新授权的插件将在重启 Anywhere 后生效；撤销权限会立即生效。';

  @override
  String get allowForNextLaunch => '允许下次启动时使用';

  @override
  String get showAPluginViewInAnywhere => '在 Anywhere 中显示插件页面';

  @override
  String get writeToTheClipboard => '写入剪贴板';

  @override
  String get openWebAddresses => '打开网页地址';

  @override
  String get openFiles => '打开文件';

  @override
  String get readRepositoryInformation => '读取仓库信息';

  @override
  String get runProjectTasks => '运行项目任务';

  @override
  String get saveExtensionData => '保存扩展数据';

  @override
  String get fileSearchSetting => '文件搜索';

  @override
  String get searchFileNamesInTheFileSearch => '通过文件搜索动作按名称查找文件';

  @override
  String get systemSearchShowsEarlyResultsFilenameScanning => '系统搜索先显示结果，文件名扫描仅在可访问且已授权的目录中进行。';

  @override
  String get includeHiddenFiles => '包含隐藏文件';

  @override
  String get includeDotfilesAndFilesInHiddenDirectories => '包含以点开头的文件和隐藏目录中的文件';

  @override
  String get searchDirectories => '搜索目录';

  @override
  String get excludedDirectoryNames => '排除项必须是目录名，每行一个。';

  @override
  String get oneDirectoryNamePerLine => '每行一个目录名';

  @override
  String get filesAndFoldersAreScannedIncludingGitignored => '扫描文件和文件夹，包括被 Git 忽略的文件。排除项会缩小搜索范围。符号链接只列出链接本身，不扫描目标。';

  @override
  String get directoryAccessIsPreparedAtStartupAnd => '启动和变更搜索范围时预先检查目录权限，跳过受保护的位置。完全磁盘访问权限为可选项，需在系统设置中启用。';

  @override
  String get authorizeSavedDirectories => '授权已保存的目录';

  @override
  String get fullDiskAccess => '完全磁盘访问权限…';

  @override
  String get saveFileSearch => '保存文件搜索设置';

  @override
  String get addAtLeastOneRule => '请至少添加一条规则。';

  @override
  String get contextShortcut => '上下文快捷键';

  @override
  String get shortcutName => '快捷键名称';

  @override
  String get eGOpenTerminal => '例如：打开终端';

  @override
  String get global => '全局';

  @override
  String get moreSpecificConditionsTakePriorityEqualMatches => '优先匹配更具体的条件。多条规则同等匹配时由你选择；已匹配的动作不可用时，不会执行其他动作。';

  @override
  String get noRulesYetAddAnActionAnd => '还没有规则。添加动作，并选择在哪个应用或上下文中执行。';

  @override
  String get addRule => '添加规则';

  @override
  String get saveContextShortcut => '保存上下文快捷键';

  @override
  String get savedApp => '已保存的应用';

  @override
  String get shortcutRule => '快捷键规则';

  @override
  String get inApp => '应用';

  @override
  String get anyApp => '任意应用';

  @override
  String get withContext => '上下文';

  @override
  String get anyCompatibleContext => '任意兼容的上下文';

  @override
  String get aRuleAlreadyUsesTheseConditions => '已有规则使用了相同条件。';

  @override
  String get saveRule => '保存规则';

  @override
  String get finishOrCancelEditingTheActionShortcut => '请先完成或取消动作快捷键编辑，再关闭窗口。';

  @override
  String get settingsCouldNotBeSavedYouCan => '设置保存失败。你可以继续编辑，或不保存直接关闭。';

  @override
  String get newEditsWereMadeWhileSavingSave => '保存期间有新的修改，请先保存再关闭。';

  @override
  String get changesApplyOnTheNextWakeStatus => '更改将在下次唤醒时生效，状态显示上一次采集的结果。';

  @override
  String get hiddenApps => '隐藏的应用';

  @override
  String get enterOneApplicationIDPerLineTo => '每行输入一个应用 ID，使其不出现在搜索结果中。';

  @override
  String get saveApps => '保存应用设置';

  @override
  String get pressShortcutKey => '请按下快捷键';

  @override
  String get chooseApp => '选择应用';

  @override
  String get findAnApp => '查找应用…';

  @override
  String get noMatchingApps => '没有匹配的应用';

  @override
  String get outputCouldNotBeCopiedTryAgain => '无法复制输出，请重试。';

  @override
  String get output => '输出';

  @override
  String get diagnostics => '诊断';

  @override
  String get copyOutput => '复制输出';

  @override
  String get noDiagnosticOutput => '没有诊断输出。';

  @override
  String get waitingForOutput => '正在等待输出…';

  @override
  String get noStandardOutput => '没有标准输出。';

  @override
  String get followNewOutput => '跟随新输出';

  @override
  String get jumpToLatestOutput => '跳到最新输出';

  @override
  String get following => '跟随中';

  @override
  String get latest => '最新';

  @override
  String get theWindowCouldNotBeChangedTry => '无法调整窗口，请重试。';

  @override
  String get running => '运行中';

  @override
  String get stopping => '正在停止';

  @override
  String get completed => '已完成';

  @override
  String get failed => '失败';

  @override
  String get stopped => '已停止';

  @override
  String get noMatchingTasks => '没有匹配的任务';

  @override
  String get tasksKeepRunningWhenHidden => '隐藏窗口后任务继续运行';

  @override
  String get noRunningTasks => '没有正在运行的任务';

  @override
  String get allTasksFinished => '所有任务均已结束';

  @override
  String get launcher => '启动器';

  @override
  String get findATask => '查找任务…';

  @override
  String get clearSearch => '清空搜索';

  @override
  String get tasks => '任务';

  @override
  String get stoppingProgress => '正在停止…';

  @override
  String get stop => '停止';

  @override
  String get moreActions => '更多操作';

  @override
  String get commandCopied => '已复制命令';

  @override
  String get commandCouldNotBeCopiedTryAgain => '无法复制命令，请重试。';

  @override
  String get copyCommand => '复制命令';

  @override
  String get removeResult => '移除结果';

  @override
  String get commandDetails => '命令详情';

  @override
  String get showingTheLatestOutput128KiBPer => '显示最新输出，每个输出流最多保留 128 KiB。';

  @override
  String get noTasksYet => '还没有任务。';

  @override
  String get chooseAProjectBuildTestOrRun => '在启动器中选择项目的构建、测试或运行动作。';

  @override
  String get anywhereActionShortcut => 'Anywhere 动作快捷键';

  @override
  String get notAvailableRightNow => '暂时不可用';

  @override
  String get searchThisFileType => '搜索此文件类型 · Tab / Shift+Tab';

  @override
  String get selectAFileToPreview => '选择文件以预览';

  @override
  String get findAppsAndActions => '查找应用和动作';

  @override
  String get actionsForYourCurrentEnvironment => '当前环境的动作';

  @override
  String get allActionsAvailableInTheCurrentApp => '当前应用环境中的所有可用动作';

  @override
  String get copyWithReturnHint => '复制 · ↵';

  @override
  String get deleteEntry => '删除此记录';

  @override
  String get previewUnavailable => '无法预览';

  @override
  String get loadingPreview => '正在加载预览…';

  @override
  String get selectEntryToPreview => '选择一条记录以查看预览';

  @override
  String get noMatchingEntries => '没有匹配的记录';

  @override
  String get noSavedEntries => '没有历史记录';

  @override
  String get newlyCopiedItemsWillAppearHereRefresh => '新复制的内容会出现在这里。\n刷新以查看新记录。';

  @override
  String get manageRecordingInSettingsClipboard => '在“设置 → 剪贴板”中管理记录。';

  @override
  String get imagePreviewUnavailable => '无法预览图片';

  @override
  String get previewUnavailableTheOriginalImageCanStill => '无法预览，仍可复制原始图片。';

  @override
  String get previewIsNotAvailableForThisFormat => '此格式暂不支持预览。复制时会恢复已保存的数据。';

  @override
  String get theWindowCouldNotBeClosedUse => '无法关闭窗口，请使用窗口关闭按钮。';

  @override
  String get backToListEsc => '返回列表 · Esc';

  @override
  String get clipboardAlwaysOnTop => '剪贴板 · 置顶';

  @override
  String get searchClipboard => '搜索剪贴板…';

  @override
  String get noSavedEntriesManageRecordingInSettings => '没有历史记录。\n请在“设置 → 剪贴板”中管理记录。';

  @override
  String get actionFailed => '动作失败';

  @override
  String get actionResult => '动作结果';

  @override
  String get result => '结果';

  @override
  String get scroll => '↑↓ 滚动';

  @override
  String get tool => '工具';

  @override
  String get closeEsc => '关闭 · Esc';

  @override
  String get backEsc => '返回 · Esc';

  @override
  String get reloadTool => '重新加载工具';

  @override
  String get backToSearchEsc => '返回搜索 · Esc';

  @override
  String get cancelEsc => '取消 · Esc';

  @override
  String get cancelEscapeSelected => '✓ 取消 · Esc';

  @override
  String get allowOnce => '✓ 允许一次';

  @override
  String get allowOnceAction => '允许一次';

  @override
  String get webToolsAreUnavailableOnThisPlatform => '此平台暂不支持 Web 工具。';

  @override
  String get requestedAccess => '请求的权限';

  @override
  String get noHostPermissions => '未请求宿主权限。';

  @override
  String get codeRunsInALocalWebViewNetwork => '代码在本地 WebView 中运行，禁止联网和页面跳转。此限制不等同于操作系统级进程沙箱。';

  @override
  String get thisReplacesTheInstalledCodeClosesIts => '这将替换已安装的代码并关闭其当前页面，保留本地设置。之后可恢复到曾经批准的版本。';

  @override
  String get approveAndInstall => '批准并安装';

  @override
  String get installedSearchForTheToolByName => '安装完成，搜索工具名称即可打开。';

  @override
  String get thePackageCouldNotBeInstalledThe => '安装失败，现有版本未改变。';

  @override
  String get theOperationCouldNotFinishCheckThe => '操作未完成，请检查工具状态后重试。';

  @override
  String get webTools => 'Web 工具';

  @override
  String get import => '导入…';

  @override
  String get noWebToolsInstalled => '尚未安装 Web 工具。';

  @override
  String get restoreApprovedVersion => '恢复已批准的版本';

  @override
  String get uninstall => '卸载';

  @override
  String get theToolDidNotBecomeReadyGo => '工具未就绪，请返回后重新打开。';

  @override
  String get thisToolIsNoLongerAvailableReturn => '此工具已不可用，请返回搜索后重新打开。';

  @override
  String get theToolCouldNotLoadReloadIt => '工具加载失败，请重新加载或返回搜索。';

  @override
  String get openingTool => '正在打开工具…';

  @override
  String get loadingTool => '正在加载工具…';

  @override
  String get theToolStoppedReloadItOrReturn => '工具已停止，请重新加载或返回搜索。';

  @override
  String get allowThisOperation => '允许此操作？';

  @override
  String get thisTool => '此工具';

  @override
  String get dismissMessage => '关闭提示';

  @override
  String get returnKey => '回车';

  @override
  String get folder => '文件夹';

  @override
  String get file => '文件';

  @override
  String get selectedFiles => '所选文件';

  @override
  String get webLink => '网页链接';

  @override
  String get gitRepository => 'Git 仓库';

  @override
  String get nodeProject => 'Node 项目';

  @override
  String get mavenProject => 'Maven 项目';

  @override
  String get gradleProject => 'Gradle 项目';

  @override
  String get rustProject => 'Rust 项目';

  @override
  String get springProject => 'Spring Boot 项目';

  @override
  String get anywhereTool => 'Anywhere 工具';

  @override
  String get pdfDocument => 'PDF 文档';

  @override
  String get document => '文档';

  @override
  String get textFile => '文本文件';

  @override
  String get spreadsheet => '电子表格';

  @override
  String get presentation => '演示文稿';

  @override
  String get archive => '压缩包';

  @override
  String get sourceCode => '源代码';

  @override
  String get configurationFile => '配置文件';

  @override
  String get ebook => '电子书';

  @override
  String get font => '字体';

  @override
  String get packageOrBinary => '软件包或二进制文件';

  @override
  String get captureOff => '已关闭 · 下次唤醒时生效';

  @override
  String get captureNotRead => '上次唤醒时未采集';

  @override
  String get captureReady => '已就绪 · 上次唤醒';

  @override
  String get noItemsSelected => '未选择项目';

  @override
  String get noTextSelected => '未选择文本';

  @override
  String get noPageAvailable => '没有可用的网页';

  @override
  String get noTextCopied => '没有复制的文本';

  @override
  String get permissionNeeded => '需要授权';

  @override
  String get sourceUnsupported => '此应用不支持';

  @override
  String get captureAppChanged => '采集期间应用发生变化';

  @override
  String get timedOut => '超时';

  @override
  String get contentTooLong => '内容过长';

  @override
  String get tooManyItems => '所选项目过多';

  @override
  String get sourceUnavailable => '无法读取此来源';

  @override
  String get readFailed => '读取失败';

  @override
  String get finderPermissionHelp => '请在系统设置中允许控制访达，然后重新唤醒 Anywhere。';

  @override
  String get selectedTextPermissionHelp => '请在系统设置中允许辅助功能访问，然后重新唤醒 Anywhere。';

  @override
  String get browserPermissionHelp => '请在“系统设置 → 隐私与安全性 → 自动化”中允许 Anywhere 访问此浏览器，然后重新唤醒。';

  @override
  String get clipboardPermissionHelp => '请在系统设置中允许访问剪贴板，然后重新唤醒 Anywhere。';

  @override
  String get appChangedHelp => '原应用已退出或重启，请重新唤醒 Anywhere 以再次采集。';

  @override
  String get tooManyItemsHelp => '请在访达中选择不超过 32 个项目，然后重新唤醒 Anywhere。';

  @override
  String get loadingMoreFiles => '正在加载更多文件…';

  @override
  String get authorizeSearchDirectories => '授权搜索目录';

  @override
  String get authorizeSearchHelp => '请在“设置 → 搜索”中授权已保存的目录。权限准备完成后才会开始搜索。';

  @override
  String get fileSearchOff => '文件搜索已关闭';

  @override
  String get searchUnavailable => '搜索不可用';

  @override
  String get partialResults => '部分结果';

  @override
  String get noMatchingFiles => '没有匹配的文件';

  @override
  String get noContentActions => '此内容没有可用动作';

  @override
  String get searchingFiles => '正在搜索文件…';

  @override
  String get fileSearchUnavailable => '文件搜索不可用';

  @override
  String get fileSearchDisabledHelp => '文件搜索已关闭，可在“设置 → 搜索”中启用。';

  @override
  String get typeFileName => '输入文件或文件夹名称开始搜索';

  @override
  String get fileSearchIncompleteHelp => '文件搜索未完成，请查看问题详情后重试。';

  @override
  String get noMatchingActions => '没有匹配的动作';

  @override
  String get noActionsAvailable => '没有可用动作';

  @override
  String get fileSearchIncomplete => '文件搜索未完成，已找到的结果仍然可用。';

  @override
  String get actionsLoadIncomplete => '部分动作未能加载，请重新唤醒 Anywhere 后重试；其他动作仍可使用。';

  @override
  String get fileSearchOtherResults => '文件搜索不可用 · 其他结果仍可使用';

  @override
  String get currentPage => '当前网页';

  @override
  String get chooseTarget => '选择目标';

  @override
  String get contentActions => '此内容的动作';

  @override
  String get searchComplete => '搜索完成';

  @override
  String get clipboardSettingsFailed => '无法读取或更新剪贴板设置，请重试。';

  @override
  String get historyLoadFailed => '无法加载剪贴板历史，请刷新重试。';

  @override
  String get historyEntryMissing => '此记录已不可用，请刷新历史。';

  @override
  String get historyPreviewFailed => '无法预览此记录。';

  @override
  String get historyOperationFailed => '操作未完成，请刷新后重试。';

  @override
  String get historyCopyFailed => '无法复制此记录。如为文件，请确认原文件仍然存在且可访问。';

  @override
  String sourceUnsupportedHelp(String source) {
    return '此应用未提供$source，其他来源仍可使用。';
  }

  @override
  String sourceTimeoutHelp(String source) {
    return '$source响应超时，请重新唤醒 Anywhere 后重试。';
  }

  @override
  String sourceTooLongHelp(String source) {
    return '$source内容过长，请减少内容后重新唤醒 Anywhere。';
  }

  @override
  String sourceFailedHelp(String source) {
    return '无法读取$source，请重新唤醒 Anywhere 后重试；其他来源仍可使用。';
  }

  @override
  String fileFoundCount(int count) {
    return '找到 $count 项';
  }

  @override
  String searchIssues(int count) {
    return '$count 个问题';
  }

  @override
  String fileSearchIssueNotice(int count) {
    return '文件搜索遇到 $count 个问题，已找到的结果仍然可用。';
  }

  @override
  String filterActions(String scope) {
    return '筛选$scope动作';
  }

  @override
  String emptyFileType(String type) {
    return '结果中没有$type，请尝试“所有类型”或其他关键词。';
  }

  @override
  String fileRootsHint(String example) {
    return '$example\n留空则搜索个人主目录';
  }

  @override
  String runningCount(int count) {
    return '$count 项运行中';
  }

  @override
  String exitCode(String code) {
    return '退出码 $code';
  }

  @override
  String taskKeyboardHelp(String find, String stop) {
    return '↑↓ 选择任务   $find 查找   $stop 停止   Esc 隐藏';
  }

  @override
  String shortcutHint(String label, String shortcut) {
    return '$label · $shortcut';
  }

  @override
  String enableFeature(String name) {
    return '启用$name';
  }

  @override
  String allowFeature(String name) {
    return '允许$name？';
  }

  @override
  String removeRule(int number) {
    return '移除第 $number 条规则';
  }

  @override
  String contextRulesCount(int count) {
    return '$count 条上下文规则';
  }

  @override
  String editActionShortcut(String title) {
    return '编辑“$title”的快捷键';
  }

  @override
  String removeActionShortcut(String title) {
    return '移除“$title”的快捷键';
  }

  @override
  String get results => '结果';

  @override
  String get succeeded => '成功';

  @override
  String get input => '输入';

  @override
  String get web => '网页';

  @override
  String get tools => '工具';

  @override
  String restrictedLocationsSkipped(String count) {
    return '已跳过 $count 个受限位置';
  }

  @override
  String clipboardConsent(String tool) {
    return '$tool请求执行剪贴板操作。授权仅对本次操作有效。';
  }

  @override
  String get quitAnywhere => '退出 Anywhere';

  @override
  String get edit => '编辑';

  @override
  String get undo => '撤销';

  @override
  String get redo => '重做';

  @override
  String get cut => '剪切';

  @override
  String get paste => '粘贴';

  @override
  String get selectAll => '全选';

  @override
  String get window => '窗口';

  @override
  String get minimize => '最小化';

  @override
  String get zoom => '缩放';

  @override
  String get closeWindow => '关闭窗口';

  @override
  String get findTaskMenu => '查找任务…';

  @override
  String get stopSelectedTask => '停止所选任务';

  @override
  String get refreshTasks => '刷新任务';

  @override
  String get openLauncher => '打开启动器';

  @override
  String get closeTaskWindow => '关闭任务窗口';

  @override
  String get runningTasksMenu => '运行中的任务…';

  @override
  String get settingsMenu => '设置…';

  @override
  String get authorizeFileSearch => '授权文件搜索';

  @override
  String get authorize => '授权';

  @override
  String get importLocalWebTool => '导入本地 Web 工具';

  @override
  String get importPackage => '导入';

  @override
  String get fileSearchAccessMessage => '请选择允许 Anywhere 搜索的文件夹，授权会保存供以后启动时使用。受保护的系统或应用数据可能仍需在系统设置中启用完全磁盘访问权限。';

  @override
  String get inspectTrustedPackage => '请选择可信的 .awplugin 包，在安装前检查其内容。';

  @override
  String get readClipboardText => '读取剪贴板文本';

  @override
  String get copyTextToClipboard => '将文本复制到剪贴板';

  @override
  String get permissionCurrentApp => '当前应用名称';

  @override
  String get permissionReadClipboard => '读取剪贴板文本（每次操作均需确认）';

  @override
  String get permissionWriteClipboard => '复制文本（每次操作均需确认）';

  @override
  String get permissionLocalStorage => '独立本地设置（1 MiB）';

  @override
  String get launcherStartupFailed => '启动器无法启动';

  @override
  String get tasksStartupFailed => '无法打开运行中的任务';

  @override
  String get shelfStartupFailed => '无法打开剪贴板悬浮窗';

  @override
  String get savedDirectories => '收藏的文件夹';

  @override
  String get browserPageDescription => '使用 Safari、Chrome 或 Edge 当前标签页的网页地址';

  @override
  String permissionContextRead(String types) {
    return '选中的上下文：$types';
  }

  @override
  String get supportedBrowserDescription => '支持的浏览器当前页面的网址';

  @override
  String installToolReview(String name) {
    return '安装$name？';
  }

  @override
  String updateToolReview(String name) {
    return '更新$name？';
  }

  @override
  String get paused => '已暂停';

  @override
  String get detail => '详情';

  @override
  String formatsCount(int count) {
    return '$count 种格式';
  }

  @override
  String get automationUsage => '在唤醒 Anywhere 时读取访达当前位置和所选项目，以及 Safari、Chrome 或 Edge 当前标签页的网址，以推荐相关动作。';

  @override
  String contentPreview(String kind) {
    return '$kind预览';
  }

  @override
  String savedSize(String size) {
    return '已保存 $size';
  }

  @override
  String get invalidActionIdentity => '动作标识无效。';

  @override
  String get letterWithModifier => '请使用修饰键加字母键。';

  @override
  String get modifierWithLetter => '请为字母键添加修饰键。';

  @override
  String get reservedEditingShortcut => '此快捷键用于编辑或窗口操作，无法重新绑定。';

  @override
  String get invalidShortcutIntent => '上下文快捷键标识无效。';

  @override
  String get shortcutRuleLimit => '请填写快捷键名称，且最多设置 16 条规则。';

  @override
  String get differentShortcutConditions => '每条规则的应用和上下文条件必须不同。';

  @override
  String get shortcutAlreadyOpens => '此快捷键已用于唤醒 Anywhere。';

  @override
  String get invalidShortcutRule => '快捷键规则无效。';

  @override
  String get searchDirectoryLimits => '最多使用 8 个目录和 32 个排除项。';

  @override
  String get absoluteSearchDirectories => '搜索目录必须使用绝对路径，每行一个。';

  @override
  String get unsupportedShortcutSettings => '不支持此版本的动作快捷键设置，请先更新 Anywhere。';

  @override
  String get oneShortcutPerAction => '每个动作只能绑定一个快捷键。';

  @override
  String get shortcutsRefreshFailed => '无法刷新动作快捷键，请重新打开设置后重试。';

  @override
  String shortcutAssignedTo(String title) {
    return '此快捷键已绑定到“$title”。';
  }

  @override
  String maxActionShortcuts(int count) {
    return '最多支持 $count 个动作快捷键。';
  }

  @override
  String get shortcutsSavedByNewerVersion => '动作快捷键由更新版本保存，请先更新 Anywhere 再修改设置。';

  @override
  String get directoryAccessFailed => '无法准备目录访问权限，请在搜索设置中重试。';

  @override
  String get openSystemSettingsFailed => '无法打开系统设置，请手动打开。';

  @override
  String get shortcutsRestoreFailed => '无法恢复动作快捷键，请修改冲突的快捷键或尝试再次关闭。';

  @override
  String extensionsLoadFailed(String detail) {
    return '无法加载扩展：$detail';
  }

  @override
  String extensionsUpdateFailed(String detail) {
    return '无法更新扩展设置：$detail';
  }

  @override
  String shortcutLoadFailed(String detail) {
    return '无法加载动作快捷键：$detail';
  }

  @override
  String get shortcutsPausedHelp => '部分动作快捷键已暂停，请重新打开设置重试，或改用其他按键组合。';

  @override
  String get shelfSelectHint => '单击选择 · 双击复制';

  @override
  String get shelfCopyAndReturn => '复制并返回原应用';

  @override
  String get shelfDeleteTitle => '删除这条历史记录？';

  @override
  String get shelfDeleteDescription => '仅删除这条已保存的历史记录。';

  @override
  String get hidePreview => '收起预览';

  @override
  String shelfPreviewScrollHint(String shortcut) {
    return '$shortcut 滚动预览';
  }

  @override
  String get details => '详细信息';

  @override
  String get sourceStatus => '来源状态';

  @override
  String get searchRules => '搜索规则';

  @override
  String get searchHelp => '搜索说明';

  @override
  String get toolOptions => '工具选项';

  @override
  String get keyboardHelp => '键盘操作';

  @override
  String get fileInformation => '文件信息';

  @override
  String get historyFeatureDisabled => '剪贴板历史已在内置功能中禁用。';

  @override
  String get historyStorageUnavailable => '剪贴板历史存储不可用。请修复数据文件后重启。';

  @override
  String get historyAccessRequired => '请在系统设置中允许访问剪贴板，以记录新复制的内容。后台采集不会弹出权限申请。';

  @override
  String get historyItemTooLarge => '复制的内容超过 16 MiB 历史记录上限，未保存。';

  @override
  String get historyReadFailed => '无法读取一条剪贴板内容，已有历史仍可使用。';

  @override
  String get historyCaptureUnavailable => '剪贴板历史采集不可用，已有历史仍可使用。';

  @override
  String get historyMonitoringUnavailable => '当前平台不支持剪贴板监控。';

  @override
  String get filePreviewChanged => '无法预览。如果文件已发生变化，请重新搜索。';

  @override
  String get filePreviewUnsupported => '此文件类型不支持文本预览';

  @override
  String filePreviewTruncated(String size) {
    return '仅显示前 $size';
  }

  @override
  String filePreviewEncodingGuessed(String encoding) {
    return '推测编码为 $encoding';
  }

  @override
  String filePreviewDecoded(String encoding) {
    return '以 $encoding 解码';
  }

  @override
  String get shelfReturnFailed => '已复制，但未能返回目标应用。请切换到该应用后粘贴。';

  @override
  String get shelfReturnUnavailable => '已复制，原目标应用已不可用。请切换到需要的应用后粘贴。';

  @override
  String get historyRecordingPaused => '历史记录已暂停采集，已有历史仍可使用。';

  @override
  String get pauseHistoryRecording => '暂时停止采集，保留已有历史';

  @override
  String historyPausedUntil(String time) {
    return '暂停至 $time';
  }

  @override
  String get resumeRecording => '恢复记录';

  @override
  String get pauseFor => '暂停…';

  @override
  String pauseMinutes(int minutes) {
    return '$minutes 分钟';
  }

  @override
  String get keepHistoryFor => '历史保留时间';

  @override
  String get untilStorageLimit => '直到达到容量上限';

  @override
  String retentionDays(int days) {
    return '$days 天';
  }

  @override
  String get changeHistoryRetention => '更改历史保留时间？';

  @override
  String get historyRetentionWarning => '超过所选期限的历史将立即永久删除。之后延长保留时间也无法恢复。';

  @override
  String historyStorageUsage(int count, String mib) {
    return '已保存 $count 条 · $mib MiB';
  }

  @override
  String get historyStorageExplanation => '仅保存在本机，未加密。最多 1,000 条 / 128 MiB 逻辑数据，数据库文件大小可能不同。过期会删除全部已存格式，不影响系统剪贴板和原始文件。';

  @override
  String get appHistory => '应用历史';

  @override
  String get appHistorySearch => '搜索最近项目和文件…';

  @override
  String get appHistoryFilterSources => '筛选应用…';

  @override
  String get appHistoryAllSources => '全部来源';

  @override
  String get appHistoryShowAllSources => '清除筛选并显示全部来源';

  @override
  String get appHistoryRevertPath => '还原编辑';

  @override
  String get appHistoryUnsavedPath => '路径未保存';

  @override
  String get appHistorySources => '历史来源';

  @override
  String get appHistorySourceDescription => '默认仅开启已安装应用的 Action，手动开关优先。可自定义历史文件路径，使用对应 Action 时才读取历史内容。';

  @override
  String get appHistorySourcePath => '历史元数据文件路径';

  @override
  String get appHistoryUnconfigured => '暂无可用的历史文件，请在「设置 → 应用历史」中重新检测或指定自定义路径。';

  @override
  String get appHistoryReadFailed => '部分来源暂时无法读取，请在「设置 → 应用历史」中检查所选文件。';

  @override
  String get appHistoryBusy => '正在结束上一次历史操作，请稍后刷新重试。';

  @override
  String get appHistoryExpired => '此历史页面已过期，请返回并重新打开相应操作。';

  @override
  String get appHistoryDisabled => '应用历史已停用，可在「设置 → 功能」中启用。';

  @override
  String get appHistoryOperationFailed => '历史操作未完成，请刷新列表后重试。';

  @override
  String get appHistorySettingsFailed => '无法读取或保存历史来源，请检查文件路径后重试。';

  @override
  String get appHistoryCopyTarget => '复制路径或 URI';

  @override
  String get appHistoryAutomatic => '正在使用自动检测的文件';

  @override
  String get appHistoryCustom => '正在使用自定义路径';

  @override
  String get appHistorySourceOff => '来源已关闭';

  @override
  String get appHistoryNotDetected => '未自动找到文件，可填写自定义路径。';

  @override
  String get appHistoryUseAutomatic => '使用自动检测';

  @override
  String get appHistoryDetectAgain => '重新检测';

  @override
  String get appHistorySaveCustom => '保存自定义路径';

  @override
  String get appHistoryActionEnabled => '显示 Action';

  @override
  String get appHistoryCustomAdapters => '自定义适配';

  @override
  String get appHistoryCustomAdaptersHelp => '在下列目录的子文件夹中放入 source.json 和 transform.js，重启后生效。同 ID 配置覆盖内置适配。请只放入可信的本机配置；使用对应 Action 时才运行转换。';

  @override
  String get appHistoryOpenAdapters => '打开适配目录';

  @override
  String get permissionInputRead => '读取启动器搜索内容';

  @override
  String get permissionInputWrite => '修改启动器搜索内容';

  @override
  String get permissionFrontmostApp => '读取实时前台应用';
}
