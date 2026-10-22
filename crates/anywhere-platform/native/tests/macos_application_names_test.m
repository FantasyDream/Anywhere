// Metadata only. No app launches, Accessibility access or language preference writes.
#import <AppKit/AppKit.h>
#include <assert.h>
#include "../macos.m"

static NSArray<NSString *> *names(NSString *path, NSString *languages) {
    NSBundle *bundle = [NSBundle bundleWithPath:path];
    if (!bundle) return @[];
    return launcher_names_for_languages(bundle, [languages componentsSeparatedByString:@","]);
}

int main(void) {
    @autoreleasepool {
        NSString *root = [NSTemporaryDirectory() stringByAppendingPathComponent:NSUUID.UUID.UUIDString];
        NSString *path = [root stringByAppendingPathComponent:@"Test.app"];
        NSString *contents = [path stringByAppendingPathComponent:@"Contents"];
        NSFileManager *files = NSFileManager.defaultManager;
        for (NSString *locale in @[ @"en", @"zh-Hans" ]) {
            NSString *directory = [contents stringByAppendingPathComponent:
                [NSString stringWithFormat:@"Resources/%@.lproj", locale]];
            assert([files createDirectoryAtPath:directory withIntermediateDirectories:YES attributes:nil error:NULL]);
            NSDictionary *strings = [locale isEqualToString:@"en"]
                ? @{ @"CFBundleDisplayName": @"Test Application", @"CFBundleName": @"Test" }
                : @{ @"CFBundleDisplayName": @"测试应用", @"CFBundleName": @"测试" };
            assert([strings writeToFile:[directory stringByAppendingPathComponent:@"InfoPlist.strings"] atomically:YES]);
        }
        assert(([@{ @"CFBundleIdentifier": @"test.anywhere.localized-names",
                   @"CFBundleName": @"Test", @"CFBundlePackageType": @"APPL" }
            writeToFile:[contents stringByAppendingPathComponent:@"Info.plist"] atomically:YES]));
        assert([names(path, @"zh-Hans-CN").firstObject isEqualToString:@"测试应用"]);
        assert([names(path, @"en").firstObject isEqualToString:@"Test Application"]);
        // Read every requested language, not just the first language that produced a name.
        NSArray *both = names(path, @"zh-Hans,en");
        assert([both containsObject:@"测试应用"] && [both containsObject:@"Test Application"]);
        assert([names(path, @"en,zh-Hans").firstObject isEqualToString:@"Test Application"]);
        char *json = launcher_bundle_application(path.UTF8String);
        assert(json);
        NSDictionary *metadata = [NSJSONSerialization JSONObjectWithData:
            [[NSString stringWithUTF8String:json] dataUsingEncoding:NSUTF8StringEncoding] options:0 error:NULL];
        launcher_free_string(json);
        assert([metadata[@"localized_names"][@"zh"] isEqualToString:@"测试应用"]);
        assert([metadata[@"localized_names"][@"en"] isEqualToString:@"Test Application"]);
        assert([metadata[@"path"] isEqualToString:path]);
        assert(names(@"/nonexistent/Anywhere.app", @"zh").count == 0);
        assert([files removeItemAtPath:root error:NULL]);

#if __MAC_OS_X_VERSION_MAX_ALLOWED >= 150400 && !defined(ANYWHERE_TEST_LEGACY_SDK)
        if (@available(macOS 15.4, *)) {
            // Real system apps use InfoPlist.loctable, not the legacy fixture format.
            NSDictionary *apps = @{
                @"/System/Applications/Reminders.app": @"提醒事项",
                @"/System/Applications/Utilities/Activity Monitor.app": @"活动监视器",
                @"/System/Applications/Calculator.app": @"计算器",
            };
            for (NSString *app in apps) {
                if (![files fileExistsAtPath:app]) continue;
                assert([names(app, @"zh-Hans").firstObject isEqualToString:apps[app]]);
                char *raw = launcher_bundle_application(app.UTF8String);
                assert(raw);
                NSDictionary *value = [NSJSONSerialization JSONObjectWithData:
                    [[NSString stringWithUTF8String:raw] dataUsingEncoding:NSUTF8StringEncoding] options:0 error:NULL];
                launcher_free_string(raw);
                assert([value[@"localized_names"][@"zh"] isEqualToString:apps[app]]);
                assert([value[@"localized_names"][@"en"] length] > 0);
                printf("%s: %s / %s\n", app.UTF8String,
                    [value[@"localized_names"][@"zh"] UTF8String],
                    [value[@"localized_names"][@"en"] UTF8String]);
            }
        }
#endif
    }
    return 0;
}
