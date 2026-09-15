#import <AppKit/AppKit.h>
#include <stdlib.h>
#include <string.h>

// Only OS context capture. All window/UI code lives in the Flutter host.
char *launcher_frontmost_application(void) {
    @autoreleasepool {
        NSRunningApplication *application = NSWorkspace.sharedWorkspace.frontmostApplication;
        if (!application) return NULL;
        NSDictionary *value = @{
            @"bundle_id": application.bundleIdentifier ?: @"",
            @"name": application.localizedName ?: @"Unknown application"
        };
        NSData *data = [NSJSONSerialization dataWithJSONObject:value options:0 error:NULL];
        if (!data) return NULL;
        NSString *json = [[NSString alloc] initWithData:data encoding:NSUTF8StringEncoding];
        return strdup(json.UTF8String);
    }
}

void launcher_free_string(char *value) { free(value); }
void launcher_free_bytes(unsigned char *value) { free(value); }

// One installed application bundle. Metadata only: no icon is extracted here.
char *launcher_bundle_application(const char *utf8_path) {
    @autoreleasepool {
        if (!utf8_path) return NULL;
        NSString *path = [NSString stringWithUTF8String:utf8_path];
        if (!path) return NULL;
        NSBundle *bundle = [NSBundle bundleWithPath:path];
        if (!bundle) return NULL;
        NSString *identifier = bundle.bundleIdentifier;
        if (identifier.length == 0) return NULL;
        NSString *name = bundle.infoDictionary[@"CFBundleDisplayName"];
        if (![name isKindOfClass:NSString.class] || name.length == 0) {
            name = bundle.infoDictionary[@"CFBundleName"];
        }
        if (![name isKindOfClass:NSString.class] || name.length == 0) {
            name = path.lastPathComponent.stringByDeletingPathExtension;
        }
        NSDictionary *value = @{
            @"bundle_id": identifier,
            @"name": name,
            @"path": bundle.bundlePath ?: path,
        };
        NSData *data = [NSJSONSerialization dataWithJSONObject:value options:0 error:NULL];
        if (!data) return NULL;
        NSString *json = [[NSString alloc] initWithData:data encoding:NSUTF8StringEncoding];
        return json ? strdup(json.UTF8String) : NULL;
    }
}

// The user's preferred languages, most preferred first, as a JSON array.
//
// Read from the system rather than from an environment variable: a Finder-launched
// app has no AppleLanguages in its environment.
char *launcher_preferred_languages(void) {
    @autoreleasepool {
        NSArray<NSString *> *languages = NSLocale.preferredLanguages;
        NSData *data = [NSJSONSerialization dataWithJSONObject:languages options:0 error:NULL];
        if (!data) return NULL;
        NSString *json = [[NSString alloc] initWithData:data encoding:NSUTF8StringEncoding];
        return json ? strdup(json.UTF8String) : NULL;
    }
}

