import 'package:flutter/foundation.dart';
import '../src/rust/api.dart';

/// Generated bridge DTOs compare command lists by identity. Compare values here
/// so a fresh bridge allocation does not invalidate an unchanged task snapshot.
bool sameTaskSummary(
    ExecutionSessionSummaryDto? left, ExecutionSessionSummaryDto? right) {
  if (identical(left, right)) return true;
  if (left == null || right == null) return false;
  return left.id == right.id &&
      left.title == right.title &&
      left.directory == right.directory &&
      listEquals(left.command, right.command) &&
      left.status == right.status &&
      left.exitCode == right.exitCode &&
      left.startedAtMs == right.startedAtMs &&
      left.finishedAtMs == right.finishedAtMs &&
      left.outputRevision == right.outputRevision &&
      left.outputTruncated == right.outputTruncated &&
      left.error == right.error;
}

bool sameTaskList(List<ExecutionSessionSummaryDto> left,
    List<ExecutionSessionSummaryDto> right) {
  if (left.length != right.length) return false;
  for (var index = 0; index < left.length; index++) {
    if (!sameTaskSummary(left[index], right[index])) return false;
  }
  return true;
}
