#import <AppKit/AppKit.h>

static int wait_open_completion(dispatch_semaphore_t done, int *opened) {
  if (NSThread.isMainThread) {
    CFTimeInterval deadline = NSProcessInfo.processInfo.systemUptime + 5.0;
    while (dispatch_semaphore_wait(done, DISPATCH_TIME_NOW) != 0) {
      if (NSProcessInfo.processInfo.systemUptime >= deadline) return 0;
      CFRunLoopRunInMode(kCFRunLoopDefaultMode, 0.01, true);
    }
    return *opened;
  }
  if (dispatch_semaphore_wait(done, dispatch_time(DISPATCH_TIME_NOW, 5 * NSEC_PER_SEC)) != 0)
    return 0;
  return *opened;
}

// AppKit reveal belongs on the main queue. If it cannot begin before the
// deadline, cancel the queued block so it cannot activate Finder later.
static int reveal_on_main_queue(NSWorkspace *workspace, NSURL *url, dispatch_time_t deadline) {
  dispatch_block_t reveal = dispatch_block_create(0, ^{
    [workspace activateFileViewerSelectingURLs:@[ url ]];
  });
  dispatch_async(dispatch_get_main_queue(), reveal);
  if (dispatch_block_wait(reveal, deadline) == 0) return 1;
  dispatch_block_cancel(reveal);
  return 0;
}

// The Rust caller validates/canonicalizes the target and exact application.
// operation: 0 default/application URL open, 1 Finder reveal, 2 Terminal,
// 3 the selected JetBrains installation's documented argument handoff.
int launcher_open_resource(const char *target, int is_file, const char *application,
                           int operation) {
  @autoreleasepool {
    if (!target) return 0;
    NSString *value = [NSString stringWithUTF8String:target];
    if (!value.length) return 0;
    NSURL *url = is_file ? [NSURL fileURLWithPath:value] : [NSURL URLWithString:value];
    if (!url) return 0;
    NSWorkspace *workspace = NSWorkspace.sharedWorkspace;
    if (operation == 1) {
      if (NSApp && !NSThread.isMainThread)
        return reveal_on_main_queue(workspace, url,
                                    dispatch_time(DISPATCH_TIME_NOW, 5 * NSEC_PER_SEC));
      [workspace activateFileViewerSelectingURLs:@[ url ]];
      return 1;
    }
    NSURL *app = nil;
    if (operation == 2)
      app = [workspace URLForApplicationWithBundleIdentifier:@"com.apple.Terminal"];
    else if (application) {
      NSString *path = [NSString stringWithUTF8String:application];
      if (!path.length) return 0;
      app = [NSURL fileURLWithPath:path];
    }
    NSWorkspaceOpenConfiguration *config = [NSWorkspaceOpenConfiguration configuration];
    config.activates = YES;
    config.createsNewApplicationInstance = operation == 3;
    __block int opened = 0;
    dispatch_semaphore_t done = dispatch_semaphore_create(0);
    void (^completion)(NSRunningApplication *, NSError *) =
        ^(NSRunningApplication *running, NSError *error) {
          opened = running != nil && error == nil;
          dispatch_semaphore_signal(done);
        };
    if (!app) {
      if (operation == 2) return 0;
      [workspace openURL:url configuration:config completionHandler:completion];
      return wait_open_completion(done, &opened);
    }
    if (operation == 3) {
      config.arguments = @[ value ];
      [workspace openApplicationAtURL:app configuration:config completionHandler:completion];
    } else
      [workspace openURLs:@[ url ]
          withApplicationAtURL:app
                 configuration:config
             completionHandler:completion];
    return wait_open_completion(done, &opened);
  }
}
