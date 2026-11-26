use gpui_kit::{Action, App, KeyBinding, KeyBindingContextPredicate};

pub(crate) fn primary_key_binding(
    keystroke: &str,
    action: impl Action,
    context: &str,
    app: &App,
) -> KeyBinding {
    KeyBinding::load(
        keystroke,
        Box::new(action),
        Some(
            KeyBindingContextPredicate::parse(context)
                .expect("valid key context")
                .into(),
        ),
        false,
        None,
        app,
    )
    .expect("valid built-in shortcut")
}

pub(crate) fn primary_modifier_only(command: bool, control: bool) -> bool {
    if cfg!(target_os = "macos") {
        command && !control
    } else {
        control && !command
    }
}

pub(crate) fn primary_shortcut_label(key: &str) -> String {
    if cfg!(target_os = "macos") {
        let key = match key {
            "Enter" => "↵",
            "Shift+Backspace" => "⇧⌫",
            _ => key,
        };
        format!("⌘{key}")
    } else {
        format!("Ctrl+{key}")
    }
}

pub(crate) fn settings_save_shortcut() -> &'static str {
    if cfg!(target_os = "macos") {
        "meta+KeyS"
    } else {
        "control+KeyS"
    }
}
