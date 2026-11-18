#import <AppKit/AppKit.h>
#include <assert.h>
static NSURL *opened_url, *opened_app;
static NSWorkspaceOpenConfiguration *opened_config;
static BOOL reveal;
@interface TestWorkspace : NSObject
+ (instancetype)sharedWorkspace;
- (BOOL)openURL:(NSURL *)url;
- (void)openURL:(NSURL *)url
        configuration:(NSWorkspaceOpenConfiguration *)config
    completionHandler:(void (^)(NSRunningApplication *, NSError *))completion;
- (void)activateFileViewerSelectingURLs:(NSArray<NSURL *> *)urls;
- (NSURL *)URLForApplicationWithBundleIdentifier:(NSString *)bundle;
- (void)openApplicationAtURL:(NSURL *)app
               configuration:(NSWorkspaceOpenConfiguration *)config
           completionHandler:(void (^)(NSRunningApplication *, NSError *))completion;
- (void)openURLs:(NSArray<NSURL *> *)urls
    withApplicationAtURL:(NSURL *)app
           configuration:(NSWorkspaceOpenConfiguration *)config
       completionHandler:(void (^)(NSRunningApplication *, NSError *))completion;
@end
@implementation TestWorkspace
+ (instancetype)sharedWorkspace {
  static TestWorkspace *value;
  if (!value) value = [self new];
  return value;
}
- (BOOL)openURL:(NSURL *)url {
  opened_url = url;
  return YES;
}
- (void)openURL:(NSURL *)url
        configuration:(NSWorkspaceOpenConfiguration *)config
    completionHandler:(void (^)(NSRunningApplication *, NSError *))completion {
  opened_url = url;
  opened_config = config;
  completion((NSRunningApplication *)[NSObject new], nil);
}
- (void)activateFileViewerSelectingURLs:(NSArray<NSURL *> *)urls {
  opened_url = urls.firstObject;
  reveal = YES;
}
- (NSURL *)URLForApplicationWithBundleIdentifier:(NSString *)bundle {
  assert([bundle isEqual:@"com.apple.Terminal"]);
  return [NSURL fileURLWithPath:@"/Terminal.app"];
}
- (void)openApplicationAtURL:(NSURL *)app
               configuration:(NSWorkspaceOpenConfiguration *)config
           completionHandler:(void (^)(NSRunningApplication *, NSError *))completion {
  opened_app = app;
  opened_config = config;
  completion((NSRunningApplication *)[NSObject new], nil);
}
- (void)openURLs:(NSArray<NSURL *> *)urls
    withApplicationAtURL:(NSURL *)app
           configuration:(NSWorkspaceOpenConfiguration *)config
       completionHandler:(void (^)(NSRunningApplication *, NSError *))completion {
  opened_url = urls.firstObject;
  [self openApplicationAtURL:app configuration:config completionHandler:completion];
}
@end
#define NSWorkspace TestWorkspace
#include "../macos_resources.m"
#undef NSWorkspace
int main(void) {
  @autoreleasepool {
    assert(launcher_open_resource("https://example.org/a?q=中文", 0, NULL, 0) == 1);
    assert([opened_url.scheme isEqual:@"https"]);
    assert(launcher_open_resource("/项目 'quote' $(literal)", 1, "/Exact.app", 0) == 1);
    assert([opened_url.path isEqual:@"/项目 'quote' $(literal)"]);
    assert([opened_app.path isEqual:@"/Exact.app"] && !opened_config.createsNewApplicationInstance);
    assert(launcher_open_resource("/项目 'quote' $(literal)", 1, "/Exact.app", 3) == 1);
    assert(opened_config.createsNewApplicationInstance && opened_config.arguments.count == 1);
    assert([opened_config.arguments.firstObject isEqual:@"/项目 'quote' $(literal)"]);
    assert(launcher_open_resource("/project", 1, NULL, 2) == 1);
    assert([opened_app.path isEqual:@"/Terminal.app"]);
    assert(launcher_open_resource("/project", 1, NULL, 1) == 1 && reveal);

    // Keep the main queue blocked until the worker's deadline expires.
    reveal = NO;
    dispatch_semaphore_t done = dispatch_semaphore_create(0);
    dispatch_async(dispatch_get_global_queue(QOS_CLASS_DEFAULT, 0), ^{
      assert(reveal_on_main_queue([TestWorkspace sharedWorkspace], opened_url,
                                  dispatch_time(DISPATCH_TIME_NOW, NSEC_PER_MSEC)) == 0);
      dispatch_semaphore_signal(done);
    });
    assert(dispatch_semaphore_wait(done, dispatch_time(DISPATCH_TIME_NOW, NSEC_PER_SEC)) == 0);
    __block BOOL drained = NO;
    dispatch_async(dispatch_get_main_queue(), ^{
      drained = YES;
    });
    CFTimeInterval deadline = NSProcessInfo.processInfo.systemUptime + 1.0;
    while (!drained && NSProcessInfo.processInfo.systemUptime < deadline)
      CFRunLoopRunInMode(kCFRunLoopDefaultMode, 0.01, true);
    assert(drained && !reveal);
  }
  return 0;
}
