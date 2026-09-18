// Exercise production traversal/relocation against synthetic AX nodes. No app is activated.
#import <AppKit/AppKit.h>
#import <ApplicationServices/ApplicationServices.h>
#include <assert.h>
static AXError test_attribute(AXUIElementRef element, CFStringRef attribute, CFTypeRef *out);
static AXError test_actions(AXUIElementRef element, CFArrayRef *out);
#define AXUIElementCopyAttributeValue test_attribute
#define AXUIElementCopyActionNames test_actions
#include "../macos_menu.m"
#undef AXUIElementCopyAttributeValue
#undef AXUIElementCopyActionNames
static AXError test_attribute(AXUIElementRef element, CFStringRef attribute, CFTypeRef *out) {
    id value = ((__bridge NSDictionary *)element)[(__bridge NSString *)attribute];
    if (!value) return kAXErrorNoValue;
    *out = CFRetain((__bridge CFTypeRef)value);
    return kAXErrorSuccess;
}
static AXError test_actions(AXUIElementRef element, CFArrayRef *out) {
    (void)element;
    *out = CFRetain((__bridge CFArrayRef)@[@"AXPress"]);
    return kAXErrorSuccess;
}
static NSDictionary *leaf(NSString *title, NSString *identifier, BOOL enabled) {
    return @{@"AXRole": @"AXMenuItem", @"AXTitle":title, @"AXIdentifier":identifier, @"AXEnabled":@(enabled)};
}
static AXUIElementRef lookup(NSArray *children, NSArray *titles, NSArray *identifiers) {
    menu_deadline = 0; menu_incomplete = NO; menu_visited = 0;
    NSDictionary *root = @{@"AXChildren": children};
    return locate((__bridge AXUIElementRef)root, titles, identifiers);
}
int main(void) {
    @autoreleasepool {
        NSString *selfInstance = launcher_process_instance(getpid());
        assert(selfInstance.length > 0);
        assert([selfInstance isEqualToString:launcher_process_instance(getpid())]);
        assert(!launcher_process_instance(-1));
        NSRunningApplication *finder = [NSRunningApplication
            runningApplicationsWithBundleIdentifier:@"com.apple.finder"]
                                           .firstObject;
        assert(finder);
        NSString *instance = launcher_process_instance(finder.processIdentifier);
        assert(instance.length > 0);
        NSDictionary *finderIdentity = @{
            @"pid" : @(finder.processIdentifier),
            @"bundle_id" : @"com.apple.finder",
            @"instance_id" : instance
        };
        assert(running_app(finderIdentity));
        assert(!running_app(@{
            @"pid" : @(finder.processIdentifier),
            @"bundle_id" : @"com.apple.finder"
        }));
        printf("Finder process identity validated (launchDate %s)\n",
               finder.launchDate ? "available" : "nil");
        NSDictionary *a = leaf(@"Duplicate", @"a", YES), *b = leaf(@"Duplicate", @"b", YES);
        AXUIElementRef found = lookup(@[a,b], @[@"Duplicate"], @[@"b"]);
        assert(found == (__bridge AXUIElementRef)b); CFRelease(found);
        assert(!lookup(@[a], @[@"Duplicate"], @[@"b"])); // no title fallback for vanished B
        assert(!lookup(@[a,b], @[@"Duplicate"], @[NSNull.null])); // ambiguous title
        NSDictionary *save = leaf(@"Save", @"", YES);
        found = lookup(@[a,save,b], @[@"Save"], @[NSNull.null]);
        assert(found == (__bridge AXUIElementRef)save); CFRelease(found);
        found = lookup(@[save,a], @[@"Save"], @[NSNull.null]);
        assert(found == (__bridge AXUIElementRef)save); CFRelease(found);
        NSDictionary *localized = leaf(@"保存", @"save", YES);
        found = lookup(@[localized], @[@"Save"], @[@"save"]);
        assert(found == (__bridge AXUIElementRef)localized); CFRelease(found);
        NSDictionary *disabled = leaf(@"Save", @"save", NO);
        NSDictionary *root = @{@"AXChildren": @[disabled]};
        NSMutableArray *items = [NSMutableArray array];
        collect_items((__bridge AXUIElementRef)root, @[], @[], items, YES);
        assert(items.count == 1 && ![items[0][@"enabled"] boolValue]);
        NSDictionary *ambiguous = @{@"AXChildren": @[leaf(@"Same", @"", YES), leaf(@"Same", @"", YES)]};
        [items removeAllObjects];
        collect_items((__bridge AXUIElementRef)ambiguous, @[], @[], items, YES);
        assert(items.count == 0);
        NSDictionary *parent = @{@"AXRole": @"AXMenuItem", @"AXTitle": @"File", @"AXEnabled":@NO, @"AXChildren":@[save]};
        assert(!lookup(@[parent], @[@"File", @"Save"], @[NSNull.null, NSNull.null]));
        NSDictionary *identity = @{@"pid":@42, @"bundle_id":@"test.editor", @"instance_id":@"one"};
        assert(application_identity_matches(identity, 42, @"test.editor", @"one"));
        assert(!application_identity_matches(identity, 42, @"other.app", @"one"));
        assert(!application_identity_matches(identity, 43, @"test.editor", @"one"));
        assert(!application_identity_matches(identity, 42, @"test.editor", @"two"));
        NSDictionary *shortcut = @{@"AXMenuItemCmdVirtualKey":@123, @"AXMenuItemCmdModifiers":@0};
        assert([shortcut_of((__bridge AXUIElementRef)shortcut) isEqualToString:@"⌘←"]);
        puts("Native menu regression checks passed (identity, ambiguity, insertion, localization, hidden leaves, shortcuts)");
    }
    return 0;
}
