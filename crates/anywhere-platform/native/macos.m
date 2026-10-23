#import <AppKit/AppKit.h>
#include <limits.h>
#include <stdlib.h>
#include <string.h>
#import "macos_application_names.h"
#import "macos_process.h"

// Only OS context capture. All window/UI code lives in the Flutter host.
char *launcher_frontmost_application(void) {
  @autoreleasepool {
    NSRunningApplication *application = NSWorkspace.sharedWorkspace.frontmostApplication;
    if (!application) return NULL;
    NSDictionary *value = @{
      @"bundle_id" : application.bundleIdentifier ?: @"",
      @"name" : application.localizedName ?: @"Unknown application",
      @"pid" : @(application.processIdentifier),
      @"instance_id" : launcher_process_instance(application.processIdentifier) ?: (id)NSNull.null
    };
    NSData *data = [NSJSONSerialization dataWithJSONObject:value options:0 error:NULL];
    if (!data) return NULL;
    NSString *json = [[NSString alloc] initWithData:data encoding:NSUTF8StringEncoding];
    return strdup(json.UTF8String);
  }
}

void launcher_free_string(char *value) { free(value); }
void launcher_free_bytes(unsigned char *value) { free(value); }

// Recheck the frozen app identity around slower Automation calls. Bundle ID
// alone cannot detect a restart, and PID alone can be reused by the kernel.
// 1 = match, 0 = invalid arguments, 2 = missing process, 3 = terminated,
// 4 = bundle mismatch, 5 = process-start mismatch. No app content is returned.
int launcher_application_identity_status(int pid, const char *bundle_id, const char *instance_id) {
  @autoreleasepool {
    if (pid <= 0 || !bundle_id || !instance_id) return 0;
    NSString *expectedBundle = [NSString stringWithUTF8String:bundle_id];
    NSString *expectedInstance = [NSString stringWithUTF8String:instance_id];
    if (expectedBundle.length == 0 || expectedInstance.length == 0) return 0;
    NSRunningApplication *running =
        [NSRunningApplication runningApplicationWithProcessIdentifier:(pid_t)pid];
    NSString *actualInstance = launcher_process_instance((pid_t)pid);
    if (!running || actualInstance.length == 0) return 2;
    if (running.terminated) return 3;
    if (![running.bundleIdentifier isEqualToString:expectedBundle]) return 4;
    if (![actualInstance isEqualToString:expectedInstance]) return 5;
    return 1;
  }
}

int launcher_application_identity_matches(int pid, const char *bundle_id, const char *instance_id) {
  return launcher_application_identity_status(pid, bundle_id, instance_id) == 1;
}

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
    NSString *name = [[NSFileManager defaultManager] displayNameAtPath:path];
    if ([name.lowercaseString hasSuffix:@".app"] && name.length > 4) {
      name = [name substringToIndex:name.length - 4];
    }
    NSString *localized = [bundle objectForInfoDictionaryKey:@"CFBundleDisplayName"];
    if (![localized isKindOfClass:NSString.class] || localized.length == 0) {
      localized = [bundle objectForInfoDictionaryKey:@"CFBundleName"];
    }
    if (![name isKindOfClass:NSString.class] || name.length == 0) {
      name = [localized isKindOfClass:NSString.class] ? localized : nil;
    }
    if (![name isKindOfClass:NSString.class] || name.length == 0) {
      name = path.lastPathComponent.stringByDeletingPathExtension;
    }
    NSArray<NSString *> *preferredNames =
        launcher_names_for_languages(bundle, NSLocale.preferredLanguages);
    if (preferredNames.count > 0) name = preferredNames.firstObject;
    NSMutableDictionary<NSString *, NSString *> *localizedNames = [NSMutableDictionary dictionary];
    for (NSString *language in @[ @"en", @"zh" ]) {
      NSArray<NSString *> *translated = launcher_names_for_languages(
          bundle, @[ [language isEqualToString:@"zh"] ? @"zh-Hans" : language ]);
      if (translated.count > 0) localizedNames[language] = translated.firstObject;
    }
    // Some system apps (e.g. Calculator) keep English only in their base Info.plist.
    if (!localizedNames[@"en"]) {
      for (id candidate in @[
             bundle.infoDictionary[@"CFBundleDisplayName"] ?: NSNull.null,
             bundle.infoDictionary[@"CFBundleName"] ?: NSNull.null,
             path.lastPathComponent.stringByDeletingPathExtension
           ]) {
        if ([candidate isKindOfClass:NSString.class] && [candidate length] > 0) {
          localizedNames[@"en"] = candidate;
          break;
        }
      }
    }
    NSMutableArray<NSString *> *aliases = [NSMutableArray array];
    NSMutableSet<NSString *> *seen = [NSMutableSet setWithObject:name];
    for (id candidate in @[
           localized ?: [NSNull null],
           bundle.infoDictionary[@"CFBundleDisplayName"] ?: [NSNull null],
           bundle.infoDictionary[@"CFBundleName"] ?: [NSNull null],
         ]) {
      if (![candidate isKindOfClass:NSString.class]) continue;
      NSString *alias = (NSString *)candidate;
      if (alias.length == 0 || [seen containsObject:alias]) continue;
      [seen addObject:alias];
      [aliases addObject:alias];
    }
    for (NSString *alias in preferredNames) {
      if (![seen containsObject:alias]) {
        [seen addObject:alias];
        [aliases addObject:alias];
      }
    }
    NSDictionary *value = @{
      @"bundle_id" : identifier,
      @"name" : name,
      @"path" : bundle.bundlePath ?: path,
      @"aliases" : aliases,
      @"localized_names" : localizedNames,
    };
    NSData *data = [NSJSONSerialization dataWithJSONObject:value options:0 error:NULL];
    if (!data) return NULL;
    NSString *json = [[NSString alloc] initWithData:data encoding:NSUTF8StringEncoding];
    return json ? strdup(json.UTF8String) : NULL;
  }
}

