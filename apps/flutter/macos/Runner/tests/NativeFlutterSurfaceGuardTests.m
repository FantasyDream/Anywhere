#import <Foundation/Foundation.h>
#import <CoreGraphics/CoreGraphics.h>
#include <assert.h>
#include <dlfcn.h>
#include <stdint.h>

extern int32_t anywhere_install_flutter_surface_guard(void);

@protocol BufferCache <NSObject>
- (id)removeSurfaceForSize:(CGSize)size;
- (void)returnSurfaces:(NSArray *)surfaces;
- (void)flush;
@end

// Real FlutterBackBufferCache uses only size/isInUse and object identity. These
// surfaces model a compositor-held A and an idle B without allocating GPU memory.
@interface TestSurface : NSObject
@property(nonatomic) CGSize size;
@property(nonatomic) BOOL isInUse;
@end
@implementation TestSurface
@end

static TestSurface *makeSurface(CGFloat size, BOOL inUse) {
  TestSurface *surface = [TestSurface new];
  surface.size = CGSizeMake(size, size);
  surface.isInUse = inUse;
  return surface;
}

static void returnMixedSizes(id<BufferCache> cache, TestSurface *held, TestSurface *idle) {
  [cache flush];
  [cache returnSurfaces:@[held]];
  [cache returnSurfaces:@[idle]];
}

int main(int argc, char **argv) {
  @autoreleasepool {
    assert(argc == 2);
    assert(dlopen(argv[1], RTLD_NOW | RTLD_GLOBAL));
    Class cacheClass = NSClassFromString(@"FlutterBackBufferCache");
    assert(cacheClass);
    id<BufferCache> cache = [[cacheClass alloc] init];
    TestSurface *held = makeSurface(100, YES), *idle = makeSurface(50, NO);
    returnMixedSizes(cache, held, idle);
    id before = [cache removeSurfaceForSize:held.size];
    // A fixed SDK may return nil here. In the current pinned runtime, this is B.
    BOOL affectedRuntime = before == idle;
    assert(before == nil || before == idle);
    assert(anywhere_install_flutter_surface_guard() == 1);
    assert(anywhere_install_flutter_surface_guard() == 1);
    returnMixedSizes(cache, held, idle);
    assert([cache removeSurfaceForSize:held.size] == nil);
    TestSurface *matching = makeSurface(100, NO);
    [cache returnSurfaces:@[matching]];
    assert([cache removeSurfaceForSize:held.size] == matching);
    assert([cache removeSurfaceForSize:held.size] == nil); // Only A in use remains.
    [cache flush];
    [cache returnSurfaces:@[idle]];
    assert([cache removeSurfaceForSize:idle.size] == idle);
    assert([cache removeSurfaceForSize:idle.size] == nil); // Empty cache allocates.
    printf("Flutter surface guard: real runtime %s; wrong-size rejected, matching/empty/in-use preserved\n",
           affectedRuntime ? "reproduced wrong-size reuse" : "already rejects mixed sizes");
  }
  return 0;
}
