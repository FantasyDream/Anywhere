#import <AppKit/AppKit.h>
#import <Carbon/Carbon.h>
#include <limits.h>
#include <pthread.h>
#include "macos_process.h"

// Only the five bundled read-only scripts reach this boundary. Serialize OSA
// components on a single dedicated worker thread so ComponentInstance affinity
// (Carbon OSA TN2006) holds across compile, execute, dispose and cache eviction.
@interface LauncherCompiledScript : NSObject
@property(nonatomic) ComponentInstance component;
@property(nonatomic) OSAID script;
@end
// OSA exposes its per-event PID sender through the Carbon component interface.
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wdeprecated-declarations"
@implementation LauncherCompiledScript
- (void)dealloc {
  if (_script != kOSANullScript) OSADispose(_component, _script);
  if (_component) CloseComponent(_component);
}
@end
#pragma clang diagnostic pop

static ComponentInstance script_component(void) {
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wdeprecated-declarations"
  ComponentInstance component = OpenDefaultComponent(kOSAComponentType, typeAppleScript);
#pragma clang diagnostic pop
  return component;
}

typedef struct {
  pid_t pid;
  __unsafe_unretained NSString *bundle;
  __unsafe_unretained NSString *instance;
  CFTimeInterval deadline;
  OSASendUPP originalSend;
  SRefCon originalRef;
} LauncherScriptCall;

static BOOL script_identity_matches(LauncherScriptCall *call) {
  NSRunningApplication *app =
      [NSRunningApplication runningApplicationWithProcessIdentifier:call->pid];
  return app && !app.terminated && [app.bundleIdentifier isEqual:call->bundle] &&
         [launcher_process_instance(call->pid) isEqual:call->instance];
}

static OSErr script_active(SRefCon ref) {
  LauncherScriptCall *call = (LauncherScriptCall *)ref;
  return NSProcessInfo.processInfo.systemUptime >= call->deadline ? errAETimeout : noErr;
}

static OSErr script_send(const AppleEvent *event, AppleEvent *reply, AESendMode mode,
                         AESendPriority priority, SInt32 timeout, AEIdleUPP idle,
                         AEFilterUPP filter, SRefCon ref) {
  LauncherScriptCall *call = (LauncherScriptCall *)ref;
  if (script_active(ref) != noErr) return errAETimeout;
  if (!script_identity_matches(call)) return procNotFound;

  // AppleScript may send standard additions to its own host. Leave those with
  // OSA; bind every external application event to the captured process, never
  // to a bundle lookup that could choose a different instance after a restart.
  AEDesc address = {typeNull, NULL};
  if (AEGetAttributeDesc(event, keyAddressAttr, typeWildCard, &address) != noErr) return paramErr;
  BOOL self = NO;
  if (address.descriptorType == typeKernelProcessID) {
    pid_t target = 0;
    self = AEGetDescData(&address, &target, sizeof(target)) == noErr && target == getpid();
  } else if (address.descriptorType == typeProcessSerialNumber) {
    ProcessSerialNumber target = {0, 0};
    self = AEGetDescData(&address, &target, sizeof(target)) == noErr && target.highLongOfPSN == 0 &&
           target.lowLongOfPSN == kCurrentProcess;
  }
  AEDisposeDesc(&address);
  double ticks = ceil((call->deadline - NSProcessInfo.processInfo.systemUptime) * 60.0);
  if (ticks < 1.0) ticks = 1.0;
  if (ticks > (double)INT32_MAX) ticks = (double)INT32_MAX;
  SInt32 remaining = (SInt32)ticks;
  if (timeout > 0) remaining = MIN(remaining, timeout);
  if (self)
    return call->originalSend(event, reply, mode, priority, remaining, idle, filter,
                              call->originalRef);
  AppleEvent bound = {typeNull, NULL};
  OSErr error = AEDuplicateDesc(event, &bound);
  if (error == noErr)
    error = AEPutAttributePtr(&bound, keyAddressAttr, typeKernelProcessID, &call->pid,
                              sizeof(call->pid));
  if (error == noErr) error = AESendMessage(&bound, reply, mode, remaining);
  AEDisposeDesc(&bound);
  return error;
}

static int script_error_status(ComponentInstance component, OSAError error) {
  if (error == errAETimeout) return 2;
  AEDesc detail = {typeNull, NULL};
  SInt32 number = error;
  if (OSAScriptError(component, kOSAErrorNumber, typeSInt32, &detail) == noErr)
    AEGetDescData(&detail, &number, sizeof(number));
  AEDisposeDesc(&detail);
  if (number == errAEEventNotPermitted) return 4;
  if (number == errAETimeout) return 2;
  if (number == 17201) return 3;
  return 0;
}