// Open the selected install through LaunchServices, reusing a running instance.
// NSWorkspace performs the cooperative activation handoff on macOS 14+; a bare
// NSRunningApplication activation request can succeed without taking foreground.
// Returns 1 when an application was activated or launched, 0 when nothing happened.
int launcher_launch_application(const char *utf8_path) {
  @autoreleasepool {
    if (!utf8_path) return 0;
    NSString *path = [NSString stringWithUTF8String:utf8_path];
    if (path.length == 0) return 0;
    NSURL *url = [NSURL fileURLWithPath:path];
    NSWorkspaceOpenConfiguration *configuration = [NSWorkspaceOpenConfiguration configuration];
    configuration.activates = YES;
    configuration.createsNewApplicationInstance = NO;
    __block int launched = 0;
    dispatch_semaphore_t done = dispatch_semaphore_create(0);
    [NSWorkspace.sharedWorkspace
        openApplicationAtURL:url
               configuration:configuration
           completionHandler:^(NSRunningApplication *application, NSError *error) {
             launched = (application != nil && error == nil) ? 1 : 0;
             dispatch_semaphore_signal(done);
           }];
    // The completion handler runs off the main thread; a bounded wait keeps this
    // call synchronous without blocking the launcher for long.
    if (dispatch_semaphore_wait(
            done, dispatch_time(DISPATCH_TIME_NOW, (int64_t)(3 * NSEC_PER_SEC))) != 0) {
      // A late callback owns its captured result. Do not read that result
      // concurrently after timing out, or report a pending open as success.
      return 0;
    }
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
      NSString *name = iconFile.pathExtension.length > 0
                           ? iconFile
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
    NSBitmapImageRep *rep = [[NSBitmapImageRep alloc] initWithBitmapDataPlanes:NULL
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
    NSGraphicsContext *context = [NSGraphicsContext graphicsContextWithBitmapImageRep:rep];
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

// Reading the generation does not read clipboard contents or prompt for access.
long long launcher_clipboard_change_count(void) {
  @try {
    @autoreleasepool {
      return (long long)NSPasteboard.generalPasteboard.changeCount;
    }
  } @catch (NSException *exception) {
    return -1;
  }
}

// Public policy API is queried dynamically to support old SDKs on new systems.
static NSInteger clipboard_access_behavior(NSPasteboard *pasteboard) {
  if (@available(macOS 15.4, *)) {
    SEL selector = NSSelectorFromString(@"accessBehavior");
    if (![pasteboard respondsToSelector:selector]) return 0;
    NSInteger (*getBehavior)(id, SEL) =
        (NSInteger(*)(id, SEL))[pasteboard methodForSelector:selector];
    return getBehavior(pasteboard, selector);
  }
  return 2;
}
int launcher_clipboard_history_access_allowed(void) {
  @try {
    @autoreleasepool {
      return clipboard_access_behavior(NSPasteboard.generalPasteboard) == 2 ? 1 : 0;
    }
  } @catch (NSException *exception) {
    return 0;
  }
}
// Foreground preflight may prompt. Actual clipboard format reads use clipboard-rs.
int launcher_clipboard_text_guard(unsigned long long capacity) {
  @try {
    @autoreleasepool {
      NSPasteboard *pasteboard = NSPasteboard.generalPasteboard;
      if (clipboard_access_behavior(pasteboard) == 3) return 4;
      NSString *text = [pasteboard stringForType:NSPasteboardTypeString];
      if (clipboard_access_behavior(pasteboard) == 3) return 4;
      if (!text && [pasteboard.types containsObject:NSPasteboardTypeString]) return 0;
      if (!text.length) return 2;
      return [text lengthOfBytesUsingEncoding:NSUTF8StringEncoding] > capacity ? 3 : 1;
    }
  } @catch (NSException *exception) {
    return 0;
  }
}
unsigned long long launcher_clipboard_item_count(void) {
  @try {
    @autoreleasepool {
      return (unsigned long long)NSPasteboard.generalPasteboard.pasteboardItems.count;
    }
  } @catch (NSException *exception) {
    return ULLONG_MAX;
  }
}
// Avoid copying an oversized representation into Rust. Reading NSData can still
// materialize lazy data in the source app; the OS API cannot preflight that cost.
int launcher_clipboard_format_guard(const char *format, unsigned long long capacity) {
  @try {
    @autoreleasepool {
      NSPasteboard *pasteboard = NSPasteboard.generalPasteboard;
      if (clipboard_access_behavior(pasteboard) != 2) return 4;
      NSString *type = format ? [NSString stringWithUTF8String:format] : nil;
      if (!type) return 0;
      NSData *data = [pasteboard dataForType:type];
      if (clipboard_access_behavior(pasteboard) != 2) return 4;
      if (!data) return 0;
      return data.length > capacity ? 3 : 1;
    }
  } @catch (NSException *exception) {
    return 0;
  }
}
