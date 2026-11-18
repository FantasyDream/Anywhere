// Compile/execute only synthetic scripts; no browser/Finder content or TCC prompt.
#import <AppKit/AppKit.h>
#import <Carbon/Carbon.h>
#include <assert.h>
static pid_t sent_pid;
static SInt32 sent_timeout;
static OSErr test_send(const AppleEvent *event, AppleEvent *reply, AESendMode mode,
                       SInt32 timeout) {
  (void)reply;
  (void)mode;
  DescType type;
  Size size;
  assert(AEGetAttributePtr(event, keyAddressAttr, typeKernelProcessID, &type, &sent_pid,
                           sizeof(sent_pid), &size) == noErr);
  sent_timeout = timeout;
  return noErr;
}
#define AESendMessage test_send
#include "../macos_automation.m"
#undef AESendMessage

static void test_worker(void) {
  @autoreleasepool {
    NSRunningApplication *finder =
        [NSRunningApplication runningApplicationsWithBundleIdentifier:@"com.apple.finder"]
            .firstObject;
    assert(finder);
    pid_t pid = finder.processIdentifier;
    NSString *start = launcher_process_instance(pid);
    unsigned int length;
    int status;
    for (int i = 0; i < 2; ++i) {
      unsigned char *bytes = launcher_run_compiled_script(
          "return \"中文\" & (character id 0) & \"line\" & linefeed", pid, "com.apple.finder",
          start.UTF8String, 5000, 100, &length, &status);
      NSData *expected = [@"中文\0line\n" dataUsingEncoding:NSUTF8StringEncoding];
      assert(status == 1 && bytes && length == expected.length);
      assert(memcmp(bytes, expected.bytes, length) == 0);
      free(bytes);
    }
    assert(!launcher_run_compiled_script("return \"long\"", pid, "com.apple.finder",
                                         start.UTF8String, 5000, 1, &length, &status));
    assert(status == 6);
    assert(!launcher_run_compiled_script("error number 17201", pid, "com.apple.finder",
                                         start.UTF8String, 5000, 10, &length, &status));
    assert(status == 3);
    assert(!launcher_run_compiled_script("error number -1743", pid, "com.apple.finder",
                                         start.UTF8String, 5000, 10, &length, &status));
    assert(status == 4);
    assert(!launcher_run_compiled_script("repeat\nset x to 1\nend repeat", pid, "com.apple.finder",
                                         start.UTF8String, 100, 10, &length, &status));
    assert(status == 2);
    assert(!launcher_run_compiled_script("return \"x\"", pid, "com.apple.finder", "proc:0:0", 5000,
                                         10, &length, &status));
    assert(status == 5);

    LauncherScriptCall call = {
        pid, @"com.apple.finder", start, NSProcessInfo.processInfo.systemUptime + 0.5, NULL, 0};
    AppleEvent event = {typeNull, NULL}, reply = {typeNull, NULL};
    AEDesc address = {typeNull, NULL};
    pid_t stale = 1;
    assert(AECreateDesc(typeKernelProcessID, &stale, sizeof(stale), &address) == noErr);
    assert(AECreateAppleEvent('core', 'getd', &address, kAutoGenerateReturnID, kAnyTransactionID,
                              &event) == noErr);
    assert(script_send(&event, &reply, kAEWaitReply, 0, kAEDefaultTimeout, NULL, NULL,
                       (SRefCon)&call) == noErr);
    assert(sent_pid == pid && sent_timeout > 0 && sent_timeout <= 30);
    call.deadline = 0;
    assert(script_send(&event, &reply, kAEWaitReply, 0, kAEDefaultTimeout, NULL, NULL,
                       (SRefCon)&call) == errAETimeout);
    AEDisposeDesc(&event);
    AEDisposeDesc(&address);
    AEDisposeDesc(&reply);
  }
}

int main(void) {
  @autoreleasepool {
    dispatch_semaphore_t done = dispatch_semaphore_create(0);
    dispatch_async(dispatch_get_global_queue(QOS_CLASS_USER_INITIATED, 0), ^{
      test_worker();
      dispatch_semaphore_signal(done);
    });
    // OSA's first compile may ask XProtect to use the main queue. Keep it free.
    NSDate *deadline = [NSDate dateWithTimeIntervalSinceNow:20];
    while (dispatch_semaphore_wait(done, DISPATCH_TIME_NOW) != 0) {
      assert(deadline.timeIntervalSinceNow > 0);
      [NSRunLoop.currentRunLoop runUntilDate:[NSDate dateWithTimeIntervalSinceNow:0.01]];
    }
  }
  return 0;
}
