//! A bounded synchronous recognizer used before opening a content workspace.
//! Runs on a Host worker, never the UI thread; cannot perform an action.
use super::ResolvedContext;
use std::sync::Arc;
pub type RecognizeInput = Arc<dyn Fn(&str) -> Option<ResolvedContext> + Send + Sync>;
#[derive(Clone)]
pub struct InputRecognizer {
    pub id: String,
    pub priority: i32,
    pub recognize: RecognizeInput,
}