// Names for one bundle in the given preference languages, as a JSON array.
//
// macOS keeps localized app names in <lang>.lproj/InfoPlist.strings, which
// `bundleWithPath` does not surface for a bundle outside the running app. Without
// this, searching for "Safari浏览器" found nothing: the index only ever held the
// canonical English name.
char *launcher_bundle_localized_names(const char *utf8_path, const char *utf8_languages) {
    @autoreleasepool {
        if (!utf8_path || !utf8_languages) return NULL;
        NSString *path = [NSString stringWithUTF8String:utf8_path];
        NSArray<NSString *> *languages =
            [[NSString stringWithUTF8String:utf8_languages] componentsSeparatedByString:@","];
        if (path.length == 0 || languages.count == 0) return NULL;
        NSString *resources = [path stringByAppendingPathComponent:@"Contents/Resources"];
        NSFileManager *files = NSFileManager.defaultManager;
        NSMutableArray<NSString *> *names = [NSMutableArray array];
        NSMutableSet<NSString *> *seen = [NSMutableSet set];
        for (NSString *language in languages) {
            NSString *trimmed = [language
                stringByTrimmingCharactersInSet:NSCharacterSet.whitespaceAndNewlineCharacterSet];
            if (trimmed.length == 0) continue;
            // "zh-Hans-CN" also has to match "zh-Hans", "zh_CN" and "zh-Hans-CN".
            NSMutableArray<NSString *> *candidates = [NSMutableArray arrayWithObject:trimmed];
            NSArray<NSString *> *parts = [trimmed componentsSeparatedByString:@"-"];
            if (parts.count >= 2) {
                NSString *script = parts[1];
                if ([script isEqualToString:@"Hans"]) {
                    [candidates addObject:@"zh_CN"];
                    [candidates addObject:@"zh-Hans"];
                } else if ([script isEqualToString:@"Hant"]) {
                    [candidates addObject:@"zh_TW"];
                    [candidates addObject:@"zh_HK"];
                    [candidates addObject:@"zh-Hant"];
                }
                [candidates addObject:[NSString stringWithFormat:@"%@_%@", parts[0], parts[1]]];
            }
            for (NSString *candidate in candidates) {
                NSString *strings = [[resources
                    stringByAppendingPathComponent:[candidate stringByAppendingPathExtension:@"lproj"]]
                    stringByAppendingPathComponent:@"InfoPlist.strings"];
                if (![files fileExistsAtPath:strings]) continue;
                NSDictionary *entries = [NSDictionary dictionaryWithContentsOfFile:strings];
                if (![entries isKindOfClass:NSDictionary.class]) continue;
                for (NSString *key in @[ @"CFBundleDisplayName", @"CFBundleName" ]) {
                    NSString *value = entries[key];
                    if (![value isKindOfClass:NSString.class] || value.length == 0) continue;
                    if (![seen containsObject:value]) {
                        [seen addObject:value];
                        [names addObject:value];
                    }
                }
                if (names.count > 0) break;
            }
        }
        if (names.count == 0) return NULL;
        NSData *data = [NSJSONSerialization dataWithJSONObject:names options:0 error:NULL];
        if (!data) return NULL;
        NSString *json = [[NSString alloc] initWithData:data encoding:NSUTF8StringEncoding];
        return json ? strdup(json.UTF8String) : NULL;
    }
}

// Activate a running instance, otherwise launch the canonical bundle path.
// Returns 1 when an application was activated or launched, 0 when nothing happened.
int launcher_launch_application(const char *utf8_path) {
    @autoreleasepool {
        if (!utf8_path) return 0;
        NSString *path = [NSString stringWithUTF8String:utf8_path];
        if (path.length == 0) return 0;
        NSURL *url = [NSURL fileURLWithPath:path];
        NSString *target = [[NSBundle bundleWithURL:url] bundleIdentifier];
        if (target.length > 0) {
            for (NSRunningApplication *running in NSWorkspace.sharedWorkspace.runningApplications) {
                // Match the chosen install so a duplicate installation is not activated.
                if ([running.bundleIdentifier isEqualToString:target] &&
                    [running.bundleURL.path isEqualToString:path]) {
                    return [running activateWithOptions:NSApplicationActivateAllWindows] ? 1 : 0;
                }
            }
        }
        NSWorkspaceOpenConfiguration *configuration = [NSWorkspaceOpenConfiguration configuration];
        configuration.activates = YES;
        __block int launched = 0;
        dispatch_semaphore_t done = dispatch_semaphore_create(0);
        [NSWorkspace.sharedWorkspace openApplicationAtURL:url
                                           configuration:configuration
                                       completionHandler:^(NSRunningApplication *application,
                                                           NSError *error) {
            launched = (application != nil && error == nil) ? 1 : 0;
            dispatch_semaphore_signal(done);
        }];
        // The completion handler runs off the main thread; a bounded wait keeps this
        // call synchronous without blocking the launcher for long.
        dispatch_semaphore_wait(done,
                                dispatch_time(DISPATCH_TIME_NOW, (int64_t)(3 * NSEC_PER_SEC)));
        return launched;
    }
}

