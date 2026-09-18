#import <AppKit/AppKit.h>
#import <ApplicationServices/ApplicationServices.h>
#import "macos_process.h"
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

static const NSUInteger kMaxMenuItems = 2000;
static _Thread_local CFAbsoluteTime menu_deadline;
static _Thread_local BOOL menu_incomplete;
static _Thread_local NSUInteger menu_visited;
static BOOL menu_expired(void) {
    if (menu_deadline > 0 && CFAbsoluteTimeGetCurrent() >= menu_deadline) menu_incomplete = YES;
    return menu_incomplete;
}

static char *dup_json(id object) {
    if (!object) return NULL;
    NSData *data = [NSJSONSerialization dataWithJSONObject:object options:0 error:NULL];
    if (!data) return NULL;
    NSString *json = [[NSString alloc] initWithData:data encoding:NSUTF8StringEncoding];
    return json ? strdup(json.UTF8String) : NULL;
}

static id ax_value(AXUIElementRef element, CFStringRef attribute) {
    if (!element || menu_expired()) return nil;
    CFTypeRef value = NULL;
    AXError error = AXUIElementCopyAttributeValue(element, attribute, &value);
    if (error == kAXErrorCannotComplete || error == kAXErrorInvalidUIElement) menu_incomplete = YES;
    if (error != kAXErrorSuccess || !value) return nil;
    return CFBridgingRelease(value);
}

static NSString *ax_string(AXUIElementRef element, CFStringRef attribute) {
    id value = ax_value(element, attribute);
    return [value isKindOfClass:NSString.class] ? value : nil;
}

static BOOL ax_bool(AXUIElementRef element, CFStringRef attribute, BOOL fallback) {
    id value = ax_value(element, attribute);
    if ([value isKindOfClass:NSNumber.class]) return [value boolValue];
    return fallback;
}

static BOOL ax_has_press(AXUIElementRef element) {
    CFArrayRef names = NULL;
    if (AXUIElementCopyActionNames(element, &names) != kAXErrorSuccess || !names) return NO;
    NSArray *array = CFBridgingRelease(names);
    return [array containsObject:(__bridge NSString *)kAXPressAction];
}

static BOOL is_apple_menu_title(NSString *title) {
    if (![title isKindOfClass:NSString.class] || title.length == 0) return YES;
    if ([title isEqualToString:@""]) return YES;
    NSString *folded = title.lowercaseString;
    return [folded isEqualToString:@"apple"] || [folded isEqualToString:@"苹果"] ||
           [folded isEqualToString:@"蘋果"];
}

static BOOL is_separator_title(NSString *title) {
    if (![title isKindOfClass:NSString.class]) return YES;
    NSString *trimmed =
        [title stringByTrimmingCharactersInSet:NSCharacterSet.whitespaceAndNewlineCharacterSet];
    if (trimmed.length == 0) return YES;
    if ([trimmed isEqualToString:@"-"] || [trimmed isEqualToString:@"–"] ||
        [trimmed isEqualToString:@"—"]) {
        return YES;
    }
    NSCharacterSet *dashes = [NSCharacterSet characterSetWithCharactersInString:@"-–—"];
    return [trimmed rangeOfCharacterFromSet:dashes.invertedSet].location == NSNotFound;
}

static NSArray *flatten_children_depth(AXUIElementRef element, NSUInteger depth) {
    if (depth > 16) { menu_incomplete = YES; return @[]; }
    if (++menu_visited > 20000 || menu_expired()) { menu_incomplete = YES; return @[]; }
    NSMutableArray *out = [NSMutableArray array];
    id raw = ax_value(element, kAXChildrenAttribute);
    if (![raw isKindOfClass:NSArray.class]) return out;
    for (id childObj in (NSArray *)raw) {
        AXUIElementRef child = (__bridge AXUIElementRef)childObj;
        NSString *role = ax_string(child, kAXRoleAttribute) ?: @"";
        if ([role isEqualToString:@"AXMenu"] || [role isEqualToString:@"AXMenuBar"]) {
            [out addObjectsFromArray:flatten_children_depth(child, depth + 1)];
        } else {
            [out addObject:childObj];
        }
    }
    return out;
}

static NSArray *flatten_children(AXUIElementRef element) {
    return flatten_children_depth(element, 0);
}

