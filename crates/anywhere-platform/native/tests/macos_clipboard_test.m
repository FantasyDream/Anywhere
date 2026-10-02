// Exercise the production reader without accessing or changing the user's clipboard.
#import <AppKit/AppKit.h>
#include <assert.h>
@interface TestPasteboard : NSObject
@property(nonatomic, copy) NSString *text;
@property(nonatomic, copy) NSArray *types;
@property(nonatomic) NSInteger accessBehavior;
@property(nonatomic) BOOL denyAfterRead;
@property(nonatomic) NSUInteger reads;
+ (instancetype)generalPasteboard;
- (NSString *)stringForType:(NSString *)type;
@end
@implementation TestPasteboard
+ (instancetype)generalPasteboard {
    static TestPasteboard *board;
    if (!board) board = [TestPasteboard new];
    return board;
}
- (NSString *)stringForType:(NSString *)type {
    (void)type;
    self.reads++;
    if (self.denyAfterRead) self.accessBehavior = 3;
    return self.text;
}
@end
#define NSPasteboard TestPasteboard
#include "../macos.m"
#undef NSPasteboard

int main(void) {
    @autoreleasepool {
        TestPasteboard *board = TestPasteboard.generalPasteboard;
        board.accessBehavior = 2;
        board.types = @[NSPasteboardTypeString];
        board.text = @"hello 中文";
        char output[64] = {0};
        unsigned int length = 0;
        assert(launcher_clipboard_text(output, sizeof(output), &length) == 1);
        NSData *expected = [board.text dataUsingEncoding:NSUTF8StringEncoding];
        assert(length == expected.length && memcmp(output, expected.bytes, length) == 0);
        assert(launcher_clipboard_text(output, 2, &length) == 3);
        board.text = nil;
        assert(launcher_clipboard_text(output, sizeof(output), &length) == 0);
        board.types = @[];
        assert(launcher_clipboard_text(output, sizeof(output), &length) == 2);
        if (@available(macOS 15.4, *)) {
            board.text = @"private";
            board.types = @[NSPasteboardTypeString];
            board.accessBehavior = 3;
            NSUInteger reads = board.reads;
            assert(launcher_clipboard_text(output, sizeof(output), &length) == 4);
            assert(length == 0 && board.reads == reads);
            board.accessBehavior = 1;
            board.denyAfterRead = YES;
            assert(launcher_clipboard_text(output, sizeof(output), &length) == 4);
            assert(length == 0);
        }
    }
    return 0;
}
