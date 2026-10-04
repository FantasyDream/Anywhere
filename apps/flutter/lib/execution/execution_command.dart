/// Render literal arguments for copying into a macOS shell; execution still uses
/// the original argument array, never this presentation string.
String formatExecutionCommand(List<String> arguments) {
  final plain = RegExp(r'^[a-zA-Z0-9_./:=@+\-]+$');
  return arguments
      .map((argument) => plain.hasMatch(argument)
          ? argument
          : "'${argument.replaceAll("'", "'\"'\"'")}'")
      .join(' ');
}