static NSString *shortcut_of(AXUIElementRef element) {
    NSString *cmd = ax_string(element, kAXMenuItemCmdCharAttribute);
    if (cmd.length == 0) {
        id key = ax_value(element, kAXMenuItemCmdVirtualKeyAttribute);
        if ([key isKindOfClass:NSNumber.class]) {
            cmd = @{@36:@"↩", @48:@"⇥", @49:@"Space", @51:@"⌫", @53:@"⎋",
                    @115:@"↖", @119:@"↘", @116:@"⇞", @121:@"⇟", @117:@"⌦",
                    @123:@"←", @124:@"→", @125:@"↓", @126:@"↑",
                    @122:@"F1", @120:@"F2", @99:@"F3", @118:@"F4", @96:@"F5",
                    @97:@"F6", @98:@"F7", @100:@"F8", @101:@"F9", @109:@"F10",
                    @103:@"F11", @111:@"F12"}[key];
        }
    }
    if (cmd.length == 0) return nil;
    id modifiersValue = ax_value(element, kAXMenuItemCmdModifiersAttribute);
    unsigned long modifiers =
        [modifiersValue isKindOfClass:NSNumber.class] ? [modifiersValue unsignedLongValue] : 0;
    BOOL noCommand = (modifiers & (1UL << 3)) != 0;
    BOOL shift = (modifiers & (1UL << 0)) != 0;
    BOOL option = (modifiers & (1UL << 1)) != 0;
    BOOL control = (modifiers & (1UL << 2)) != 0;
    NSMutableString *out = [NSMutableString string];
    if (control) [out appendString:@"⌃"];
    if (option) [out appendString:@"⌥"];
    if (shift) [out appendString:@"⇧"];
    if (!noCommand) [out appendString:@"⌘"];
    [out appendString:cmd.uppercaseString];
    return out;
}

// All retained AX references are local to this one bounded discovery/execute call.
static NSArray *menu_children(AXUIElementRef element) {
    NSMutableArray *entries = [NSMutableArray array];
    for (id childObject in flatten_children(element)) {
        AXUIElementRef child = (__bridge AXUIElementRef)childObject;
        NSString *role = ax_string(child, kAXRoleAttribute) ?: @"";
        if (![role isEqualToString:@"AXMenuItem"] && ![role isEqualToString:@"AXMenuBarItem"]) continue;
        [entries addObject:@{
            @"element": childObject, @"role": role,
            @"title": ax_string(child, kAXTitleAttribute) ?: @"",
            @"identifier": ax_string(child, kAXIdentifierAttribute) ?: @"",
        }];
    }
    return entries;
}

static NSDictionary *unique_entry(NSArray *entries, NSString *title, NSString *identifier) {
    NSDictionary *match = nil;
    for (NSDictionary *entry in entries) {
        BOOL matches = identifier.length > 0 ? [entry[@"identifier"] isEqualToString:identifier]
                                            : [entry[@"title"] isEqualToString:title];
        if (matches) {
            if (match) return nil; // Ambiguous identity must never execute an arbitrary item.
            match = entry;
        }
    }
    return match;
}

static void collect_items(AXUIElementRef element, NSArray *path, NSArray *identifiers,
                          NSMutableArray *out, BOOL parentEnabled) {
    if (path.count >= 12) { menu_incomplete = YES; return; }
    if (menu_expired()) return;
    NSArray *children = menu_children(element);
    for (NSDictionary *entry in children) {
        if (++menu_visited > 20000 || out.count >= kMaxMenuItems || menu_expired()) {
            menu_incomplete = YES;
            return;
        }
        AXUIElementRef child = (__bridge AXUIElementRef)entry[@"element"];
        NSString *role = entry[@"role"], *title = entry[@"title"];
        if (path.count == 0 && is_apple_menu_title(title)) continue;
        if (is_separator_title(title)) continue;
        NSString *identifier = entry[@"identifier"];
        if (identifier.length && !unique_entry(children, title, identifier)) identifier = @"";
        // Without a stable identifier, duplicate siblings cannot be relocated safely.
        if (!unique_entry(children, title, identifier)) continue;
        NSArray *nextPath = [path arrayByAddingObject:title];
        NSArray *nextIdentifiers = [identifiers arrayByAddingObject:identifier.length ? (id)identifier : NSNull.null];
        NSArray *descendants = menu_children(child);
        BOOL enabled = parentEnabled && ax_bool(child, kAXEnabledAttribute, descendants.count > 0);
        if (descendants.count > 0) {
            collect_items(child, nextPath, nextIdentifiers, out, enabled);
            continue;
        }
        if (![role isEqualToString:@"AXMenuItem"] || !ax_has_press(child)) continue;
        NSMutableDictionary *item = [@{
            @"title": title, @"menu_path": nextPath, @"identifiers": nextIdentifiers,
            @"enabled": @(enabled),
        } mutableCopy];
        NSString *shortcut = shortcut_of(child);
        if (shortcut.length) item[@"shortcut"] = shortcut;
        [out addObject:item];
    }
}

