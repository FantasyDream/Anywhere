#pragma once
#import <Foundation/Foundation.h>
#include <libproc.h>

// NSRunningApplication.launchDate is legitimately nil for processes such as Finder.
// Capture and validation must share the kernel's process-start identity instead.
static inline NSString *launcher_process_instance(pid_t pid) {
  if (pid <= 0) return nil;
  struct proc_bsdinfo info = {0};
  if (proc_pidinfo(pid, PROC_PIDTBSDINFO, 0, &info, sizeof(info)) != sizeof(info)) {
    return nil;
  }
  return [NSString stringWithFormat:@"proc:%llu:%llu", (unsigned long long)info.pbi_start_tvsec,
                                    (unsigned long long)info.pbi_start_tvusec];
}
