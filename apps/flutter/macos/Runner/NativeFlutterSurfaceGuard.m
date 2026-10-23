#import <CoreGraphics/CoreGraphics.h>
#import <Foundation/Foundation.h>
#import <objc/runtime.h>
#include <stdatomic.h>
#include <stdint.h>
#include <string.h>

// Compatibility for the pinned Flutter macOS runtime (flutter/flutter#192522).
// A back-buffer cache can return a surface of a different size during A/B/A
// resizes. Reject that result before Impeller wraps its Metal texture. Returning
// nil uses FlutterSurfaceManager's normal allocation path; no cache ivars, engine
// resources, private Apple APIs or renderer settings are modified.
@protocol AnywhereSizedSurface <NSObject>
@property(nonatomic, readonly) CGSize size;
@end

static id (*originalTakeSurface)(id, SEL, CGSize);
static atomic_uint rejectedSurfaces;

static id takeMatchingSurface(id cache, SEL selector, CGSize requested) {
  id<AnywhereSizedSurface> surface = originalTakeSurface(cache, selector, requested);
  if (surface && !CGSizeEqualToSize(surface.size, requested)) {
    // Keep diagnostics bounded even if a framework update changes its behavior.
    if (atomic_fetch_add(&rejectedSurfaces, 1) == 0) {
      fprintf(stderr, "{\"event\":\"flutter_stale_surface_discarded\"}\n");
    }
    return nil;
  }
  return surface;
}

static BOOL hasType(Method method, unsigned int argument, const char *expected) {
  if (!method) return NO;
  char *actual = argument == UINT_MAX ? method_copyReturnType(method)
                                      : method_copyArgumentType(method, argument);
  BOOL matches = actual && strcmp(actual, expected) == 0;
  free(actual);
  return matches;
}

// Install once before any engine starts. Refuse a changed ABI instead of calling
// through an incompatible private Flutter selector. SDK upgrades must rerun the
// real-framework regression below and can remove this adapter once fixed upstream.
int32_t anywhere_install_flutter_surface_guard(void) {
  static dispatch_once_t once;
  static int32_t installed;
  dispatch_once(&once, ^{
    Class cache = NSClassFromString(@"FlutterBackBufferCache");
    Class surface = NSClassFromString(@"FlutterSurface");
    if (!cache || !surface) return;
    Method take = class_getInstanceMethod(cache, @selector(removeSurfaceForSize:));
    Method size = class_getInstanceMethod(surface, @selector(size));
    if (!take || !size || method_getNumberOfArguments(take) != 3 ||
        !hasType(take, UINT_MAX, @encode(id)) || !hasType(take, 2, @encode(CGSize)) ||
        !hasType(size, UINT_MAX, @encode(CGSize)))
      return;
    originalTakeSurface = (id(*)(id, SEL, CGSize))method_getImplementation(take);
    method_setImplementation(take, (IMP)takeMatchingSurface);
    installed = 1;
  });
  return installed;
}
