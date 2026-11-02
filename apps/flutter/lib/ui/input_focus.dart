import 'package:flutter/widgets.dart';

/// Reattach an existing desktop editor after a native panel returns focus.
/// Keeping the same controller and focus node preserves query and composition.
void requestTextInput(FocusNode focus) {
  final editor = focus.context?.findAncestorStateOfType<EditableTextState>();
  if (editor != null) {
    editor.requestKeyboard();
  } else {
    focus.requestFocus();
  }
}
