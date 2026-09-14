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