// Small PNG icon for a bundle. The caller frees the returned buffer.
unsigned char *launcher_application_icon(const char *utf8_path, unsigned int size,
                                         unsigned int *out_length) {
    @autoreleasepool {
        if (out_length) *out_length = 0;
        if (!utf8_path || size == 0 || size > 512) return NULL;
        NSString *path = [NSString stringWithUTF8String:utf8_path];
        if (path.length == 0) return NULL;
        // Prefer the bundle's own icon file: `iconForFile` can hand back a stale or
        // generic icon for an app LaunchServices has not indexed yet, which is what a
        // freshly installed browser showed us. Fall back to the workspace lookup for
        // bundles that keep their icon in Assets.car.
        NSImage *image = nil;
        NSBundle *bundle = [NSBundle bundleWithPath:path];
        NSString *iconFile = bundle.infoDictionary[@"CFBundleIconFile"];
        if ([iconFile isKindOfClass:NSString.class] && iconFile.length > 0) {
            NSString *name = iconFile.pathExtension.length > 0 ? iconFile
                                                               : [iconFile stringByAppendingPathExtension:@"icns"];
            NSString *iconPath = [[path stringByAppendingPathComponent:@"Contents/Resources"]
                stringByAppendingPathComponent:name];
            if ([NSFileManager.defaultManager fileExistsAtPath:iconPath]) {
                image = [[NSImage alloc] initWithContentsOfFile:iconPath];
            }
        }
        if (!image) image = [[NSWorkspace sharedWorkspace] iconForFile:path];
        if (!image) return NULL;
        NSSize target = NSMakeSize((CGFloat)size, (CGFloat)size);
        NSBitmapImageRep *rep = [[NSBitmapImageRep alloc]
            initWithBitmapDataPlanes:NULL
                          pixelsWide:(NSInteger)size
                          pixelsHigh:(NSInteger)size
                       bitsPerSample:8
                     samplesPerPixel:4
                            hasAlpha:YES
                            isPlanar:NO
                      colorSpaceName:NSDeviceRGBColorSpace
                         bytesPerRow:0
                        bitsPerPixel:0];
        if (!rep) return NULL;
        NSGraphicsContext *context =
            [NSGraphicsContext graphicsContextWithBitmapImageRep:rep];
        if (!context) return NULL;
        [NSGraphicsContext saveGraphicsState];
        NSGraphicsContext.currentContext = context;
        [image drawInRect:NSMakeRect(0, 0, target.width, target.height)
                 fromRect:NSZeroRect
                operation:NSCompositingOperationSourceOver
                 fraction:1.0];
        [NSGraphicsContext restoreGraphicsState];
        NSData *png = [rep representationUsingType:NSBitmapImageFileTypePNG properties:@{}];
        if (png.length == 0) return NULL;
        unsigned char *buffer = malloc(png.length);
        if (!buffer) return NULL;
        memcpy(buffer, png.bytes, png.length);
        if (out_length) *out_length = (unsigned int)png.length;
        return buffer;
    }
}

// Clipboard text snapshot. Returns 1 and fills the output buffer on success,
// 2 when the clipboard holds no plain text, 3 when the value exceeds capacity,
// 0 when reading failed.
int launcher_clipboard_text(char *output, unsigned int capacity, unsigned int *out_length) {
    @autoreleasepool {
        if (out_length) *out_length = 0;
        NSPasteboard *pasteboard = NSPasteboard.generalPasteboard;
        NSString *text = [pasteboard stringForType:NSPasteboardTypeString];
        if (text.length == 0) return 2;
        NSData *data = [text dataUsingEncoding:NSUTF8StringEncoding];
        if (!data) return 2;
        if (out_length) *out_length = (unsigned int)data.length;
        if (data.length > capacity || !output) return 3;
        memcpy(output, data.bytes, data.length);
        return 1;
    }
}
