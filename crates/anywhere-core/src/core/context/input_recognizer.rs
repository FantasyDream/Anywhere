//! A bounded, synchronous recognizer used before opening a content workspace.
//! It may inspect metadata, but must not start a process or perform an action.
use super::ResolvedContext;
#[derive(Clone, Copy)]
pub struct InputRecognizer {
    pub id: &'static str,
    pub priority: i32,
    pub recognize: fn(&str) -> Option<ResolvedContext>,
}