// Compilation may also send Apple Events while resolving an application's
// terminology. Use the same captured process/deadline as execution, and restore
// callbacks before their stack-owned context can leave this function.
static OSAError script_compile(LauncherCompiledScript *compiled, const AEDesc *source,
                               LauncherScriptCall *call) {
  OSAActiveUPP active = NULL;
  SRefCon activeRef = 0;
  if (OSAGetSendProc(compiled.component, &call->originalSend, &call->originalRef) != noErr ||
      !call->originalSend || OSAGetActiveProc(compiled.component, &active, &activeRef) != noErr)
    return errOSASystemError;
  if (OSASetSendProc(compiled.component, script_send, (SRefCon)call) != noErr)
    return errOSASystemError;
  if (OSASetActiveProc(compiled.component, script_active, (SRefCon)call) != noErr) {
    OSASetSendProc(compiled.component, call->originalSend, call->originalRef);
    return errOSASystemError;
  }
  @try {
    OSAID script = kOSANullScript;
    OSAError error = OSACompile(
        compiled.component, source,
        kOSAModeNeverInteract | kOSAModeDontReconnect | kOSAModeCantSwitchLayer, &script);
    compiled.script = script;
    return error;
  } @finally {
    OSASetSendProc(compiled.component, call->originalSend, call->originalRef);
    OSASetActiveProc(compiled.component, active, activeRef);
  }
}

typedef struct LauncherOsaJob {
  NSString *text;
  pid_t pid;
  NSString *bundleID;
  NSString *start;
  CFTimeInterval deadline;
  unsigned int capacity;
  unsigned char *bytes;
  unsigned int length;
  int status;
  dispatch_semaphore_t done;
  struct LauncherOsaJob *next;
} LauncherOsaJob;

static pthread_mutex_t osa_queue_mutex = PTHREAD_MUTEX_INITIALIZER;
static pthread_cond_t osa_queue_cond = PTHREAD_COND_INITIALIZER;
static LauncherOsaJob *osa_queue_head = NULL;
static LauncherOsaJob *osa_queue_tail = NULL;

static void execute_osa_job_on_worker(
    LauncherOsaJob *job, NSMutableDictionary<NSString *, LauncherCompiledScript *> *cache) {
  @autoreleasepool {
    LauncherScriptCall call = {job->pid, job->bundleID, job->start, job->deadline, NULL, 0};
    if (script_active((SRefCon)&call) != noErr) {
      job->status = 2;
      return;
    }
    if (!script_identity_matches(&call)) {
      job->status = 5;
      return;
    }
    LauncherCompiledScript *compiled = cache[job->text];
    if (!compiled) {
      compiled = [LauncherCompiledScript new];
      compiled.component = script_component();
      if (!compiled.component) return;
      NSData *utf8 = [job->text dataUsingEncoding:NSUTF8StringEncoding];
      AEDesc input = {typeNull, NULL};
      OSAError error = AECreateDesc(typeUTF8Text, utf8.bytes, utf8.length, &input);
      if (error == noErr) error = script_compile(compiled, &input, &call);
      AEDisposeDesc(&input);
      if (error != noErr) {
        job->status =
            !script_identity_matches(&call) ? 5 : script_error_status(compiled.component, error);
        return;
      }
      if (cache.count >= 8) [cache removeAllObjects];
      cache[job->text] = compiled;
    }
    if (script_active((SRefCon)&call) != noErr) {
      job->status = 2;
      return;
    }
    OSAActiveUPP oldActive = NULL;
    SRefCon oldActiveRef = 0;
    if (OSAGetSendProc(compiled.component, &call.originalSend, &call.originalRef) != noErr ||
        !call.originalSend ||
        OSAGetActiveProc(compiled.component, &oldActive, &oldActiveRef) != noErr)
      return;
    if (OSASetSendProc(compiled.component, script_send, (SRefCon)&call) != noErr) return;
    if (OSASetActiveProc(compiled.component, script_active, (SRefCon)&call) != noErr) {
      OSASetSendProc(compiled.component, call.originalSend, call.originalRef);
      return;
    }
    OSAID result = kOSANullScript;
    OSAError error;
    @try {
      error = OSAExecute(compiled.component, compiled.script, kOSANullScript,
                         kOSAModeNeverInteract | kOSAModeDontReconnect | kOSAModeCantSwitchLayer,
                         &result);
    } @finally {
      OSASetSendProc(compiled.component, call.originalSend, call.originalRef);
      OSASetActiveProc(compiled.component, oldActive, oldActiveRef);
    }
    AEDesc output = {typeNull, NULL};
    if (!script_identity_matches(&call))
      job->status = 5;
    else if (script_active((SRefCon)&call) != noErr)
      job->status = 2;
    else if (error != noErr)
      job->status = script_error_status(compiled.component, error);
    else {
      error = OSACoerceToDesc(compiled.component, result, typeUTF8Text, kOSAModeNull, &output);
      if (error == noErr) job->status = AEGetDescDataSize(&output) > job->capacity ? 6 : 1;
    }
    if (result != kOSANullScript) OSADispose(compiled.component, result);
    if (job->status == 1) {
      job->length = (unsigned int)AEGetDescDataSize(&output);
      job->bytes = malloc(MAX(job->length, 1u));
      if (!job->bytes || AEGetDescData(&output, job->bytes, job->length) != noErr) {
        free(job->bytes);
        job->bytes = NULL;
        job->length = 0;
        job->status = 0;
      }
    }
    AEDisposeDesc(&output);
  }
}

