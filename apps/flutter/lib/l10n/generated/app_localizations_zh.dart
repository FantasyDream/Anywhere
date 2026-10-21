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
  String get navigate => '移动选择';

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
  String get noMatches => '无匹配项';

  @override
  String get noHistory => '暂无剪贴板历史';

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
  String get clipboardAgeDescription => '仅自动填入 15 秒内复制的内容';

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
  String get searchDrafts => '搜索设置存在尚未应用的修改。';

  @override
  String get settingsSaveFailed => '设置保存失败，请重试。';

  @override
  String get shortcutHelp => '选择快捷键，再按下新的按键组合。按 Esc 取消。';

  @override
  String get pressShortcut => '请按下快捷键…';

  @override
  String get workspace => '工作区';

  @override
  String get localProcessing => '本地处理';

  @override
  String get webTool => 'Web 工具';

  @override
  String get reload => '重新加载';

  @override
  String get image => '图片';

  @override
  String get richText => '富文本';

  @override
  String get data => '数据';

  @override
  String get text => '文本';

  @override
  String get pin => '置顶';

  @override
  String get unpin => '取消置顶';

  @override
  String get copiedContent => '已复制的内容';

  @override
  String resultsCount(int count) {
    return '$count 个结果';
  }

  @override
  String entriesCount(int count) {
    return '$count 条记录';
  }

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
}