static AXUIElementRef locate(AXUIElementRef root, NSArray *titles, NSArray *identifiers) {
    if (titles.count == 0 || titles.count != identifiers.count) return NULL;
    AXUIElementRef current = root;
    CFRetain(current);
    for (NSUInteger depth = 0; depth < titles.count; depth++) {
        id title = titles[depth], identifier = identifiers[depth];
        if (![title isKindOfClass:NSString.class] ||
            (identifier != NSNull.null && ![identifier isKindOfClass:NSString.class])) {
            CFRelease(current); return NULL;
        }
        NSArray *children = menu_children(current);
        NSDictionary *entry = unique_entry(children, title, identifier == NSNull.null ? nil : identifier);
        if (!entry || menu_expired()) { CFRelease(current); return NULL; }
        AXUIElementRef next = (__bridge AXUIElementRef)entry[@"element"];
        // A disabled ancestor makes the leaf non-executable as well.
        if (depth + 1 < titles.count && !ax_bool(next, kAXEnabledAttribute, YES)) {
            CFRelease(current); return NULL;
        }
        CFRetain(next);
        CFRelease(current);
        current = next;
    }
    return current;
}

static NSString *process_instance(NSRunningApplication *app) {
    return launcher_process_instance(app.processIdentifier);
}

static BOOL application_identity_matches(NSDictionary *identity, pid_t pid,
                                         NSString *bundle, NSString *instance) {
    id expectedPid = identity[@"pid"], expectedBundle = identity[@"bundle_id"], expectedInstance = identity[@"instance_id"];
    return [expectedPid isKindOfClass:NSNumber.class] && [expectedPid intValue] > 0 &&
           [expectedPid intValue] == pid &&
           [expectedBundle isKindOfClass:NSString.class] && [expectedBundle length] > 0 &&
           [expectedBundle isEqualToString:bundle] &&
           [expectedInstance isKindOfClass:NSString.class] && [expectedInstance length] > 0 &&
           [expectedInstance isEqualToString:instance];
}

static NSRunningApplication *running_app(NSDictionary *identity) {
    id pid = identity[@"pid"];
    if (![pid isKindOfClass:NSNumber.class] || [pid intValue] <= 0) return nil;
    NSRunningApplication *app = [NSRunningApplication runningApplicationWithProcessIdentifier:[pid intValue]];
    if (!app || app.terminated || !application_identity_matches(identity, app.processIdentifier,
                                                               app.bundleIdentifier, process_instance(app))) return nil;
    return app;
}

static NSDictionary *parse_dictionary(const char *json) {
    if (!json) return nil;
    id value = [NSJSONSerialization JSONObjectWithData:[NSData dataWithBytes:json length:strlen(json)] options:0 error:NULL];
    return [value isKindOfClass:NSDictionary.class] ? value : nil;
}

int launcher_accessibility_trusted(int prompt) {
    if (AXIsProcessTrusted()) return 1;
    if (!prompt) return 0;
    __block int trusted = 0;
    void (^check)(void) = ^{
        @autoreleasepool {
            static BOOL prompted = NO;
            if (prompted) { trusted = AXIsProcessTrusted() ? 1 : 0; return; }
            prompted = YES;
            NSDictionary *options =
                @{(__bridge id)kAXTrustedCheckOptionPrompt : @YES};
            trusted = AXIsProcessTrustedWithOptions((__bridge CFDictionaryRef)options) ? 1 : 0;
        }
    };
    if ([NSThread isMainThread]) {
        check();
    } else {
        dispatch_sync(dispatch_get_main_queue(), check);
    }
    return trusted;
}

