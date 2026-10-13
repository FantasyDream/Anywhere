// Read-only checks of the exact Automation identity validator. No AX/TCC or focus changes.
#import <AppKit/AppKit.h>
#include <assert.h>
#include <pthread.h>
#include <limits.h>
#include "../macos.m"

static void *verify_finder(void *unused) {
    (void)unused;
    @autoreleasepool {
        NSRunningApplication *finder = [NSRunningApplication
            runningApplicationsWithBundleIdentifier:@"com.apple.finder"].firstObject;
        assert(finder);
        int pid = finder.processIdentifier;
        NSString *instance = launcher_process_instance(pid);
        assert(instance.length > 0);
        for (int i = 0; i < 8; i++) {
            assert(launcher_application_identity_matches(pid, "com.apple.finder", instance.UTF8String) == 1);
        }
        // Neither the bundle alone nor a recycled PID may authorize an Automation reply.
        assert(launcher_application_identity_matches(pid, "wrong.bundle", instance.UTF8String) == 0);
        assert(launcher_application_identity_status(pid, "wrong.bundle", instance.UTF8String) == 4);
        assert(launcher_application_identity_matches(pid, "com.apple.finder", "proc:0:0") == 0);
        assert(launcher_application_identity_status(pid, "com.apple.finder", "proc:0:0") == 5);
        assert(launcher_application_identity_matches(INT_MAX, "com.apple.finder", instance.UTF8String) == 0);
        assert(launcher_application_identity_matches(0, "com.apple.finder", instance.UTF8String) == 0);
        assert(launcher_application_identity_matches(pid, NULL, instance.UTF8String) == 0);
        assert(launcher_application_identity_matches(pid, "com.apple.finder", NULL) == 0);
        assert(launcher_application_identity_status(pid, "com.apple.finder", NULL) == 0);
    }
    return NULL;
}

int main(void) {
    verify_finder(NULL);
    // Rust Automation reads run on blocking workers, not the AppKit main thread.
    pthread_t worker;
    assert(pthread_create(&worker, NULL, verify_finder, NULL) == 0);
    assert(pthread_join(worker, NULL) == 0);
    return 0;
}