static void *osa_worker_main(void *unused) {
  (void)unused;
  pthread_setname_np("anywhere-osa-worker");
  NSMutableDictionary<NSString *, LauncherCompiledScript *> *cache = [NSMutableDictionary new];
  for (;;) {
    pthread_mutex_lock(&osa_queue_mutex);
    while (!osa_queue_head) {
      pthread_cond_wait(&osa_queue_cond, &osa_queue_mutex);
    }
    LauncherOsaJob *job = osa_queue_head;
    osa_queue_head = job->next;
    if (!osa_queue_head) osa_queue_tail = NULL;
    pthread_mutex_unlock(&osa_queue_mutex);

    execute_osa_job_on_worker(job, cache);
    dispatch_semaphore_signal(job->done);
  }
  return NULL;
}

static BOOL ensure_osa_worker_started(void) {
  static dispatch_once_t once;
  static BOOL started = NO;
  dispatch_once(&once, ^{
    pthread_attr_t attr;
    if (pthread_attr_init(&attr) == 0) {
      pthread_attr_setdetachstate(&attr, PTHREAD_CREATE_DETACHED);
      pthread_t thread;
      started = pthread_create(&thread, &attr, osa_worker_main, NULL) == 0;
      pthread_attr_destroy(&attr);
    }
  });
  return started;
}

// Status: 1 success, 2 deadline, 3 selection limit, 4 permission, 5 changed
// process, 6 output limit, 0 unavailable. The Rust caller frees returned bytes.
unsigned char *launcher_run_compiled_script(const char *source, int pid, const char *bundle,
                                            const char *instance, unsigned int timeout_ms,
                                            unsigned int capacity, unsigned int *length,
                                            int *status) {
  @autoreleasepool {
    if (length) *length = 0;
    if (status) *status = 0;
    if (!source || !bundle || !length || !status || !capacity || !timeout_ms) return NULL;
    NSString *text = [NSString stringWithUTF8String:source];
    NSString *bundleID = [NSString stringWithUTF8String:bundle];
    NSString *start = instance ? [NSString stringWithUTF8String:instance] : nil;
    if (!text || text.length > 16384 || !bundleID.length) return NULL;
    // The diagnostic directory command has no activation snapshot. Resolve one
    // running Finder instance once, then use the same PID/start checks below.
    if (pid == 0 && [bundleID isEqual:@"com.apple.finder"]) {
      NSArray<NSRunningApplication *> *apps =
          [NSRunningApplication runningApplicationsWithBundleIdentifier:bundleID];
      if (apps.count != 1) return NULL;
      pid = apps.firstObject.processIdentifier;
      start = launcher_process_instance(pid);
    }
    if (pid <= 0 || !start.length) return NULL;
    CFTimeInterval deadline = NSProcessInfo.processInfo.systemUptime + timeout_ms / 1000.0;
    LauncherScriptCall preflight = {pid, bundleID, start, deadline, NULL, 0};
    if (!script_identity_matches(&preflight)) {
      *status = 5;
      return NULL;
    }
    if (!ensure_osa_worker_started()) return NULL;

    LauncherOsaJob *job = calloc(1, sizeof(LauncherOsaJob));
    if (!job) return NULL;
    job->text = [text copy];
    job->pid = pid;
    job->bundleID = [bundleID copy];
    job->start = [start copy];
    job->deadline = deadline;
    job->capacity = capacity;
    job->done = dispatch_semaphore_create(0);

    pthread_mutex_lock(&osa_queue_mutex);
    if (osa_queue_tail)
      osa_queue_tail->next = job;
    else
      osa_queue_head = job;
    osa_queue_tail = job;
    pthread_cond_signal(&osa_queue_cond);
    pthread_mutex_unlock(&osa_queue_mutex);

    if (NSThread.isMainThread) {
      while (dispatch_semaphore_wait(job->done, DISPATCH_TIME_NOW) != 0) {
        CFRunLoopRunInMode(kCFRunLoopDefaultMode, 0.01, true);
      }
    } else {
      dispatch_semaphore_wait(job->done, DISPATCH_TIME_FOREVER);
    }

    unsigned char *bytes = job->bytes;
    *length = job->length;
    *status = job->status;
    job->text = nil;
    job->bundleID = nil;
    job->start = nil;
    job->done = nil;
    free(job);
    return bytes;
  }
}