char *launcher_application_version(const char *bundle_id) {
    @autoreleasepool {
        if (!bundle_id) return NULL;
        NSString *identifier = [NSString stringWithUTF8String:bundle_id];
        if (identifier.length == 0) return NULL;
        NSRunningApplication *running =
            [NSRunningApplication runningApplicationsWithBundleIdentifier:identifier].firstObject;
        NSURL *url = running.bundleURL;
        if (!url) return NULL;
        NSBundle *bundle = [NSBundle bundleWithURL:url];
        NSString *version = bundle.infoDictionary[@"CFBundleShortVersionString"];
        if (![version isKindOfClass:NSString.class] || version.length == 0) {
            version = bundle.infoDictionary[@"CFBundleVersion"];
        }
        if (![version isKindOfClass:NSString.class] || version.length == 0) return NULL;
        return strdup(version.UTF8String);
    }
}

char *launcher_discover_menu_actions(const char *application_json) {
    @autoreleasepool {
        if (!AXIsProcessTrusted()) return strdup("[]");
        NSRunningApplication *running = running_app(parse_dictionary(application_json));
        if (!running) return NULL;
        menu_deadline = CFAbsoluteTimeGetCurrent() + 3.0;
        menu_incomplete = NO;
        menu_visited = 0;
        AXUIElementRef app = AXUIElementCreateApplication(running.processIdentifier);
        AXUIElementSetMessagingTimeout(app, 0.1);
        CFTypeRef menuBar = NULL;
        AXError err = AXUIElementCopyAttributeValue(app, kAXMenuBarAttribute, &menuBar);
        CFRelease(app);
        if (err != kAXErrorSuccess || !menuBar) return NULL;
        NSMutableArray *items = [NSMutableArray array];
        collect_items((AXUIElementRef)menuBar, @[], @[], items, YES);
        CFRelease(menuBar);
        // Never cache a timeout/partial traversal as a successful menu snapshot.
        if (menu_incomplete || menu_expired() || !running_app(parse_dictionary(application_json))) return NULL;
        return dup_json(items);
    }
}

// 0 ok, 1 wrong process, 2 no menu, 3 missing/ambiguous, 4 disabled, 5 press failed,
// 6 invalid locator, 7 untrusted, 8 activation failed.
int launcher_perform_menu_action(const char *application_json, const char *locator_json) {
    @autoreleasepool {
        if (!AXIsProcessTrusted()) return 7;
        NSDictionary *identity = parse_dictionary(application_json);
        NSDictionary *locator = parse_dictionary(locator_json);
        NSArray *titles = locator[@"titles"], *identifiers = locator[@"identifiers"];
        if (![titles isKindOfClass:NSArray.class] || ![identifiers isKindOfClass:NSArray.class] ||
            titles.count == 0 || titles.count != identifiers.count ||
            ![locator[@"bundle_id"] isEqual:identity[@"bundle_id"]]) return 6;
        NSRunningApplication *running = running_app(identity);
        if (!running) return 1;
        if (![running activateWithOptions:NSApplicationActivateAllWindows]) return 8;
        // Activation is asynchronous. A fixed sleep does not prove the target is active.
        for (NSUInteger attempt = 0; attempt < 25 && !running.active; attempt++) usleep(10000);
        if (!running.active || !running_app(identity)) return 8;
        menu_deadline = CFAbsoluteTimeGetCurrent() + 3.0;
        menu_incomplete = NO;
        menu_visited = 0;
        AXUIElementRef app = AXUIElementCreateApplication(running.processIdentifier);
        AXUIElementSetMessagingTimeout(app, 0.1);
        CFTypeRef menuBar = NULL;
        AXError err = AXUIElementCopyAttributeValue(app, kAXMenuBarAttribute, &menuBar);
        CFRelease(app);
        if (err != kAXErrorSuccess || !menuBar) return 2;
        AXUIElementRef item = locate((AXUIElementRef)menuBar, titles, identifiers);
        CFRelease(menuBar);
        if (!item) return 3;
        if (!ax_bool(item, kAXEnabledAttribute, NO)) { CFRelease(item); return 4; }
        if (!running.active || !running_app(identity) || menu_expired()) { CFRelease(item); return 1; }
        AXError press = AXUIElementPerformAction(item, kAXPressAction);
        CFRelease(item);
        return press == kAXErrorSuccess ? 0 : 5;
    }
}
