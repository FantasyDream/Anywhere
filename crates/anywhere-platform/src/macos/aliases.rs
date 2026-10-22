//! Compatibility aliases for macOS bundle identities. Core receives ordinary aliases.
pub(super) fn well_known_aliases(bundle_id: &str) -> &'static [&'static str] {
    match bundle_id {
        "com.apple.finder" => &["访达"],
        "com.apple.Safari" => &["浏览器"],
        "com.google.Chrome" => &["谷歌浏览器", "Chrome"],
        "com.google.Chrome.canary" => &["谷歌浏览器"],
        "com.apple.Terminal" => &["终端"],
        "com.googlecode.iterm2" => &["终端"],
        "com.microsoft.VSCode" => &["代码"],
        "com.apple.Notes" => &["备忘录"],
        "com.apple.mail" => &["邮件"],
        "com.apple.iCal" => &["日历"],
        "com.apple.Preview" => &["预览"],
        "com.apple.Music" => &["音乐"],
        "com.apple.Photos" => &["照片"],
        "com.apple.MobileSMS" => &["信息"],
        "com.apple.calculator" => &["计算器"],
        "com.apple.Maps" => &["地图"],
        "com.apple.AppStore" => &["应用商店"],
        "com.apple.systempreferences" | "com.apple.systemsettings" => &["系统设置", "系统偏好设置"],
        "com.apple.TextEdit" => &["文本编辑"],
        "com.tencent.xinWeChat" => &["微信"],
        "com.tencent.qq" => &["QQ"],
        "com.alibaba.DingTalkMac" => &["钉钉"],
        "com.larksuite.ingee" | "com.electron.lark" | "com.lark.suite" => &["飞书"],
        "ru.keepcoder.Telegram" => &["电报"],
        "com.tinyspeck.slackmacgap" => &["Slack"],
        "com.apple.dt.Xcode" => &["Xcode"],
        "com.apple.iWork.Pages" => &["文稿"],
        "com.apple.iWork.Numbers" => &["表格"],
        "com.apple.iWork.Keynote" => &["演示"],
        "com.microsoft.Word" => &["Word"],
        "com.microsoft.Excel" => &["Excel"],
        "com.microsoft.Powerpoint" => &["PPT"],
        "us.zoom.xos" => &["Zoom"],
        "notion.id" => &["Notion"],
        _ => &[],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn existing_localized_search_aliases_are_preserved() {
        assert!(well_known_aliases("com.apple.finder").contains(&"访达"));
        assert!(well_known_aliases("com.google.Chrome").contains(&"谷歌浏览器"));
        assert!(well_known_aliases("com.lark.suite").contains(&"飞书"));
        assert!(well_known_aliases("test.unknown").is_empty());
    }
}
