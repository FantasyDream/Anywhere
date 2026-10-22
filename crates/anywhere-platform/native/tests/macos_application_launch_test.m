// Exercise the production entrypoint without launching or focusing any real app.
#import <AppKit/AppKit.h>
#import <objc/runtime.h>
#include <assert.h>
#include "../macos.m"

@interface LaunchFixtureApplication : NSObject
@property(nonatomic, copy) NSString *bundleIdentifier;
@property(nonatomic, strong) NSURL *bundleURL;
@property(nonatomic) int activationCalls;
@end
@implementation LaunchFixtureApplication
- (BOOL)activateWithOptions:(NSApplicationActivationOptions)options {
    (void)options;
    self.activationCalls++;
    // The old running-app shortcut fails this fixture's foreground request.
    return NO;
}
@end

@interface LaunchFixtureWorkspace : NSWorkspace
@property(nonatomic, strong) LaunchFixtureApplication *application;
@property(nonatomic, strong) NSURL *openedURL;
@property(nonatomic, strong) NSWorkspaceOpenConfiguration *configuration;
@property(nonatomic) int openCalls;
@property(nonatomic) int runningReads;
@property(nonatomic) BOOL alreadyRunning;
@property(nonatomic) BOOL failOpen;
@property(nonatomic) BOOL deferCompletion;
@property(nonatomic, copy) void (^pending)(NSRunningApplication *, NSError *);
@end
@implementation LaunchFixtureWorkspace
- (NSArray<NSRunningApplication *> *)runningApplications {
    self.runningReads++;
    return self.alreadyRunning ? @[(NSRunningApplication *)self.application] : @[];
}
- (void)openApplicationAtURL:(NSURL *)url
              configuration:(NSWorkspaceOpenConfiguration *)configuration
          completionHandler:(void (^)(NSRunningApplication *, NSError *))completion {
    self.openCalls++;
    self.openedURL = url;
    self.configuration = configuration;
    if (self.deferCompletion) {
        self.pending = completion;
    } else {
        completion((NSRunningApplication *)self.application,
            self.failOpen ? [NSError errorWithDomain:NSCocoaErrorDomain code:4 userInfo:nil] : nil);
    }
}
@end

static LaunchFixtureWorkspace *fixture;
static id fixture_workspace(id receiver, SEL selector) {
    (void)receiver;
    (void)selector;
    return fixture;
}

int main(void) {
    @autoreleasepool {
        NSFileManager *files = NSFileManager.defaultManager;
        NSURL *root = [NSURL fileURLWithPath:[NSTemporaryDirectory()
            stringByAppendingPathComponent:NSUUID.UUID.UUIDString] isDirectory:YES];
        NSURL *app = [root URLByAppendingPathComponent:@"Selected Install.app" isDirectory:YES];
        NSURL *contents = [app URLByAppendingPathComponent:@"Contents" isDirectory:YES];
        assert([files createDirectoryAtURL:contents withIntermediateDirectories:YES attributes:nil error:NULL]);
        NSDictionary *info = @{@"CFBundleIdentifier": @"test.anywhere.selected",
                               @"CFBundleName": @"Selected Install",
                               @"CFBundlePackageType": @"APPL"};
        assert([info writeToURL:[contents URLByAppendingPathComponent:@"Info.plist"] atomically:YES]);
        fixture = [[LaunchFixtureWorkspace alloc] init];
        fixture.application = [[LaunchFixtureApplication alloc] init];
        fixture.application.bundleIdentifier = @"test.anywhere.selected";
        fixture.application.bundleURL = app;
        Method shared = class_getClassMethod(NSWorkspace.class, @selector(sharedWorkspace));
        IMP original = method_setImplementation(shared, (IMP)fixture_workspace);
        @try {
            // Both cold and already-running apps use the selected URL, activate
            // it, and explicitly avoid launching a second instance.
            for (NSNumber *running in @[@NO, @YES]) {
                fixture.alreadyRunning = running.boolValue;
                int before = fixture.openCalls;
                assert(launcher_launch_application(app.path.UTF8String) == 1);
                assert(fixture.openCalls == before + 1);
                assert([fixture.openedURL isEqual:app]);
                assert(fixture.configuration.activates);
                assert(!fixture.configuration.createsNewApplicationInstance);
                assert(fixture.runningReads == 0);
                assert(fixture.application.activationCalls == 0);
            }
            int before = fixture.openCalls;
            assert(launcher_launch_application(NULL) == 0);
            assert(launcher_launch_application("") == 0);
            assert(launcher_launch_application("\xff") == 0);
            assert(fixture.openCalls == before);
            fixture.failOpen = YES;
            assert(launcher_launch_application(app.path.UTF8String) == 0);
            fixture.failOpen = NO;
            fixture.deferCompletion = YES;
            assert(launcher_launch_application(app.path.UTF8String) == 0);
            assert(fixture.pending);
            // A callback after the bounded wait safely releases its captures;
            // it must not retroactively turn the timeout into success.
            fixture.pending((NSRunningApplication *)fixture.application, nil);
            fixture.pending = nil;
        } @finally {
            method_setImplementation(shared, original);
            fixture = nil;
            assert([files removeItemAtURL:root error:NULL]);
        }
        puts("Application launch: selected URL, running reuse, activation, failure and timeout OK");
    }
    return 0;
}
