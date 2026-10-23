import Foundation

@main
struct NativeLocalizationTests {
  static func main() throws {
    for (preference, preferred, expected) in [
      ("zh", ["en-US"], "zh"), ("en", ["zh-Hans-CN"], "en"),
      ("system", ["ja-JP", "zh-TW"], "zh"),
      ("system", ["en-GB", "zh-Hans"], "en"),
      ("system", ["zh_CN", "en"], "zh"),
      ("system", ["ja"], "en"), ("unknown", [], "en"),
    ] {
      precondition(
        NativeLocalization.language(preference: preference, preferred: preferred) == expected
      )
    }
    let files = FileManager.default
    let temporary = files.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? files.removeItem(at: temporary) }
    let contents = temporary.appendingPathComponent("LocaleFixture.bundle/Contents")
    let resources = contents.appendingPathComponent("Resources")
    try files.createDirectory(at: resources, withIntermediateDirectories: true)
    let info = [
      "CFBundleIdentifier": "test.anywhere.native-localization", "CFBundlePackageType": "BNDL",
    ]
    let data = try PropertyListSerialization.data(fromPropertyList: info, format: .xml, options: 0)
    try data.write(to: contents.appendingPathComponent("Info.plist"))
    let source = URL(fileURLWithPath: CommandLine.arguments[1])
    for language in ["en", "zh-Hans"] {
      try files.copyItem(
        at: source.appendingPathComponent("\(language).lproj"),
        to: resources.appendingPathComponent("\(language).lproj")
      )
    }
    let bundle = Bundle(url: contents.deletingLastPathComponent())!
    NativeLocalization.preference = "zh"
    precondition(NativeLocalization.text(.settingsMenu, bundle: bundle) == "设置…")
    precondition(NativeLocalization.text(.copy, bundle: bundle) == "复制")
    NativeLocalization.preference = "en"
    precondition(NativeLocalization.text(.settingsMenu, bundle: bundle) == "Settings…")
    precondition(NativeLocalization.text(.copy, bundle: bundle) == "Copy")
    // Missing resources fall back to the matching source label, never a raw key.
    precondition(NativeLocalization.text(.settingsMenu) == "Settings…")
    print(
      "Native localization: preferences, region fallback, Apple bundles and live switching passed"
    )
  }
}
