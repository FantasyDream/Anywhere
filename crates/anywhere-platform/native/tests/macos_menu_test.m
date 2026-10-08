// Exercise production traversal/relocation against synthetic AX nodes. No app is activated.
#import <AppKit/AppKit.h>
#import <ApplicationServices/ApplicationServices.h>
#include <assert.h>
static AXError test_attribute(AXUIElementRef element, CFStringRef attribute, CFTypeRef *out);
static AXError test_actions(AXUIElementRef element, CFArrayRef *out);
static AXError test_press(AXUIElementRef element, CFStringRef action);
static AXError test_timeout(AXUIElementRef element, float timeout);
static NSUInteger test_press_count;
static NSUInteger test_window_reads;
static AXUIElementRef test_prepared_element;
static float test_prepared_timeout;
static BOOL test_require_bound;
static NSUInteger test_attribute_count;
#define AXUIElementCopyAttributeValue test_attribute
#define AXUIElementCopyActionNames test_actions
#define AXUIElementPerformAction test_press
#define AXUIElementSetMessagingTimeout test_timeout
#include "../macos_menu.m"
#undef AXUIElementCopyAttributeValue
#undef AXUIElementCopyActionNames
#undef AXUIElementPerformAction
#undef AXUIElementSetMessagingTimeout
static AXError test_timeout(AXUIElementRef element, float timeout) {
    test_prepared_element = element;
    test_prepared_timeout = timeout;
    NSDictionary *node = (__bridge NSDictionary *)element;
    return node[@"TestTimeoutError"] ? (AXError)[node[@"TestTimeoutError"] intValue] : kAXErrorSuccess;
}
static void test_bound(AXUIElementRef element) {
    if (!test_require_bound) return;
    assert(element == test_prepared_element);
    assert(test_prepared_timeout > 0 && test_prepared_timeout <= 0.101);
    test_prepared_element = NULL; // Every read/press needs its own preparation.
}
static AXError test_press(AXUIElementRef element, CFStringRef action) {
    test_bound(element); (void)action;
    test_press_count++;
    return kAXErrorSuccess;
}
static AXError test_attribute(AXUIElementRef element, CFStringRef attribute, CFTypeRef *out) {
    test_bound(element);
    test_attribute_count++;
    NSDictionary *node = (__bridge NSDictionary *)element;
    if (node[@"TestTitleAfterEnabled"] && CFEqual(attribute, kAXEnabledAttribute)) {
        ((NSMutableDictionary *)node)[@"AXTitle"] = node[@"TestTitleAfterEnabled"];
    }
    NSNumber *fault = node[@"TestAXError"];
    if (fault) return (AXError)fault.intValue;
    if (node[@"TestSecondWindow"] && CFEqual(attribute, kAXFocusedWindowAttribute)) {
        id window = test_window_reads++ == 0 ? node[@"AXFocusedWindow"] : node[@"TestSecondWindow"];
        *out = CFRetain((__bridge CFTypeRef)window);
        return kAXErrorSuccess;
    }
    id value = node[(__bridge NSString *)attribute];
    if (!value) return kAXErrorNoValue;
    *out = CFRetain((__bridge CFTypeRef)value);
    return kAXErrorSuccess;
}
static AXError test_actions(AXUIElementRef element, CFArrayRef *out) {
    test_bound(element);
    *out = CFRetain((__bridge CFArrayRef)@[@"AXPress"]);
    return kAXErrorSuccess;
}
static NSDictionary *leaf(NSString *title, NSString *identifier, BOOL enabled) {
    return @{@"AXRole": @"AXMenuItem", @"AXTitle":title, @"AXIdentifier":identifier, @"AXEnabled":@(enabled)};
}
static AXUIElementRef lookup(NSArray *children, NSArray *titles, NSArray *identifiers) {
    menu_deadline = 0; menu_incomplete = NO; menu_visited = 0;
    menu_ax_failure = kAXErrorSuccess;
    NSDictionary *root = @{@"AXChildren": children};
    return locate((__bridge AXUIElementRef)root, titles, identifiers, NULL);
}
static int lookup_status(NSDictionary *root, NSArray *titles, NSArray *identifiers,
                         CFAbsoluteTime deadline) {
    menu_deadline = deadline; menu_incomplete = NO; menu_visited = 0;
    menu_ax_failure = kAXErrorSuccess;
    int status = -1;
    AXUIElementRef found = locate((__bridge AXUIElementRef)root, titles, identifiers, &status);
    if (found) CFRelease(found);
    return status;
}
static NSDictionary *capture_selection(NSDictionary *focused) {
    NSDictionary *root = focused ? @{ @"AXFocusedUIElement": focused } : @{};
    return selected_text_for_app((__bridge AXUIElementRef)root, NO,
                                 CFAbsoluteTimeGetCurrent() + 1.0);
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
        assert(!lookup(@[localized], @[@"Save"], @[@"save"])); // refresh old display first
        found = lookup(@[localized], @[@"保存"], @[@"save"]);
        assert(found == (__bridge AXUIElementRef)localized); CFRelease(found);
        assert(press_snapshot((__bridge AXUIElementRef)localized, @"保存", @"save", ^BOOL { return YES; }) == 0);
        assert(test_press_count == 1);
        assert(press_snapshot((__bridge AXUIElementRef)localized, @"Save", @"save", ^BOOL { return YES; }) == 3);
        assert(press_snapshot((__bridge AXUIElementRef)localized, @"保存", NSNull.null, ^BOOL { return YES; }) == 3);
        assert(press_snapshot((__bridge AXUIElementRef)localized, @"保存", @"save", ^BOOL { return NO; }) == 1);
        NSMutableDictionary *changing = [leaf(@"Save", @"save", YES) mutableCopy];
        changing[@"TestTitleAfterEnabled"] = @"Delete";
        assert(press_snapshot((__bridge AXUIElementRef)changing, @"Save", @"save", ^BOOL { return YES; }) == 3);
        assert(test_press_count == 1); // No press for stale metadata or target instance.
        assert(!lookup(@[leaf(@"Delete", @"save", YES)], @[@"Save"], @[@"save"]));
        assert(!lookup(@[leaf(@"Save", @"other-command", YES)], @[@"Save"], @[NSNull.null]));
        assert(!lookup(@[leaf(@"Save", @"", YES)], @[@"Save"], @[@"save"]));
        found = lookup(@[leaf(@"Save", @"save", YES), leaf(@"Delete", @"save", YES)],
                       @[@"Save"], @[@"save"]);
        assert(found); CFRelease(found); // Shared IDs remain distinguishable by exact title.
        assert(!lookup(@[leaf(@"Save", @"save", YES), leaf(@"Save", @"save", YES)],
                       @[@"Save"], @[@"save"])); // Duplicate pairs still rejected.
        NSDictionary *disabled = leaf(@"Save", @"save", NO);
        NSDictionary *root = @{@"AXChildren": @[disabled]};
        NSMutableArray *items = [NSMutableArray array];
        collect_items((__bridge AXUIElementRef)root, @[], @[], items, YES);
        assert(items.count == 1 && ![items[0][@"enabled"] boolValue]);
        NSDictionary *ambiguous = @{@"AXChildren": @[leaf(@"Same", @"", YES), leaf(@"Same", @"", YES)]};
        [items removeAllObjects];
        collect_items((__bridge AXUIElementRef)ambiguous, @[], @[], items, YES);
        assert(items.count == 0);
        NSDictionary *duplicateIds = @{@"AXChildren": @[leaf(@"Save", @"shared", YES),
                                                          leaf(@"Delete", @"shared", YES), leaf(@"Open", @"", YES)]};
        collect_items((__bridge AXUIElementRef)duplicateIds, @[], @[], items, YES);
        assert(items.count == 3);
        assert([items[0][@"title"] isEqualToString:@"Save"]);
        assert([items[1][@"title"] isEqualToString:@"Delete"]);
        [items removeAllObjects];
        collect_items((__bridge AXUIElementRef)@{@"AXChildren": @[leaf(@"Save", @"shared", YES),
                          leaf(@"Save", @"shared", YES), leaf(@"Open", @"", YES)]},
                      @[], @[], items, YES);
        assert(items.count == 1 && [items[0][@"title"] isEqualToString:@"Open"]);
        // IDEA-like menus reuse a selector ID at every level, including menu headers.
        NSDictionary *fileMenu = @{@"AXRole": @"AXMenuBarItem", @"AXTitle": @"File",
            @"AXIdentifier": @"handleAction:", @"AXEnabled": @YES,
            @"AXChildren": @[@{@"AXRole": @"AXMenu", @"AXChildren": @[
                leaf(@"Open…", @"handleAction:", YES), leaf(@"Save All", @"handleAction:", YES)]}]};
        NSDictionary *editMenu = @{@"AXRole": @"AXMenuBarItem", @"AXTitle": @"Edit",
            @"AXIdentifier": @"handleAction:", @"AXEnabled": @YES,
            @"AXChildren": @[@{@"AXRole": @"AXMenu", @"AXChildren": @[
                leaf(@"Copy", @"handleAction:", YES), leaf(@"Paste", @"handleAction:", NO)]}]};
        NSDictionary *sharedMenus = @{@"AXChildren": @[fileMenu, editMenu]};
        [items removeAllObjects];
        collect_items((__bridge AXUIElementRef)sharedMenus, @[], @[], items, YES);
        assert(items.count == 4);
        assert(([items[1][@"menu_path"] isEqual:@[@"File", @"Save All"]]));
        assert(([items[1][@"identifiers"] isEqual:@[@"handleAction:", @"handleAction:"]]));
        assert(![items[3][@"enabled"] boolValue]);
        found = lookup(@[editMenu, fileMenu], @[@"File", @"Save All"],
                       @[@"handleAction:", @"handleAction:"]);
        assert(found);
        assert(press_snapshot(found, @"Save All", @"handleAction:", ^BOOL { return YES; }) == 0);
        CFRelease(found);
        assert(test_press_count == 2);
        assert(!lookup(@[editMenu, fileMenu], @[@"File", @"Save As"],
                       @[@"handleAction:", @"handleAction:"]));
        assert(!lookup(@[editMenu, fileMenu], @[@"File", @"Save All"],
                       @[@"handleAction:", @"missing-id"]));
        NSDictionary *parent = @{@"AXRole": @"AXMenuItem", @"AXTitle": @"File", @"AXEnabled":@NO, @"AXChildren":@[save]};
        assert(!lookup(@[parent], @[@"File", @"Save"], @[NSNull.null, NSNull.null]));
        assert(lookup_status(@{@"AXChildren":@[parent]}, @[@"File", @"Save"],
                             @[NSNull.null, NSNull.null], 0) == 4);
        assert(lookup_status(@{@"AXChildren":@[save]}, @[@"Other"],
                             @[NSNull.null], 0) == 3);
        assert(lookup_status(@{@"AXChildren":@[save]}, @[@"Save"],
                             @[NSNull.null], CFAbsoluteTimeGetCurrent() - 1) == 9);
        assert(lookup_status(@{@"TestAXError":@(kAXErrorAPIDisabled)}, @[@"Save"],
                             @[NSNull.null], 0) == 7);
        assert(lookup_status(@{@"TestAXError":@(kAXErrorCannotComplete)}, @[@"Save"],
                             @[NSNull.null], 0) == 9);
        assert(menu_ax_status(kAXErrorAPIDisabled, 2) == 7);
        assert(menu_ax_status(kAXErrorCannotComplete, 5) == 9);
        assert(menu_ax_status(kAXErrorActionUnsupported, 5) == 5);
        menu_deadline = 0; menu_incomplete = NO; menu_ax_failure = kAXErrorSuccess;
        NSDictionary *projectWindow = @{ @"AXTitle": @"project [~/repo] – Foo.java" };
        NSDictionary *projectApp = @{ @"AXFocusedWindow": projectWindow };
        test_require_bound = YES;
        NSDictionary *projectResult = editor_project_window_for_app((__bridge AXUIElementRef)projectApp);
        assert([projectResult[@"status"] isEqualToString:@"ok"]);
        assert([projectResult[@"title"] isEqual:projectWindow[@"AXTitle"]]);
        // The focused window is a separate AX object; app-only timeout fails this.
        menu_deadline = CFAbsoluteTimeGetCurrent() + 0.03;
        assert(ax_value((__bridge AXUIElementRef)projectWindow, kAXTitleAttribute));
        assert(test_prepared_timeout <= 0.03);
        menu_deadline = 0;
        assert(ax_has_press((__bridge AXUIElementRef)save));
        assert(press_snapshot((__bridge AXUIElementRef)save, @"Save", NSNull.null, ^BOOL { return YES; }) == 0);
        NSUInteger readsBeforeFailure = test_attribute_count;
        NSDictionary *invalidWindow = @{ @"TestTimeoutError": @(kAXErrorInvalidUIElement), @"AXTitle": @"stale" };
        assert(!ax_value((__bridge AXUIElementRef)invalidWindow, kAXTitleAttribute));
        assert(menu_incomplete && test_attribute_count == readsBeforeFailure);
        menu_incomplete = NO; menu_ax_failure = kAXErrorSuccess;
        test_require_bound = NO;
        test_window_reads = 0;
        NSDictionary *changingWindow = @{ @"AXFocusedWindow": projectWindow,
            @"TestSecondWindow": @{ @"AXTitle": @"different [~/other] – Bar.java" } };
        assert([editor_project_window_for_app((__bridge AXUIElementRef)changingWindow)[@"status"] isEqualToString:@"changed"]);
        assert([editor_project_window_for_app((__bridge AXUIElementRef)@{})[@"status"] isEqualToString:@"no_window"]);
        NSString *largeTitle = [@"x" stringByPaddingToLength:8193 withString:@"x" startingAtIndex:0];
        assert([editor_project_window_for_app((__bridge AXUIElementRef)@{@"AXFocusedWindow": @{@"AXTitle": largeTitle}})[@"status"] isEqualToString:@"too_large"]);
        menu_deadline = CFAbsoluteTimeGetCurrent() - 1;
        assert([editor_project_window_for_app((__bridge AXUIElementRef)projectApp)[@"status"] isEqualToString:@"timeout"]);
        menu_deadline = 0; menu_incomplete = NO; menu_ax_failure = kAXErrorSuccess;
        assert([editor_project_window_for_app((__bridge AXUIElementRef)@{@"TestAXError": @(kAXErrorAPIDisabled)})[@"status"] isEqualToString:@"untrusted"]);
        menu_deadline = 0; menu_incomplete = NO; menu_ax_failure = kAXErrorSuccess;
        assert([capture_selection(@{ @"AXSelectedText": @"hello" })[@"text"] isEqualToString:@"hello"]);
        assert([capture_selection(@{ @"AXSelectedText": @"" })[@"status"] isEqualToString:@"no_selection"]);
        assert([capture_selection(nil)[@"status"] isEqualToString:@"no_selection"]);
        assert([capture_selection(@{ @"TestAXError": @(kAXErrorAttributeUnsupported) })[@"status"]
                isEqualToString:@"unsupported"]);
        assert([capture_selection(@{ @"TestAXError": @(kAXErrorAPIDisabled) })[@"status"]
                isEqualToString:@"untrusted"]);
        assert([capture_selection(@{ @"TestAXError": @(kAXErrorCannotComplete) })[@"status"]
                isEqualToString:@"timeout"]);
        assert([capture_selection(@{ @"AXSelectedText": @42 })[@"status"] isEqualToString:@"unsupported"]);
        NSDictionary *untrustedFocus = selected_text_for_app(
            (__bridge AXUIElementRef)@{ @"TestAXError": @(kAXErrorAPIDisabled) }, NO,
            CFAbsoluteTimeGetCurrent() + 1.0);
        assert([untrustedFocus[@"status"] isEqualToString:@"untrusted"]);
        NSDictionary *expiredSelection = selected_text_for_app(
            (__bridge AXUIElementRef)@{ @"AXFocusedUIElement": @{ @"AXSelectedText": @"hello" } },
            NO, CFAbsoluteTimeGetCurrent() - 1.0);
        assert([expiredSelection[@"status"] isEqualToString:@"timeout"]);
        NSString *longSelection = [@"x" stringByPaddingToLength:5000 withString:@"x" startingAtIndex:0];
        NSDictionary *capped = capture_selection(@{ @"AXSelectedText": longSelection });
        assert([capped[@"status"] isEqualToString:@"ok"]);
        assert([capped[@"text"] isEqualToString:longSelection]);
        NSString *nearPair = [@"x" stringByPaddingToLength:16380 withString:@"x" startingAtIndex:0];
        NSDictionary *exactLimit = capture_selection(@{ @"AXSelectedText": [nearPair stringByAppendingString:@"😀"] });
        assert([exactLimit[@"status"] isEqualToString:@"ok"]);
        assert([exactLimit[@"text"] lengthOfBytesUsingEncoding:NSUTF8StringEncoding] == 16384);
        NSString *surrogatePair = [nearPair stringByAppendingString:@"😀suffix"];
        NSDictionary *cappedPair = capture_selection(@{ @"AXSelectedText": surrogatePair });
        assert([cappedPair[@"status"] isEqualToString:@"too_large"] && !cappedPair[@"text"]);
        NSDictionary *oversized = capture_selection(@{ @"AXSelectedText": [longSelection stringByPaddingToLength:16385 withString:@"x" startingAtIndex:0] });
        assert([oversized[@"status"] isEqualToString:@"too_large"] && !oversized[@"text"]);
        NSDictionary *identity = @{@"pid":@42, @"bundle_id":@"test.editor", @"instance_id":@"one"};
        assert(application_identity_matches(identity, 42, @"test.editor", @"one"));
        assert(!application_identity_matches(identity, 42, @"other.app", @"one"));
        assert(!application_identity_matches(identity, 43, @"test.editor", @"one"));
        assert(!application_identity_matches(identity, 42, @"test.editor", @"two"));
        menu_deadline = 0; menu_incomplete = NO; menu_ax_failure = kAXErrorSuccess;
        NSDictionary *shortcut = @{@"AXMenuItemCmdVirtualKey":@123, @"AXMenuItemCmdModifiers":@0};
        assert([shortcut_of((__bridge AXUIElementRef)shortcut) isEqualToString:@"⌘←"]);
        puts("Native menu and selection checks passed (identity, ambiguity, insertion, localization, disabled ancestors, AX errors, shortcuts, text cap)");
    }
    return 0;
}
