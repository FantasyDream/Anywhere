#import <CoreServices/CoreServices.h>
#import <Foundation/Foundation.h>
#include <assert.h>
static NSString *expression;
static NSArray *scopes;
static BOOL complete = YES;
static MDQueryRef test_create(CFAllocatorRef allocator, CFStringRef text, CFArrayRef values,
                              CFArrayRef sort) {
  (void)allocator;
  (void)values;
  (void)sort;
  expression = (__bridge NSString *)text;
  return (MDQueryRef)CFBridgingRetain([NSObject new]);
}
static void test_scope(MDQueryRef query, CFArrayRef roots, OptionBits options) {
  (void)query;
  (void)options;
  scopes = (__bridge NSArray *)roots;
}
static void test_max(MDQueryRef query, CFIndex count) {
  (void)query;
  assert(count == 4096);
}
static Boolean test_execute(MDQueryRef query, CFOptionFlags flags) {
  (void)query;
  assert(flags == 0);
  return YES;
}
static Boolean test_complete(MDQueryRef query) {
  (void)query;
  return complete;
}
static void test_stop(MDQueryRef query) { (void)query; }
static CFIndex test_count(MDQueryRef query) {
  (void)query;
  return 0;
}
#define MDQueryCreate test_create
#define MDQuerySetSearchScope test_scope
#define MDQuerySetMaxCount test_max
#define MDQueryExecute test_execute
#define MDQueryIsGatheringComplete test_complete
#define MDQueryStop test_stop
#define MDQueryGetResultCount test_count
#include "../macos_spotlight.m"
static int not_cancelled(void *value) { return value != NULL; }
int main(void) {
  @autoreleasepool {
    dispatch_semaphore_t done = dispatch_semaphore_create(0);
    dispatch_async(dispatch_get_global_queue(QOS_CLASS_DEFAULT, 0), ^{
      unsigned int length;
      int status;
      unsigned char *bytes =
          launcher_spotlight_paths("' OR *?\\ 中文", "[\"/first\",\"/second\"]", 100, 1024,
                                   not_cancelled, NULL, &length, &status);
      assert(bytes && status == 1 && length == 0);
      free(bytes);
      assert(scopes.count == 2 && [scopes[1] isEqual:@"/second"]);
      assert([expression isEqual:@"kMDItemFSName == '*\\' OR \\*\\?\\\\ 中文*'cd"]);
      complete = NO;
      assert(!launcher_spotlight_paths("report", "[\"/first\"]", 20, 1024, not_cancelled, NULL,
                                       &length, &status));
      assert(status == 2);
      assert(!launcher_spotlight_paths("report", "[\"/first\"]", 100, 1024, not_cancelled,
                                       (void *)1, &length, &status));
      assert(status == 7);
      dispatch_semaphore_signal(done);
    });
    assert(dispatch_semaphore_wait(done, dispatch_time(DISPATCH_TIME_NOW, 3 * NSEC_PER_SEC)) == 0);
  }
  return 0;
}
