// Read bundle-supplied names without changing the process's language preferences.
#import <Foundation/Foundation.h>

static NSArray<NSString *> *launcher_names_for_languages(
    NSBundle *bundle, NSArray<NSString *> *languages) {
    NSMutableOrderedSet<NSString *> *names = [NSMutableOrderedSet orderedSet];
    for (NSString *language in languages) {
        NSString *preference = [language
            stringByTrimmingCharactersInSet:NSCharacterSet.whitespaceAndNewlineCharacterSet];
        if (preference.length == 0) continue;
        NSArray<NSString *> *keys = @[ @"CFBundleDisplayName", @"CFBundleName" ];
#if __MAC_OS_X_VERSION_MAX_ALLOWED >= 150400 && !defined(ANYWHERE_TEST_LEGACY_SDK)
        if (@available(macOS 15.4, *)) {
            // This public API also reads InfoPlist.loctable, used by modern system apps.
            for (NSString *key in keys) {
                NSString *missing = @"__anywhere_missing_bundle_name__";
                NSString *name = [bundle localizedStringForKey:key value:missing
                    table:@"InfoPlist" localizations:@[ preference ]];
                if (name.length > 0 && ![name isEqualToString:missing]) [names addObject:name];
            }
            continue;
        }
#endif
        // Older systems store these names in localized InfoPlist.strings files.
        NSArray<NSString *> *matches = [NSBundle
            preferredLocalizationsFromArray:bundle.localizations forPreferences:@[ preference ]];
        for (NSString *localization in matches) {
            // The Bundle selector may return an unrelated fallback when no language matches.
            NSString *requested = [NSLocale componentsFromLocaleIdentifier:preference][NSLocaleLanguageCode];
            NSString *selected = [NSLocale componentsFromLocaleIdentifier:localization][NSLocaleLanguageCode];
            if (![requested isEqualToString:selected] &&
                !([requested isEqualToString:@"en"] && [localization isEqualToString:@"English"])) continue;
            NSURL *url = [bundle URLForResource:@"InfoPlist" withExtension:@"strings"
                                  subdirectory:nil localization:localization];
            if (!url) continue;
            NSData *data = [NSData dataWithContentsOfURL:url];
            if (!data) continue;
            id entries = [NSPropertyListSerialization propertyListWithData:data
                options:NSPropertyListImmutable format:NULL error:NULL];
            if (![entries isKindOfClass:NSDictionary.class]) continue;
            for (NSString *key in keys) {
                id name = entries[key];
                if ([name isKindOfClass:NSString.class] && [name length] > 0) [names addObject:name];
            }
        }
    }
    return names.array;
}
