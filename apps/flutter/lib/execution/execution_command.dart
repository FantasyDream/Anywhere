import 'package:flutter/foundation.dart';
import '../infrastructure/desktop_environment.dart';

/// Copyable command presentation. Execution uses the original argv exclusively.
/// Windows displays PowerShell syntax; other desktop hosts use POSIX syntax.
String formatExecutionCommand(List<String> arguments) {
  if (arguments.isEmpty) return '';
  final plain = RegExp(r'^[a-zA-Z0-9_./:=@+\-]+$');
  final powershell =
      DesktopEnvironment.current.platform == TargetPlatform.windows;
  final rendered = arguments
      .map((argument) => plain.hasMatch(argument)
          ? argument
          : powershell
              ? "'${argument.replaceAll("'", "''")}'"
              : "'${argument.replaceAll("'", "'\"'\"'")}'")
      .join(' ');
  return powershell ? '& $rendered' : rendered;
}
