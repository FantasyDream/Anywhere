// Exercise permission and allocation guards without accessing the user's clipboard.
#import <AppKit/AppKit.h>
#include <assert.h>
#ifdef ANYWHERE_TEST_LEGACY_SDK
#undef __MAC_OS_X_VERSION_MAX_ALLOWED
#define __MAC_OS_X_VERSION_MAX_ALLOWED 140000
#define NSPasteboardAccessBehaviorAlwaysDeny unavailable_in_legacy_sdk
#endif
@interface TestPasteboard : NSObject
@property(nonatomic, copy) NSString *text;
@property(nonatomic, copy) NSArray *types;
@property(nonatomic, copy) NSArray *pasteboardItems;
@property(nonatomic) NSInteger accessBehavior;
@property(nonatomic) BOOL denyAfterRead;
@property(nonatomic) NSUInteger reads;
@property(nonatomic) NSInteger changeCount;
+ (instancetype)generalPasteboard;
- (NSString *)stringForType:(NSString *)type;
- (NSData *)dataForType:(NSString *)type;
@end
@implementation TestPasteboard
+ (instancetype)generalPasteboard { static TestPasteboard *board; if (!board) board = [TestPasteboard new]; return board; }
- (NSString *)stringForType:(NSString *)type { (void)type; self.reads++; if (self.denyAfterRead) self.accessBehavior = 3; return self.text; }
- (NSData *)dataForType:(NSString *)type { return [[self stringForType:type] dataUsingEncoding:NSUTF8StringEncoding]; }
@end
#define NSPasteboard TestPasteboard
#include "../macos.m"
#undef NSPasteboard
int main(void) {
    @autoreleasepool {
        TestPasteboard *board = TestPasteboard.generalPasteboard;
        board.accessBehavior = 2; board.types = @[NSPasteboardTypeString]; board.text = @"hello 中文";
        assert(launcher_clipboard_text_guard(64) == 1);
        assert(launcher_clipboard_text_guard(2) == 3);
        assert(launcher_clipboard_format_guard("public.utf8-plain-text", 64) == 1);
        assert(launcher_clipboard_format_guard("public.utf8-plain-text", 2) == 3);
        board.text = nil;
        assert(launcher_clipboard_text_guard(64) == 0);
        board.types = @[]; assert(launcher_clipboard_text_guard(64) == 2);
        board.pasteboardItems = @[@1, @2]; assert(launcher_clipboard_item_count() == 2);
        board.changeCount = 123; assert(launcher_clipboard_change_count() == 123);
        if (@available(macOS 15.4, *)) {
            board.text = @"private"; board.types = @[NSPasteboardTypeString];
            for (NSNumber *behavior in @[@0, @1, @3]) {
                board.accessBehavior = behavior.integerValue;
                NSUInteger reads = board.reads;
                assert(launcher_clipboard_history_access_allowed() == 0);
                assert(launcher_clipboard_format_guard("public.utf8-plain-text", 64) == 4);
                assert(board.reads == reads);
            }
            board.accessBehavior = 3; NSUInteger reads = board.reads;
            assert(launcher_clipboard_text_guard(64) == 4 && board.reads == reads);
            board.accessBehavior = 2; board.denyAfterRead = YES;
            assert(launcher_clipboard_format_guard("public.utf8-plain-text", 64) == 4);
            board.accessBehavior = 2;
            assert(launcher_clipboard_text_guard(64) == 4);
        }
    }
    puts("Clipboard access and allocation guards passed (including legacy SDK)");
    return 0;
}
