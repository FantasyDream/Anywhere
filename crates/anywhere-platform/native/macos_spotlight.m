#import <CoreServices/CoreServices.h>
#import <Foundation/Foundation.h>

// MDQuery accepts all search scopes in one query. Repeating mdfind's undocumented
// -onlyin option cannot establish that every configured root was searched.
unsigned char *launcher_spotlight_paths(const char *name, const char *roots_json,
                                        unsigned int timeout_ms, unsigned int capacity,
                                        int (*cancelled)(void *), void *context,
                                        unsigned int *length, int *status) {
  @autoreleasepool {
    if (length) *length = 0;
    if (status) *status = 0;
    if (!name || !roots_json || !cancelled || !length || !status || !capacity || !timeout_ms ||
        NSThread.isMainThread)
      return NULL;
    NSString *text = [NSString stringWithUTF8String:name];
    NSData *data =
        [[NSString stringWithUTF8String:roots_json] dataUsingEncoding:NSUTF8StringEncoding];
    NSArray *roots = data ? [NSJSONSerialization JSONObjectWithData:data options:0 error:nil] : nil;
    if (!text.length || ![roots isKindOfClass:NSArray.class] || !roots.count || roots.count > 8)
      return NULL;
    NSMutableArray<NSString *> *scopes = [NSMutableArray new];
    for (id root in roots) {
      if (![root isKindOfClass:NSString.class] || ![root isAbsolutePath]) return NULL;
      [scopes addObject:root];
    }
    NSMutableString *literal = [NSMutableString new];
    for (NSUInteger i = 0; i < text.length; ++i) {
      unichar ch = [text characterAtIndex:i];
      if (ch == '\\' || ch == '\'' || ch == '*' || ch == '?') [literal appendString:@"\\"];
      [literal appendFormat:@"%C", ch];
    }
    NSString *expression = [NSString stringWithFormat:@"kMDItemFSName == '*%@*'cd", literal];
    MDQueryRef query =
        MDQueryCreate(kCFAllocatorDefault, (__bridge CFStringRef)expression, NULL, NULL);
    if (!query) return NULL;
    MDQuerySetSearchScope(query, (__bridge CFArrayRef)scopes, 0);
    MDQuerySetMaxCount(query, 4096);
    CFTimeInterval deadline = NSProcessInfo.processInfo.systemUptime + timeout_ms / 1000.0;
    BOOL started = !cancelled(context) && MDQueryExecute(query, 0);
    while (started && !MDQueryIsGatheringComplete(query) && !cancelled(context) &&
           NSProcessInfo.processInfo.systemUptime < deadline)
      CFRunLoopRunInMode(kCFRunLoopDefaultMode, 0.01, true);
    if (cancelled(context))
      *status = 7;
    else if (started && !MDQueryIsGatheringComplete(query))
      *status = 2;
    else if (started)
      *status = 1;
    MDQueryStop(query);
    NSMutableData *output = [NSMutableData new];
    if (*status == 1) {
      CFIndex count = MDQueryGetResultCount(query);
      for (CFIndex i = 0; i < count; ++i) {
        MDItemRef item = (MDItemRef)MDQueryGetResultAtIndex(query, i);
        id value = CFBridgingRelease(MDItemCopyAttribute(item, kMDItemPath));
        if (![value isKindOfClass:NSString.class]) continue;
        NSData *path = [value dataUsingEncoding:NSUTF8StringEncoding];
        if (!path || path.length > 16384) continue;
        if (output.length + path.length + 1 > capacity) break;
        [output appendData:path];
        [output appendBytes:"" length:1];
      }
    }
    CFRelease(query);
    if (*status != 1) return NULL;
    unsigned char *bytes = malloc(MAX(output.length, 1u));
    if (!bytes) {
      *status = 0;
      return NULL;
    }
    memcpy(bytes, output.bytes, output.length);
    *length = (unsigned int)output.length;
    return bytes;
  }
}
