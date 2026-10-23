import Foundation

/// Native chrome uses Apple's Bundle localization and the same ARB source as Flutter.
/// This preference never reaches Core, action identity or captured application content.
enum NativeLocalization {
  static var preference = "system"

  static func language(preference: String, preferred: [String]) -> String {
    if preference == "en" || preference == "zh" { return preference }
    for identifier in preferred {
      let code = identifier.replacingOccurrences(of: "_", with: "-")
        .split(separator: "-").first.map(String.init)
      if code == "en" || code == "zh" { return code! }
    }
    return "en"
  }

  static func text(_ key: NativeText, bundle: Bundle = .main) -> String {
    let language = language(preference: preference, preferred: Locale.preferredLanguages)
    let folder = language == "zh" ? "zh-Hans" : "en"
    guard let path = bundle.path(forResource: folder, ofType: "lproj"),
      let localized = Bundle(path: path)
    else { return key.english }
    return localized.localizedString(forKey: key.rawValue, value: key.english, table: "Anywhere")
  }
}
