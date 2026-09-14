//! UI-neutral application, context, command and ranking engines.
pub mod application;
pub mod command;
pub mod context;
pub mod execution;
pub mod history;
pub mod input;
pub mod provider;
pub mod ranking;
pub use application::{Application, LauncherApplication};
pub fn ping() -> &'static str {
    "pong"
}

#[cfg(test)]
mod tests {
    #[test]
    fn core_runs_without_gui() {
        assert_eq!(super::ping(), "pong");
    }
}
